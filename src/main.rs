extern crate dotenv;
extern crate envy;
#[macro_use]
extern crate failure;
extern crate graphql_client;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate structopt;
#[macro_use]
extern crate prettytable;

use graphql_client::*;
use structopt::StructOpt;

type URI = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/whoami.graphql",
    response_derives = "Debug"
)]
struct WhoAmIQuery;

#[derive(StructOpt)]
struct Command {
    #[structopt(name = "repository")]
    whoami: bool,
}

// #[derive(Deserialize, Debug)]
// struct Env {
//     github_api_token: String,
// }

#[derive(Debug, Fail)]
enum GraphQLError {
    #[fail(display = "{}", message)]
    Error {
        message: String,
    }
}

fn execute_query<R, Q: serde::Serialize + ?Sized>(query: &Q) -> Result<R, failure::Error> where for<'de> R: serde::Deserialize<'de> {
    let client = reqwest::Client::new();
    let mut res = client
        .post("https://registry.wapm.dev/graphql")
        // .bearer_auth(config.github_api_token)
        .json(&query)
        .send()?;


    let response_body: Response<R> = res.json()?;

    if let Some(errors) = response_body.errors {
        let error_messages: Vec<String> = errors.into_iter().map(|err| err.message).collect();
        return Err(GraphQLError::Error { message: error_messages.join(", ") }.into())
    }

    Ok(response_body.data.expect("missing response data"))
}

fn main() -> Result<(), failure::Error> {
    // dotenv::dotenv().ok();
    env_logger::init();

    // let config: Env = envy::from_env()?;

    let args = Command::from_args();

    let q = WhoAmIQuery::build_query(who_am_i_query::Variables {});
    let response: who_am_i_query::ResponseData = execute_query(&q)?;
    println!("{:?}", response);


    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

}
