use crate::graphql::execute_query;
use std::fs;
use std::fs::File;
use std::io::copy;
use std::path::PathBuf;

use graphql_client::*;
use reqwest;

use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct AddOpt {
    #[structopt(parse(from_str))]
    package: String,
}

#[derive(Debug, Fail)]
enum AddError {
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

pub fn add(options: AddOpt) -> Result<(), failure::Error> {
    let name = options.package;
    let q = GetPackageQuery::build_query(get_package_query::Variables {
        name: name.to_string(),
    });
    let response: get_package_query::ResponseData = execute_query(&q)?;
    match response.package {
        Some(package) => {
            let last_version = package
                .last_version
                .ok_or(AddError::NoVersionsAvailable { name: name })?;
            println!(
                "Installing package {}@{}",
                package.name, last_version.version
            );
            let download_url = last_version.distribution.download_url;
            // println!("Downloading from url: {}", download_url);
            let mut response = reqwest::get(&download_url)?;
            let path_buf = PathBuf::from("./wapm_modules/");
            let mut package_file_location = path_buf.clone();
            fs::create_dir_all(path_buf)?;
            let package_file = &format!("{}.wasm", package.name);
            package_file_location.push(package_file);
            let mut dest = File::create(package_file_location)?;
            copy(&mut response, &mut dest)?;
            println!("Package added successfully to wapm_modules!")
        }
        None => return Err(AddError::PackageNotFound { name: name }.into()),
    };
    Ok(())
}
