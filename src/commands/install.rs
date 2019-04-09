use crate::graphql::execute_query;
use std::fs::File;
use std::io::{copy, Read};
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use flate2::read::GzDecoder;
use graphql_client::*;
use reqwest;
use tar::Archive;

use crate::lock::{get_package_namespace_and_name, regenerate_lockfile, Lockfile};
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
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
fn decompress_and_extract_archive(
    mut compressed_archive: File,
    pkg_name: &str,
) -> Result<(), failure::Error> {
    let mut compressed_archive_data = Vec::new();
    compressed_archive
        .read_to_end(&mut compressed_archive_data)
        .map_err(|err| InstallError::CorruptFile {
            name: format!("{:?}", compressed_archive),
            error: format!("{}", err),
        })?;

    let mut gz = GzDecoder::new(&compressed_archive_data[..]);
    let mut archive_data = Vec::new();
    gz.read_to_end(&mut archive_data)
        .map_err(|err| InstallError::CorruptFile {
            name: format!("{}", pkg_name),
            error: format!("{}", err),
        })?;

    // deal with uncompressed data
    let mut archive = Archive::new(archive_data.as_slice());

    archive
        .unpack(pkg_name)
        .map_err(|err| InstallError::CorruptFile {
            name: format!("{}", pkg_name),
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
    println!("Installing package {}", fully_qualified_package_name);
    let current_dir = env::current_dir()?;

    let package_dir = create_package_dir(&current_dir, &namespace, &fully_qualified_package_name)?;
    let download_url = last_version.distribution.download_url;
    let mut response = reqwest::get(&download_url)?;

    // REVIEW: where should we put the compressed archive?
    let compressed_package_archive_path = package_dir.join(&format!("temp/{}.tar.gz", pkg_name));
    let mut dest = File::create(compressed_package_archive_path.clone())?;
    copy(&mut response, &mut dest)?;

    let decompress_and_extract_result = decompress_and_extract_archive(dest, pkg_name);
    if let Err(_) = fs::remove_dir_all(compressed_package_archive_path) {
        // warn here?
    }
    // return after cleaning up if we failed
    let _ = decompress_and_extract_result?;

    let manifest_file_path = current_dir.join(MANIFEST_FILE_NAME);

    let mut maybe_manifest = Manifest::open(&manifest_file_path);
    let maybe_lockfile = Lockfile::open(current_dir);

    match maybe_manifest {
        Ok(ref mut manifest) => {
            manifest.add_dependency(&package.name, &last_version.version);
        }
        _ => {}
    };

    // with the manifest updated, we can now regenerate the lockfile
    regenerate_lockfile(maybe_manifest, maybe_lockfile)?;

    println!("Package installed successfully to wapm_modules!");
    Ok(())
}

fn create_package_dir<P: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P,
    namespace_dir: P2,
    fully_qualified_package_name: &str,
) -> Result<PathBuf, io::Error> {
    let mut package_dir = project_dir.as_ref().join("wapm_modules");
    package_dir.push(namespace_dir);
    package_dir.push(&fully_qualified_package_name);
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

    #[test]
    fn creates_package_directory() {
        let tmp_dir = tempdir::TempDir::new("install_package").unwrap();
        let expected_package_version = "0.1.2";
        let expected_package_name = "my_pkg";
        let expected_fully_qualified_package_name =
            fully_qualified_package_display_name(expected_package_name, expected_package_version);
        let tmp_dir_path = tmp_dir.path();
        let expected_package_directory = tmp_dir_path.join("wapm_modules/_/my_pkg@0.1.2");
        let actual_package_directory =
            create_package_dir(tmp_dir_path, "_", &expected_fully_qualified_package_name).unwrap();
        assert!(expected_package_directory.exists());
        assert_eq!(expected_package_directory, actual_package_directory);
    }
}
