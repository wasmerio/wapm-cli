use failure;
use graphql_client::Response;
use reqwest::Client;
use serde;

use super::config::Config;

#[derive(Debug, Fail)]
enum GraphQLError {
    #[fail(display = "{}", message)]
    Error { message: String },
}

pub fn execute_query<R, Q: serde::Serialize + ?Sized>(query: &Q) -> Result<R, failure::Error>
where
    for<'de> R: serde::Deserialize<'de>,
{
    let client = Client::new();
    let config = Config::from_file();

    let registry_url = &config.registry.get_graphql_url();
    // println!("REGISTRY {}", registry_url);

    let mut res = client
        // .post("https://registry.wapm.dev/graphql")
        .post(registry_url)
        .bearer_auth(&config.registry.token.unwrap_or("".to_string()))
        .json(&query)
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
