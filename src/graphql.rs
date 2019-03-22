use failure;
use graphql_client::Response;
use reqwest::Client;
use serde;

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
    let mut res = client
        // .post("https://registry.wapm.dev/graphql")
        .post("http://localhost:8000/graphql")
        // .bearer_auth("d43c74c2d78dbf9f15ec45afb55cc4666b774b25")
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
