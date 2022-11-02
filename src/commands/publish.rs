//! The publish command uploads the package specified in the Manifest (`wapm.toml`)
//! to the wapm registry.
use crate::data::manifest::{Manifest, MANIFEST_FILE_NAME};
use crate::database;
use crate::graphql::execute_query_modifier;
use crate::keys;
use crate::util::create_temp_dir;
use crate::validate;

use console::{style, Emoji};
use flate2::{write::GzEncoder, Compression};
use graphql_client::*;
use rpassword_wasi as rpassword;
use structopt::StructOpt;
use tar::Builder;
use thiserror::Error;

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::io::{Read, Write as IoWrite};
use std::path::{Path, PathBuf};

use wapm_toml::Package;

static UPLOAD: Emoji<'_, '_> = Emoji("⬆️  ", "");
static PACKAGE: Emoji<'_, '_> = Emoji("📦  ", "");

#[derive(StructOpt, Debug)]
pub struct PublishOpt {
    /// Run the publish logic without sending anything to the registry server
    #[structopt(long = "dry-run")]
    dry_run: bool,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package_chunked.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutationChunked;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug, Clone"
)]
struct PublishPackageMutation;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_signed_url.graphql",
    response_derives = "Debug, Clone"
)]
struct GetSignedUrl;

fn normalize_path(cwd: &Path, path: &Path) -> PathBuf {
    let mut out = PathBuf::from(cwd);
    let mut components = path.components();
    if path.is_absolute() {
        warn!(
            "Interpreting absolute path {} as a relative path",
            path.to_string_lossy()
        );
        components.next();
    }
    for comp in components {
        out.push(comp);
    }
    out
}

