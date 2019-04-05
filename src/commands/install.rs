use crate::graphql::execute_query;
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use graphql_client::*;
use reqwest;

use crate::dependency_resolver::RegistryResolver;
use crate::lockfile::Lockfile;
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

    #[fail(display = "The package name is invalid: {}", _0)]
    InvalidPackageName(String),
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package.graphql",
    response_derives = "Debug"
)]
struct GetPackageQuery;

pub fn install(options: InstallOpt) -> Result<(), failure::Error> {
    let name = options.package;
    println!("install package {}", name);
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

    let package_file_path = package_dir.join(&format!("{}.wasm", pkg_name));
    let mut dest = File::create(package_file_path)?;
    copy(&mut response, &mut dest)?;

    let manifest_file_path = current_dir.join(MANIFEST_FILE_NAME);
    // update wapm.toml
    match Manifest::open(&manifest_file_path) {
        Ok(mut manifest) => {
            manifest.add_dependency(&package.name, &last_version.version);
            // construct lockfile
            let resolver = RegistryResolver;
            let existing_lockfile = Lockfile::open(&manifest.base_directory_path).ok();
            let mut lockfile =
                Lockfile::new_from_manifest(&manifest, existing_lockfile, &resolver)?;
            match lockfile
                .modules
                .get_mut(&format!("{} {}", package.name, last_version.version))
            {
                Some(lockfile_module) => lockfile_module.resolved = download_url,
                _ => {}
            };
            // write the manifest
            manifest.save()?;
            // write the lockfile
            lockfile.save(&manifest.base_directory_path)?;
        }
        Err(_e) => {
            println!("didn't open manifest: {:?}", _e);
            // TODO: implement new_from_install
            // Install dependency with no manifest
            // let resolver = RegistryResolver;
            // let lockfile = Lockfile::new_from_install(&manifest, &resolver)?;
        }
    }

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

#[inline]
fn get_package_namespace_and_name(package_name: &str) -> Result<(&str, &str), failure::Error> {
    let split: Vec<&str> = package_name.split('/').collect();
    match &split[..] {
        [namespace, name] => Ok((*namespace, *name)),
        _ => Err(InstallError::InvalidPackageName(package_name.to_string()).into()),
    }
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
