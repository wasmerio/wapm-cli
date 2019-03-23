use std::error::Error as StdError;
use std::io::{stdin, stdout};
use std::result::Result as StdResult;
use crate::graphql::execute_query;
use std::io::prelude::*;                                                           
use crate::config::Config;
use std::fs;
use std::path::PathBuf;
use std::io::copy;
use std::fs::File;

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
    let package_name = "abc";
    let q = PublishPackageMutation::build_query(publish_package_mutation::Variables {
        package_name: package_name.to_string(),
        file_name: Some("module".to_string())
    });
    let response: publish_package_mutation::ResponseData = execute_query(&q)?;
    Ok(())
}