pub fn publish(publish_opts: PublishOpt) -> anyhow::Result<()> {
    let mut builder = Builder::new(Vec::new());
    let cwd = crate::config::Config::get_current_dir()?;

    validate::validate_directory(cwd.clone())?;

    let manifest = Manifest::find_in_directory(&cwd)?;

    let manifest_path_buf = cwd.join(MANIFEST_FILE_NAME);
    builder.append_path_with_name(&manifest_path_buf, MANIFEST_FILE_NAME)?;
    let package = &manifest.package;
    let modules = manifest.module.as_ref().ok_or(PublishError::NoModule)?;
    let manifest_string = toml::to_string(&manifest)?;

    let readme = package.readme.as_ref().and_then(|readme_path| {
        let normalized_path = normalize_path(&manifest.base_directory_path, readme_path);
        if builder.append_path(&normalized_path).is_err() {
            // TODO: Maybe do something here
        }
        fs::read_to_string(normalized_path).ok()
    });
    let license_file = package.license_file.as_ref().and_then(|license_file_path| {
        let normalized_path = normalize_path(&manifest.base_directory_path, license_file_path);
        if builder.append_path(&normalized_path).is_err() {
            // TODO: Maybe do something here
        }
        fs::read_to_string(normalized_path).ok()
    });

    for module in modules {
        let normalized_path = normalize_path(&manifest.base_directory_path, &module.source);
        normalized_path
            .metadata()
            .map_err(|_| PublishError::SourceMustBeFile {
                module: module.name.clone(),
                path: normalized_path.clone(),
            })?;
        builder
            .append_path(normalized_path)
            .map_err(|_| PublishError::ErrorBuildingPackage(module.name.clone()))?;

        if let Some(bindings) = &module.bindings {
            for path in bindings.referenced_files(&manifest.base_directory_path) {
                let normalized_path = normalize_path(&manifest.base_directory_path, &path);
                normalized_path
                    .metadata()
                    .map_err(|_| PublishError::MissingBindings {
                        module: module.name.clone(),
                        path: normalized_path.clone(),
                    })?;
                builder
                    .append_path(normalized_path)
                    .map_err(|_| PublishError::ErrorBuildingPackage(module.name.clone()))?;
            }
        }
    }

    // bundle the package filesystem
    for (_alias, path) in manifest.fs.unwrap_or_default().iter() {
        let normalized_path = normalize_path(&cwd, path);
        let path_metadata = normalized_path.metadata().map_err(|_| {
            PublishError::MissingManifestFsPath(normalized_path.to_string_lossy().to_string())
        })?;
        if path_metadata.is_dir() {
            builder.append_dir_all(path, &normalized_path)
        } else {
            return Err(PublishError::PackageFileSystemEntryMustBeDirectory(
                path.to_string_lossy().to_string(),
            )
            .into());
        }
        .map_err(|_| {
            PublishError::MissingManifestFsPath(normalized_path.to_string_lossy().to_string())
        })?;
    }

    builder.finish().ok();
    let tar_archive_data = builder.into_inner().map_err(|_|
                                                        // TODO:
                                                        PublishError::NoModule)?;
    let archive_name = "package.tar.gz".to_string();
    let archive_dir = create_temp_dir()?;
    let archive_dir_path: &std::path::Path = archive_dir.as_ref();
    fs::create_dir(archive_dir_path.join("wapm_package"))?;
    let archive_path = archive_dir_path.join("wapm_package").join(&archive_name);
    let mut compressed_archive = fs::File::create(&archive_path).unwrap();
    let mut gz_enc = GzEncoder::new(&mut compressed_archive, Compression::default());

    gz_enc.write_all(&tar_archive_data).unwrap();
    let _compressed_archive = gz_enc.finish().unwrap();
    let mut compressed_archive_reader = fs::File::open(&archive_path)?;

    let maybe_signature_data = sign_compressed_archive(&mut compressed_archive_reader)?;
    let archived_data_size = archive_path.metadata()?.len();
    let use_chunked_uploads = archived_data_size > 1242880;

    assert!(archive_path.exists());
    assert!(archive_path.is_file());

    if publish_opts.dry_run {
        // dry run: publish is done here

        println!(
            "Successfully published package `{}@{}`",
            package.name, package.version
        );

        info!(
            "Publish succeeded, but package was not published because it was run in dry-run mode"
        );

        return Ok(());
    }

    // file is larger than 1MB, use chunked uploads
    if std::env::var("FORCE_WAPM_USE_CHUNKED_UPLOAD").is_ok()
        || (std::env::var("WAPM_USE_CHUNKED_UPLOAD").is_ok() && use_chunked_uploads)
    {
        try_chunked_uploading(
            package,
            &manifest_string,
            &license_file,
            &readme,
            &archive_name,
            &archive_path,
            &maybe_signature_data,
            archived_data_size,
        )
        .or_else(|_| {
            try_default_uploading(
                package,
                &manifest_string,
                &license_file,
                &readme,
                &archive_name,
                &archive_path,
                &maybe_signature_data,
                &publish_opts,
            )
        })
    } else {
        try_default_uploading(
            package,
            &manifest_string,
            &license_file,
            &readme,
            &archive_name,
            &archive_path,
            &maybe_signature_data,
            &publish_opts,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn try_default_uploading(
    package: &Package,
    manifest_string: &String,
    license_file: &Option<String>,
    readme: &Option<String>,
    archive_name: &String,
    archive_path: &PathBuf,
    maybe_signature_data: &SignArchiveResult,
    publish_opts: &PublishOpt,
) -> Result<(), anyhow::Error> {
    let maybe_signature_data = match maybe_signature_data {
        SignArchiveResult::Ok {
            public_key_id,
            signature,
        } => {
            info!(
                "Package successfully signed with public key: \"{}\"!",
                &public_key_id
            );
            Some(publish_package_mutation::InputSignature {
                public_key_key_id: public_key_id.to_string(),
                data: signature.to_string(),
            })
        }
        SignArchiveResult::NoKeyRegistered => {
            // TODO: uncomment this when we actually want users to start using it
            //warn!("Publishing package without a verifying signature. Consider registering a key pair with wapm");
            None
        }
    };

    println!("{} {}Publishing...", style("[1/1]").bold().dim(), PACKAGE,);

    // regular upload
    let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
        name: package.name.to_string(),
        version: package.version.to_string(),
        description: package.description.clone(),
        manifest: manifest_string.to_string(),
        license: package.license.clone(),
        license_file: license_file.to_owned(),
        readme: readme.to_owned(),
        repository: package.repository.clone(),
        homepage: package.homepage.clone(),
        file_name: Some(archive_name.clone()),
        signature: maybe_signature_data,
    });
    assert!(archive_path.exists());
    assert!(archive_path.is_file());

    if !publish_opts.dry_run {
        let _response: publish_package_mutation::ResponseData = execute_query_modifier(&q, |f| {
            f.file(archive_name.to_string(), archive_path).unwrap()
        })
        .map_err(on_error)?;
    }

    println!(
        "Successfully published package `{}@{}`",
        package.name, package.version
    );

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn try_chunked_uploading(
    package: &Package,
    manifest_string: &String,
    license_file: &Option<String>,
    readme: &Option<String>,
    archive_name: &String,
    archive_path: &PathBuf,
    maybe_signature_data: &SignArchiveResult,
    archived_data_size: u64,
) -> Result<(), anyhow::Error> {
    let maybe_signature_data = match maybe_signature_data {
        SignArchiveResult::Ok {
            public_key_id,
            signature,
        } => {
            info!(
                "Package successfully signed with public key: \"{}\"!",
                &public_key_id
            );
            Some(publish_package_mutation_chunked::InputSignature {
                public_key_key_id: public_key_id.to_string(),
                data: signature.to_string(),
            })
        }
        SignArchiveResult::NoKeyRegistered => {
            // TODO: uncomment this when we actually want users to start using it
            //warn!("Publishing package without a verifying signature. Consider registering a key pair with wapm");
            None
        }
    };

    println!("{} {} Uploading...", style("[1/2]").bold().dim(), UPLOAD);

    let get_google_signed_url = GetSignedUrl::build_query(get_signed_url::Variables {
        name: package.name.to_string(),
        version: package.version.to_string(),
    });

    let _response: get_signed_url::ResponseData =
        execute_query_modifier(&get_google_signed_url, |f| {
            f.file(archive_name.clone(), archive_path.clone()).unwrap()
        })?;

    let url = _response.url.ok_or_else(|| {
        anyhow!(
            "could not get signed url for package {}@{}",
            package.name,
            package.version
        )
    })?;

    let signed_url = url.url;
    let url = url::Url::parse(&signed_url).unwrap();
    let client = reqwest::blocking::Client::builder()
        .default_headers(reqwest::header::HeaderMap::default())
        .build()
        .unwrap();

    let res = client
        .post(url)
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .header("x-goog-resumable", "start");

    let result = res.send().unwrap();

    if result.status() != reqwest::StatusCode::from_u16(201).unwrap() {
        return Err(anyhow!(
            "Uploading package failed: got HTTP {:?} when uploading",
            result.status()
        ));
    }

    let headers = result
        .headers()
        .into_iter()
        .filter_map(|(k, v)| {
            let k = k.to_string();
            let v = v.to_str().ok()?.to_string();
            Some((k.to_lowercase(), v))
        })
        .collect::<BTreeMap<_, _>>();

    let session_uri = headers.get("location").unwrap().clone();

    let total = archived_data_size;

    use indicatif::{ProgressBar, ProgressState, ProgressStyle};

    // archive_path
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .open(&archive_path)
        .map_err(|e| anyhow!("cannot open archive {}: {e}", archive_path.display()))?;

    let pb = ProgressBar::new(archived_data_size);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
    .unwrap()
    .with_key("eta", |state: &ProgressState, w: &mut dyn Write| {
        write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap()
    })
    .progress_chars("#>-"));

    let chunk_size = 256 * 1024;
    let file_pointer = 0;

    loop {
        let mut chunk = Vec::with_capacity(chunk_size);
        let n = std::io::Read::by_ref(&mut file)
            .take(chunk_size as u64)
            .read_to_end(&mut chunk)?;
        if n == 0 {
            break;
        }

        let start = file_pointer;
        let end = file_pointer + chunk.len().saturating_sub(1);
        let content_range = format!("bytes {start}-{end}/{total}");

        let client = reqwest::blocking::Client::builder()
            .default_headers(reqwest::header::HeaderMap::default())
            .build()
            .unwrap();

        let res = client
            .put(&session_uri)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header(reqwest::header::CONTENT_LENGTH, format!("{}", chunk.len()))
            .header("Content-Range".to_string(), content_range)
            .body(chunk.to_vec());

        pb.set_position(file_pointer as u64);

        let _response = res.send().map_err(|e| {
            anyhow!("cannot send request to {session_uri} (chunk {}..{}): {e}", file_pointer, file_pointer + chunk_size)
        })?;

        if n < chunk_size {
            break;
        }
    }

    pb.finish_and_clear();

    println!("{} {}Publishing...", style("[2/2]").bold().dim(), PACKAGE,);

    let q =
        PublishPackageMutationChunked::build_query(publish_package_mutation_chunked::Variables {
            name: package.name.to_string(),
            version: package.version.to_string(),
            description: package.description.clone(),
            manifest: manifest_string.to_string(),
            license: package.license.clone(),
            license_file: license_file.to_owned(),
            readme: readme.to_owned(),
            repository: package.repository.clone(),
            homepage: package.homepage.clone(),
            file_name: Some(archive_name.to_string()),
            signature: maybe_signature_data,
            signed_url: Some(signed_url),
        });

    let _response: publish_package_mutation_chunked::ResponseData =
        crate::graphql::execute_query(&q)?;

    println!(
        "Successfully published package `{}@{}`",
        package.name, package.version
    );

    Ok(())
}

fn on_error(e: anyhow::Error) -> anyhow::Error {
    #[cfg(feature = "telemetry")]
    sentry::integrations::anyhow::capture_anyhow(&e);

    e
}

#[derive(Debug, Error)]
enum PublishError {
    #[error("Cannot publish without a module.")]
    NoModule,
    #[error("Unable to publish the \"{module}\" module because \"{}\" is not a file", path.display())]
    SourceMustBeFile { module: String, path: PathBuf },
    #[error("Unable to load the bindings for \"{module}\" because \"{}\" doesn't exist", path.display())]
    MissingBindings { module: String, path: PathBuf },
    #[error("Error building package when parsing module \"{0}\".")]
    ErrorBuildingPackage(String),
    #[error(
        "Path \"{0}\", specified in the manifest as part of the package file system does not exist.",
    )]
    MissingManifestFsPath(String),
    #[error("When processing the package filesystem, found path \"{0}\" which is not a directory")]
    PackageFileSystemEntryMustBeDirectory(String),
}

