use graphql_client::{QueryBody, Response};
use serde;
use std::env;
use std::string::ToString;
use thiserror::Error;
use crate::whoami_distro;
use url::Url;

#[cfg(not(target_os = "wasi"))]
use {
    crate::proxy,
    reqwest::{
        blocking::{multipart::Form, Client},
        header::USER_AGENT,
    },
};
// #[cfg(target_os = "wasi")]
// use {wasm_bus_reqwest::prelude::header::*, wasm_bus_reqwest::prelude::*};

#[derive(Debug, Error)]
enum GraphQLError {
    #[error("{message}")]
    Error { message: String },
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(not(target_os = "wasi"))]
pub fn execute_query_modifier<R, V, F>(registry: &Url, query: &QueryBody<V>, form_modifier: F) -> anyhow::Result<R>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
    F: FnOnce(Form) -> Form,
{
    let client = {
        let builder = Client::builder();

        #[cfg(not(target_os = "wasi"))]
        let builder = if let Some(proxy) = proxy::maybe_set_up_proxy()? {
            builder.proxy(proxy)
        } else {
            builder
        };
        builder.build()?
    };

    let registry_url = registry;

    let vars = serde_json::to_string(&query.variables).unwrap();

    let form = Form::new()
        .text("query", query.query.to_string())
        .text("operationName", query.operation_name.to_string())
        .text("variables", vars);

    let form = form_modifier(form);

    let user_agent = format!(
        "wapm/{} {} {}",
        VERSION,
        whoami::platform(),
        whoami_distro(),
    );

    let res = client
        .post(registry_url.clone())
        .multipart(form)
        .bearer_auth(
            env::var("WAPM_REGISTRY_TOKEN")
            .unwrap_or_default()
        )
        .header(USER_AGENT, user_agent)
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

#[cfg(not(target_os = "wasi"))]
pub fn execute_query<R, V>(registry: &Url, query: &QueryBody<V>) -> anyhow::Result<R>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
{
    execute_query_modifier(registry, query, |f| f)
}

#[cfg(target_os = "wasi")]
pub fn execute_query<R, V>(registry: &Url, query: &QueryBody<V>) -> anyhow::Result<R>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
{
    Err(anyhow::anyhow!("networking is not implemented on wasm32-wasi"))
}
