use crate::graphql::execute_query;
use std::fs::File;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

use graphql_client::*;
use reqwest;

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
    let fully_qualified_package_name =
        fully_qualified_package_display_name(&package.name, &last_version.version);
    println!("Installing package {}", fully_qualified_package_name);
    let current_dir = env::current_dir()?;
    let package_dir = create_module_dir(&current_dir, &fully_qualified_package_name)?;
    let download_url = last_version.distribution.download_url;
    let mut response = reqwest::get(&download_url)?;
    let package_file_path = package_dir.join(&format!("{}.wasm", package.name));
    let mut dest = File::create(package_file_path)?;
    copy(&mut response, &mut dest)?;
    println!("Package installed successfully to wapm_modules!");
    Ok(())
}

fn create_module_dir<P: AsRef<Path>>(
    project_dir: P,
    fully_qualified_package_name: &str,
) -> Result<PathBuf, io::Error> {
    let mut package_dir = project_dir.as_ref().join("wapm_modules");
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
    use crate::commands::install::{create_module_dir, fully_qualified_package_display_name};

    #[test]
    fn creates_package_directory() {
        let tmp_dir = tempdir::TempDir::new("install_package").unwrap();
        let expected_package_version = "0.1.2";
        let expected_package_name = "my_pkg";
        let expected_fully_qualified_package_name =
            fully_qualified_package_display_name(expected_package_name, expected_package_version);
        let tmp_dir_path = tmp_dir.path();
        let expected_package_directory = tmp_dir_path.join("wapm_modules/my_pkg@0.1.2");
        let actual_package_directory =
            create_module_dir(tmp_dir_path, &expected_fully_qualified_package_name).unwrap();
        assert!(expected_package_directory.exists());
        assert_eq!(expected_package_directory, actual_package_directory);
    }
}
