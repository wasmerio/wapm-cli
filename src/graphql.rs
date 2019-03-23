use failure;
use graphql_client::{QueryBody, Response};
use reqwest::Client;
use serde;
use std::string::ToString;

use super::config::Config;

#[derive(Debug, Fail)]
enum GraphQLError {
    #[fail(display = "{}", message)]
    Error { message: String },
}

pub fn execute_query_modifier<R, V, F>(
    query: &QueryBody<V>,
    form_modifier: F,
) -> Result<R, failure::Error>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
    F: FnOnce(reqwest::multipart::Form) -> reqwest::multipart::Form,
{
    let client = Client::new();
    let config = Config::from_file();

    let registry_url = &config.registry.get_graphql_url();
    // println!("REGISTRY {}", registry_url);
    // type T = serde::Serialize;
    // let vars = serde_json::to_value(query.variables);
    let vars = serde_json::to_string(&query.variables).unwrap();

    let form = reqwest::multipart::Form::new()
        .text("query", query.query.to_string())
        .text("operationName", query.operation_name.to_string())
        .text("variables", vars);
    let form = form_modifier(form);

    let mut res = client
        // .post("https://registry.wapm.dev/graphql")
        .post(registry_url)
        .multipart(form)
        .bearer_auth(&config.registry.token.unwrap_or("".to_string()))
        // .json(&query)
        .send()?;

    let response_body: Response<R> = res.json()?;

    if let Some(errors) = response_body.errors {
        let error_messages: Vec<String> = errors.into_iter().map(|err| err.message).collect();
        return Err(GraphQLError::Error {
            message: error_messages.join(", "),
        }
        .into());
    }

    Ok(response_body.data.expect("missing response data"))
}

pub fn execute_query<R, V>(query: &QueryBody<V>) -> Result<R, failure::Error>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
{
    execute_query_modifier(query, |f| f)
}
