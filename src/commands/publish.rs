//! The publish command uploads the package specified in the Manifest (`wapm.toml`)
//! to the wapm registry.
use crate::data::manifest::{Manifest, MANIFEST_FILE_NAME};
use crate::database;
use crate::graphql::execute_query_modifier;
use crate::keys;
use crate::util::create_temp_dir;
use crate::validate;

use flate2::{write::GzEncoder, Compression};
use graphql_client::*;
use rpassword_wasi as rpassword;
use structopt::StructOpt;
use tar::Builder;
use thiserror::Error;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(StructOpt, Debug)]
pub struct PublishOpt {
    /// Run the publish logic without sending anything to the registry server
    #[structopt(long = "dry-run")]
    dry_run: bool,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutation;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_signed_url.graphql",
    response_derives = "Debug"
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

    let maybe_signature_data = match sign_compressed_archive(&mut compressed_archive_reader)? {
        SignArchiveResult::Ok {
            public_key_id,
            signature,
        } => {
            info!(
                "Package successfully signed with public key: \"{}\"!",
                &public_key_id
            );
            Some(publish_package_mutation::InputSignature {
                public_key_key_id: public_key_id,
                data: signature,
            })
        }
        SignArchiveResult::NoKeyRegistered => {
            // TODO: uncomment this when we actually want users to start using it
            //warn!("Publishing package without a verifying signature. Consider registering a key pair with wapm");
            None
        }
    };

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
    if true {

        let get_google_signed_url = GetSignedUrl::build_query(get_signed_url::Variables {
            name: package.name.to_string(),
            version: package.version.to_string(),
        });
        
        let _response: get_signed_url::ResponseData =
        execute_query_modifier(
            &get_google_signed_url, 
            |f| f.file(archive_name.clone(), archive_path.clone()).unwrap()
        ).map_err(
            |e| {
                #[cfg(feature = "telemetry")]
                sentry::integrations::anyhow::capture_anyhow(&e);
                e
            },
        )?;

        let url = _response.url
        .ok_or({
            let e = anyhow!("could not get signed url for package {}@{}", package.name, package.version);
            #[cfg(feature = "telemetry")]
            sentry::integrations::anyhow::capture_anyhow(&e);
            e
        })?;

        let url = url::Url::parse(&url.url).unwrap();

        println!("got signed url: {}", url);

        /*

        https://storage.googleapis.com/wapm-io-backend-test-bucket/felix-test_wamp-0.1.0-16648c49-b2c3-4390-b6d8-84ba546b488d.tar.gz?
        X-Goog-Algorithm=GOOG4-RSA-SHA256
        X-Goog-Credential=storage-object-service%40wasmer.iam.gserviceaccount.com%2F20220929%2Fauto%2Fstorage%2Fgoog4_request
        X-Goog-Date=20220929T114028Z
        X-Goog-Expires=60
        X-Goog-SignedHeaders=content-type%3Bhost
        X-Goog-Signature=5282fcccf4e2af7356ff83a4c3e96bb8d91cf7be525fc0e03c77fd8d6c751630dd64cbb436f7122b75e1669751afb75ca2d0d5845fbe22a9bcbe31ab35aa2feec265010e44e90c45f4eaf0318a54b815e2ba523b1458e211ee45858ae39a4e2b9f248f24351c76fcf9f57ca2fb704e8f1e1467b0f6ef389d1de53d7b273113cbc8c674153e9a9a5d6927475cfb098e947bba318c1665f950f614c403e53740a0d571f322d01ee15a8b8a30a342e49e36ef45b9a036cc87aab9e555c746aa668f502377b90560275297869a7fe6b06e6ca772453d6dfab934ea633ce432113bce60f768280fcf7829e443a7db24c600ea83a45d1e15b10b0507a068e4f0235d13

        POST /paris.jpg?uploads HTTP/2
        Host: travel-maps.storage.googleapis.com
        Date: Wed, 24 Mar 2021 18:11:50 GMT
        Content-Type: image/jpg
        Content-Length: 0
        Authorization: Bearer ya29.AHES6ZRVmB7fkLtd1XTmq6mo0S1wqZZi3-Lh_s-6Uw7p8vtgSwg
        */

        let client = reqwest::blocking::Client::new();
        let res = client.post(url)
            .header(reqwest::header::CONTENT_TYPE, "host")
            .header(reqwest::header::CONTENT_LENGTH, archived_data_size.to_string())
            .send()
            .unwrap();

        let posted = res.text().unwrap();

        println!("auth API: {}", posted);
        
        let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
            name: package.name.to_string(),
            version: package.version.to_string(),
            description: package.description.clone(),
            manifest: manifest_string,
            license: package.license.clone(),
            license_file,
            readme,
            repository: package.repository.clone(),
            homepage: package.homepage.clone(),
            file_name: Some(archive_name.clone()),
            signature: maybe_signature_data,
            uploaded_filename: None,
        });

        return Ok(());
    }

    // regular upload
    let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
        name: package.name.to_string(),
        version: package.version.to_string(),
        description: package.description.clone(),
        manifest: manifest_string,
        license: package.license.clone(),
        license_file,
        readme,
        repository: package.repository.clone(),
        homepage: package.homepage.clone(),
        file_name: Some(archive_name.clone()),
        signature: maybe_signature_data,
        uploaded_filename: None,
    });
    assert!(archive_path.exists());
    assert!(archive_path.is_file());

    if !publish_opts.dry_run {
        let _response: publish_package_mutation::ResponseData =
            execute_query_modifier(&q, |f| f.file(archive_name, archive_path).unwrap())
                .map_err(on_error)?;
    }

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

#[derive(Debug)]
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
