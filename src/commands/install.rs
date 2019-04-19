use crate::graphql::execute_query;

use graphql_client::*;

use crate::lock::{get_package_namespace_and_name, regenerate_lockfile};
use structopt::StructOpt;
use crate::bonjour;

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
    CannotRegenLockFile(String, failure::Error),

    #[fail(display = "Failed to install packages in manifest. {}", _0)]
    FailureInstallingPackages(failure::Error),
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package.graphql",
    response_derives = "Debug"
)]
struct GetPackageQuery;

pub fn install(options: InstallOpt) -> Result<(), failure::Error> {
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
            regenerate_lockfile(vec![(&package.name, &last_version.version)])
                .map_err(|err| InstallError::CannotRegenLockFile(display_package_name, err))?;

            // insert record into manifest file

            println!("Package installed successfully to wapm_packages!");
        }
        None => {
            regenerate_lockfile(vec![])
                .map_err(|err| InstallError::FailureInstallingPackages(err))?;
            println!("Packages installed to wapm_packages!");
        }
    };
    Ok(())
}