#[derive(Debug, Clone)]
pub enum SignArchiveResult {
    Ok {
        public_key_id: String,
        signature: String,
    },
    NoKeyRegistered,
}

/// Takes the package archive as a File and attempts to sign it using the active key
/// returns the public key id used to sign it and the signature string itself
pub fn sign_compressed_archive(
    compressed_archive: &mut fs::File,
) -> anyhow::Result<SignArchiveResult> {
    let key_db = database::open_db()?;
    let personal_key = if let Ok(v) = keys::get_active_personal_key(&key_db) {
        v
    } else {
        return Ok(SignArchiveResult::NoKeyRegistered);
    };
    let password = rpassword::prompt_password(&format!(
        "Please enter your password for the key pair {}:",
        &personal_key.public_key_id
    ))
    .ok();
    let private_key = if let Some(priv_key_location) = personal_key.private_key_location {
        match minisign::SecretKey::from_file(&priv_key_location, password) {
            Ok(priv_key_data) => priv_key_data,
            Err(e) => {
                error!(
                    "Could not read private key from location {}: {}",
                    priv_key_location, e
                );
                return Err(e.into());
            }
        }
    } else {
        // TODO: add more info about why this might have happened and what the user can do about it
        warn!("Active key does not have a private key location registered with it!");
        return Err(anyhow!("Cannot sign package, no private key"));
    };
    Ok(SignArchiveResult::Ok {
        public_key_id: personal_key.public_key_id,
        signature: (minisign::sign(
            Some(&minisign::PublicKey::from_base64(
                &personal_key.public_key_value,
            )?),
            &private_key,
            compressed_archive,
            false,
            None,
            None,
        )?
        .to_string()),
    })
}
