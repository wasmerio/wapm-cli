use crate::graphql::execute_query;

use graphql_client::*;

use crate::dataflow;
use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InstallOpt {
    packages: Vec<String>,
}

#[derive(Debug, Fail)]
enum InstallError {
    #[fail(display = "Package not found in the registry: {}", name)]
    PackageNotFound { name: String },

    #[fail(display = "No package versions available for package {}", name)]
    NoVersionsAvailable { name: String },

    #[fail(display = "Failed to install packages. {}", _0)]
    CannotRegenLockFile(dataflow::Error),

    #[fail(display = "Failed to install packages in manifest. {}", _0)]
    FailureInstallingPackages(dataflow::Error),
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package.graphql",
    response_derives = "Debug"
)]
struct GetPackageQuery;

pub fn install(options: InstallOpt) -> Result<(), failure::Error> {
    let current_directory = env::current_dir()?;
    if !options.packages.is_empty() {
        let mut packages = vec![];
        for name in options.packages {
            let name_with_version: Vec<&str> = name.split("@").collect();

            if name_with_version.len() > 1 {
                let package_name = name_with_version[0].to_string();
                let package_version = name_with_version[1].to_string();
                packages.push((package_name, package_version));
            } else {
                let q = GetPackageQuery::build_query(get_package_query::Variables {
                    name: name.to_string(),
                });
                let response: get_package_query::ResponseData = execute_query(&q)?;
                let package = response
                    .package
                    .ok_or(InstallError::PackageNotFound { name: name.clone() })?;
                let last_version = package
                    .last_version
                    .ok_or(InstallError::NoVersionsAvailable { name: name.clone() })?;
                let package_name = package.name.clone();
                let package_version = last_version.version.clone();
                packages.push((package_name, package_version));
            }
        }

        let installed_packages: Vec<(&str, &str)> = packages
            .iter()
            .map(|(s1, s2)| (s1.as_str(), s2.as_str()))
            .collect();

        dataflow::update(installed_packages, &current_directory)
            .map_err(|err| InstallError::CannotRegenLockFile(err))?;

        println!("Package installed successfully to wapm_packages!");
    } else {
        let added_packages = vec![];
        dataflow::update(added_packages, &current_directory)
            .map_err(|err| InstallError::FailureInstallingPackages(err))?;
        println!("Packages installed to wapm_packages!");
    }
    Ok(())
}
