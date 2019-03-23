use crate::config::Config;
use crate::graphql::execute_query_modifier;
use crate::manifest::Manifest;
use std::error::Error as StdError;
use std::fs;
use std::fs::File;
use std::io::copy;
use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::path::PathBuf;
use std::result::Result as StdResult;

use graphql_client::*;
use reqwest;

use structopt::StructOpt;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_package.graphql",
    response_derives = "Debug"
)]
struct PublishPackageMutation;

pub fn publish() -> Result<(), failure::Error> {
    let manifest = Manifest::new_from_path(None)?;
    // println!("Manifest target path {:?}", manifest.target_absolute_path());
    // let contents = manifest.get_target_contents()?;
    let target_path = manifest.target_absolute_path()?;
    // let mut file = File::open(manifest.)?;
    // manifest.get_target()
    let name = manifest.name;
    let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
        name: name.to_string(),
        version: manifest.version,
        description: manifest.description,
        license: manifest.license,
        readme: None,
        file_name: Some("module".to_string()),
    });
    let response: publish_package_mutation::ResponseData =
        execute_query_modifier(&q, |f| f.file("module", target_path).unwrap())?;
    Ok(())
}
