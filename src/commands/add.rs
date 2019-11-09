//! Code pertaining to the `add` subcommand: it adds dependencies to
//! the manifest without installing

use crate::graphql::execute_query;
use graphql_client::*;

use crate::data::manifest::Manifest;
use structopt::StructOpt;

/// Options for the `add` subcommand
#[derive(StructOpt, Debug)]
pub struct AddOpt {
    packages: Vec<String>,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package_version.graphql",
    response_derives = "Debug"
)]
struct GetPackageVersionQuery;

#[derive(Debug, Fail)]
enum AddError {
    #[fail(display = "There were problems adding packages")]
    GenericError,
    #[fail(
        display = "Could not find a manifest in the current directory, try running `wapm init`"
    )]
    NoManifest,
    #[fail(display = "No packages listed to add")]
    ArgumentsRequired,
}

/// Run the add command
pub fn add(options: AddOpt) -> Result<(), failure::Error> {
    let mut error = false;
    let mut manifest: Manifest = {
        let cur_dir = std::env::current_dir()?;
        Manifest::find_in_directory(cur_dir).map_err(|_| AddError::NoManifest)?
    };

    if options.packages.is_empty() {
        return Err(AddError::ArgumentsRequired.into());
    }

    for (package_name, maybe_version) in options.packages.into_iter().map(|package_str| {
        if package_str.contains('@') {
            let mut p = package_str.split('@');
            let package_name = p.next().unwrap();
            let package_version = p.next().unwrap();
            (package_name.to_string(), Some(package_version.to_string()))
        } else {
            (package_str, None)
        }
    }) {
        let q = GetPackageVersionQuery::build_query(get_package_version_query::Variables {
            name: package_name.clone(),
            version: maybe_version.clone(),
        });
        let response: get_package_version_query::ResponseData = execute_query(&q)?;

        if let Some(pv) = response.package_version {
            info!("Adding {}@{}", &package_name, &pv.version);
            manifest.add_dependency(package_name, pv.version);
        } else {
            error = true;
            if let Some(ver) = maybe_version {
                error!("Package \"{}@{}\" was not found", &package_name, &ver);
            } else {
                error!("Package \"{}\" was not found", &package_name);
            }
        }
    }

    manifest.save()?;

    if error {
        Err(AddError::GenericError.into())
    } else {
        println!("Packages successfully added!");
        Ok(())
    }
}

#[cfg(feature = "integration_tests")]
impl AddOpt {
    pub fn new(packages: Vec<String>) -> Self {
        AddOpt { packages }
    }
}
