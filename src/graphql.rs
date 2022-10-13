use graphql_client::{QueryBody, Response};
use serde;
use std::env;
use std::string::ToString;
use thiserror::Error;
#[cfg(not(target_os = "wasi"))]
use {
    crate::proxy,
    reqwest::{
        blocking::{multipart::Form, Client},
        header::USER_AGENT,
    },
};
#[cfg(target_os = "wasi")]
use {wasm_bus_reqwest::prelude::header::*, wasm_bus_reqwest::prelude::*};

use crate::util::whoami_distro;

use super::config::Config;

#[derive(Debug, Error)]
enum GraphQLError {
    #[error("{message}")]
    Error { message: String },
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub type DateTime = String;

pub fn execute_query_modifier<R, V, F>(query: &QueryBody<V>, form_modifier: F) -> anyhow::Result<R>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
    F: FnOnce(Form) -> Form,
{
    let config = Config::from_file()?;
    let registry_url = &config.registry.get_graphql_url();
    execute_query_modifier_inner(registry_url, query, form_modifier)
}

pub fn execute_query_modifier_inner<R, V, F>(
    registry_url: &str,
    query: &QueryBody<V>,
    form_modifier: F,
) -> anyhow::Result<R>
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
    let config = Config::from_file()?;

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
        .post(registry_url)
        .multipart(form)
        .bearer_auth(
            env::var("WAPM_REGISTRY_TOKEN").unwrap_or(
                config
                    .registry
                    .get_login_token_for_registry(&config.registry.get_current_registry())
                    .unwrap_or_else(|| "".to_string()),
            ),
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

pub fn execute_query<R, V>(query: &QueryBody<V>) -> anyhow::Result<R>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
{
    execute_query_modifier(query, |f| f)
}

pub fn execute_query_custom_registry<R, V>(
    registry_url: &str,
    query: &QueryBody<V>,
) -> anyhow::Result<R>
where
    for<'de> R: serde::Deserialize<'de>,
    V: serde::Serialize,
{
    execute_query_modifier_inner(registry_url, query, |f| f)
}
