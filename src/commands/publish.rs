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
        let normalized_path = normalize_path(&manifest.base_directory_path, &readme_path);
        if let Err(_) = builder.append_path(&normalized_path) {
            // Maybe do something here
        }
        fs::read_to_string(normalized_path).ok()
    });
    let license_file = package.license_file.as_ref().and_then(|license_file_path| {
        let normalized_path = normalize_path(&manifest.base_directory_path, &license_file_path);
        if let Err(_) = builder.append_path(&normalized_path) {
            // Maybe do something here
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
        let normalized_path = normalize_path(&cwd, &path);
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
    });
    assert!(archive_path.exists());
    assert!(archive_path.is_file());
    if !publish_opts.dry_run {
        let _response: publish_package_mutation::ResponseData =
            execute_query_modifier(&q, |f| f.file(archive_name, archive_path).unwrap()).map_err(
                |e| {
                    #[cfg(feature = "telemetry")]
                    sentry::integrations::anyhow::capture_anyhow(&e);
                    e
                },
            )?;
    }

    println!(
        "Successfully published package `{}@{}`",
        package.name, package.version
    );

    if publish_opts.dry_run {
        info!(
            "Publish succeeded, but package was not published because it was run in dry-run mode"
        );
    }
    Ok(())
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
