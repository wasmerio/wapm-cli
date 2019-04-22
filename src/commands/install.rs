use crate::graphql::execute_query;

use graphql_client::*;

use crate::dataflow;
use crate::util::get_package_namespace_and_name;
use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InstallOpt {
    #[structopt(parse(from_str))]
    package: Option<String>,
}

#[derive(Debug, Fail)]
enum InstallError {
    #[fail(display = "Package not found in the registry: {}", name)]
    PackageNotFound { name: String },

    #[fail(display = "No package versions available for package {}", name)]
    NoVersionsAvailable { name: String },

    #[fail(display = "Failed to install {}. {}", _0, _1)]
    CannotRegenLockFile(String, dataflow::Error),

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
    match options.package {
        Some(name) => {
            let q = GetPackageQuery::build_query(get_package_query::Variables {
                name: name.to_string(),
            });
            let response: get_package_query::ResponseData = execute_query(&q)?;
            let package = response
                .package
                .ok_or(InstallError::PackageNotFound { name: name.clone() })?;
            let last_version = package
                .last_version
                .ok_or(InstallError::NoVersionsAvailable { name })?;

            let (namespace, pkg_name) = get_package_namespace_and_name(&package.name)?;

            let display_package_name: String = if namespace == "_" {
                pkg_name.to_string()
            } else {
                package.name.clone()
            };

            println!("installing...");
            let added_packages = vec![(package.name.as_str(), last_version.version.as_str())];
            dataflow::update(added_packages.clone(), &current_directory)
                .map_err(|err| InstallError::CannotRegenLockFile(display_package_name, err))?;

            println!("Package installed successfully to wapm_packages!");
        }
        None => {
            let added_packages = vec![];
            dataflow::update(added_packages, current_directory)
                .map_err(|err| InstallError::FailureInstallingPackages(err))?;
            println!("Packages installed to wapm_packages!");
        }
    };
    Ok(())
}
