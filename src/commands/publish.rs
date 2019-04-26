//! The publish command uploads the package specified in the Manifest (`wapm.toml`)
//! to the wapm registry.
use crate::validate;

use crate::data::manifest::{Manifest, MANIFEST_FILE_NAME};
use crate::graphql::execute_query_modifier;
use flate2::{write::GzEncoder, Compression};
use graphql_client::*;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tar::Builder;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutation;

pub fn publish() -> Result<(), failure::Error> {
    let mut builder = Builder::new(Vec::new());
    let cwd = env::current_dir()?;

    validate::validate_directory(cwd.clone())?;

    let manifest_path_buf = cwd.join(MANIFEST_FILE_NAME);
    let contents =
        fs::read_to_string(&manifest_path_buf).map_err(|_e| PublishError::MissingManifestInCwd)?;
    let manifest: Manifest = toml::from_str(&contents)?;

    builder.append_path_with_name(&manifest_path_buf, MANIFEST_FILE_NAME)?;
    let package = &manifest.package;
    let modules = manifest.module.as_ref().ok_or(PublishError::NoModule)?;
    let manifest_string = toml::to_string(&manifest)?;

    let readme = package.readme.as_ref().and_then(|readme_path| {
        if let Err(_) = builder.append_path(readme_path) {
            // Maybe do something here
        }
        fs::read_to_string(manifest.base_directory_path.join(readme_path)).ok()
    });
    let license_file = package.license_file.as_ref().and_then(|license_file_path| {
        if let Err(_) = builder.append_path(license_file_path) {
            // Maybe do something here
        }
        fs::read_to_string(manifest.base_directory_path.join(license_file_path)).ok()
    });
    // include a LICENSE file if it exists and an explicit license_file was not given
    if package.license_file.is_none() {
        let license_path = PathBuf::from("LICENSE");
        if license_path.exists() {
            builder.append_path(license_path).ok();
        }
    }
    for module in modules {
        if module.source.is_relative() {
            let source_path = manifest.base_directory_path.join(&module.source);
            source_path
                .metadata()
                .map_err(|_| PublishError::SourceMustBeFile(module.name.clone()))?;
            let source_file_name = source_path
                .file_name()
                .ok_or(PublishError::SourceMustBeFile(module.name.clone()))?;
            builder
                .append_path_with_name(&source_path, source_file_name)
                .map_err(|_| PublishError::ErrorBuildingPackage(module.name.clone()))?;
        } else {
            module
                .source
                .metadata()
                .map_err(|_| PublishError::SourceMustBeFile(module.name.clone()))?;
            let source_file_name = module.source.file_name().ok_or(PublishError::NoModule)?;
            builder
                .append_path_with_name(&module.source, source_file_name)
                .map_err(|_| PublishError::ErrorBuildingPackage(module.name.clone()))?;
        }
    }

    builder.finish().ok();
    let tar_archive_data = builder.into_inner().map_err(|_|
                                                        // TODO:
                                                        PublishError::NoModule)?;
    let archive_name = "package.tar.gz".to_string();
    let archive_dir = tempdir::TempDir::new("wapm_package")?;
    let archive_path = archive_dir.as_ref().join(&archive_name);
    let mut compressed_archive = fs::File::create(&archive_path).unwrap();
    let mut gz_enc = GzEncoder::new(&mut compressed_archive, Compression::default());

    gz_enc.write_all(&tar_archive_data).unwrap();
    let _compressed_archive = gz_enc.finish().unwrap();

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
    });
    assert!(archive_path.exists());
    assert!(archive_path.is_file());
    let _response: publish_package_mutation::ResponseData =
        execute_query_modifier(&q, |f| f.file(archive_name, archive_path).unwrap())?;
    Ok(())
}

#[derive(Debug, Fail)]
enum PublishError {
    #[fail(display = "Cannot publish without a module.")]
    NoModule,
    #[fail(display = "Module \"{}\" must have a source that is a file.", _0)]
    SourceMustBeFile(String),
    #[fail(display = "Missing manifest in current directory.")]
    MissingManifestInCwd,
    #[fail(display = "Error building package when parsing module \"{}\".", _0)]
    ErrorBuildingPackage(String),
}
