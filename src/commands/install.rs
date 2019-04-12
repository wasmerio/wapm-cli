use crate::graphql::execute_query;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use flate2::read::GzDecoder;
use graphql_client::*;
use reqwest;
use tar::Archive;

use crate::lock::{get_package_namespace_and_name, regenerate_lockfile, Lockfile};
use crate::manifest::PACKAGES_DIR_NAME;
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
use std::fs::OpenOptions;
use std::io::SeekFrom;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InstallOpt {
    #[structopt(parse(from_str))]
    package: String,
}

#[derive(Debug, Fail)]
enum InstallError {
    #[fail(display = "Package not found in the registry: {}", name)]
    PackageNotFound { name: String },

    #[fail(display = "No package versions available for package {}", name)]
    NoVersionsAvailable { name: String },

    #[fail(display = "Can't process package file {} because {}", name, error)]
    CorruptFile { name: String, error: String },

    #[fail(display = "{}: {}", custom_text, error)]
    MiscError { custom_text: String, error: String },

    #[fail(display = "Failed to regenerate lock file: {}", _0)]
    CannotRegenLockFile(String),

    #[fail(display = "Failed to decompress or open package: {}", _0)]
    CannotOpenPackageArchive(String),
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package.graphql",
    response_derives = "Debug"
)]
struct GetPackageQuery;

/// Loads a GZipped tar in to memory, decompresses it, and unpackages the
/// content to `pkg_name`
fn decompress_and_extract_archive<P: AsRef<Path>, F: io::Seek + io::Read>(
    mut compressed_archive: F,
    pkg_name: P,
) -> Result<(), failure::Error> {
    compressed_archive.seek(SeekFrom::Start(0))?;
    let gz = GzDecoder::new(compressed_archive);
    let mut archive = Archive::new(gz);
    archive
        .unpack(&pkg_name)
        .map_err(|err| InstallError::CorruptFile {
            name: format!("{}", pkg_name.as_ref().display()),
            error: format!("{}", err),
        })?;
    Ok(())
}

pub fn install(options: InstallOpt) -> Result<(), failure::Error> {
    let name = options.package;
    let q = GetPackageQuery::build_query(get_package_query::Variables {
        name: name.to_string(),
    });
    let response: get_package_query::ResponseData = execute_query(&q)?;
    let package = response
        .package
        .ok_or(InstallError::PackageNotFound { name: name.clone() })?;
    let last_version = package
        .last_version
        .ok_or(InstallError::NoVersionsAvailable { name: name })?;

    let (namespace, pkg_name) = get_package_namespace_and_name(&package.name)?;

    let fully_qualified_package_name =
        fully_qualified_package_display_name(&pkg_name, &last_version.version);
    let current_dir = env::current_dir()?;
    let package_dir = create_package_dir(&current_dir, &namespace, &fully_qualified_package_name)
        .map_err(|err| InstallError::MiscError {
        custom_text: "Could not create package directory".to_string(),
        error: format!("{}", err),
    })?;
    let download_url = last_version.distribution.download_url;
    let mut response = reqwest::get(&download_url)?;
    let temp_dir =
        tempdir::TempDir::new("wapm_package_install").map_err(|err| InstallError::MiscError {
            custom_text: "Failed to create temporary directory to open the package in".to_string(),
            error: format!("{}", err),
        })?;
    let temp_tar_gz_path = temp_dir.path().join("package.tar.gz");
    let mut dest = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(&temp_tar_gz_path)
        .map_err(|err| InstallError::MiscError {
            custom_text: "Could not open temporary directory for compressed archive".to_string(),
            error: format!("{}", err),
        })?;
    io::copy(&mut response, &mut dest).map_err(|err| InstallError::MiscError {
        custom_text: "Could not copy response to temporary directory".to_string(),
        error: format!("{}", err),
    })?;
    decompress_and_extract_archive(dest, &package_dir)
        .map_err(|err| InstallError::CannotOpenPackageArchive(format!("{}", err)))?;
    let manifest_file_path = current_dir.join(MANIFEST_FILE_NAME);
    let mut maybe_manifest = Manifest::open(&manifest_file_path);
    let mut lockfile_string = String::new();
    let maybe_lockfile = Lockfile::open(&current_dir, &mut lockfile_string);
    // with the manifest updated, we can now regenerate the lockfile
    regenerate_lockfile(
        maybe_manifest,
        maybe_lockfile,
        vec![(&package.name, &last_version.version)],
    )
    .map_err(|err| InstallError::CannotRegenLockFile(format!("{}", err)))?;
    println!("Package installed successfully to wapm_packages!");
    Ok(())
}

fn create_package_dir<P: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P,
    namespace_dir: P2,
    fully_qualified_package_name: &str,
) -> Result<PathBuf, io::Error> {
    let mut package_dir = project_dir.as_ref().join(PACKAGES_DIR_NAME);
    package_dir.push(namespace_dir);
    package_dir.push(&fully_qualified_package_name);
    println!("created package_dir {}", package_dir.display());

    fs::create_dir_all(&package_dir)?;
    Ok(package_dir)
}

#[inline]
fn fully_qualified_package_display_name(package_name: &str, package_version: &str) -> String {
    format!("{}@{}", package_name, package_version)
}

#[cfg(test)]
mod test {
    use crate::commands::install::{create_package_dir, fully_qualified_package_display_name};
    use crate::manifest::PACKAGES_DIR_NAME;
    use std::path::PathBuf;

    #[test]
    fn creates_package_directory() {
        let tmp_dir = tempdir::TempDir::new("install_package").unwrap();
        let expected_package_version = "0.1.2";
        let expected_package_name = "my_pkg";
        let expected_fully_qualified_package_name =
            fully_qualified_package_display_name(expected_package_name, expected_package_version);
        let tmp_dir_path = tmp_dir.path();
        let expected_package_directory = tmp_dir_path.join(
            [PACKAGES_DIR_NAME, "_/my_pkg@0.1.2"]
                .iter()
                .collect::<PathBuf>(),
        );
        let actual_package_directory =
            create_package_dir(tmp_dir_path, "_", &expected_fully_qualified_package_name).unwrap();
        assert!(expected_package_directory.exists());
        assert_eq!(expected_package_directory, actual_package_directory);
    }
}
