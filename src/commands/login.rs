use crate::config::{Config, UpdateRegistry};
use crate::graphql::execute_query;
use rpassword_wasi as rpassword;
use std::io::prelude::*;
use std::io::{stdin, stdout};
use structopt::StructOpt;

use graphql_client::*;

#[derive(StructOpt, Debug)]
pub struct LoginOpt {
    /// Provide the token
    token: Option<String>,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/login.graphql",
    response_derives = "Debug"
)]
struct LoginMutation;

pub fn login(login_options: LoginOpt) -> anyhow::Result<()> {
    if let Some(token) = login_options.token {
        let mut config = Config::from_file()?;
        config.registry.set_login_token_for_registry(
            &config.registry.get_current_registry(),
            &token,
            UpdateRegistry::Update,
        );
        config.save()?;
        println!("Login for WAPM saved");
        return Ok(());
    }

    print!("Username: ");
    stdout().flush().ok().expect("Could not flush stdout");

    let buffer = &mut String::new();
    stdin().read_line(buffer)?;
    let username = buffer.trim_end();

    let password = rpassword::prompt_password("Password: ").expect("Can't get password");

    let q = LoginMutation::build_query(login_mutation::Variables {
        username: username.to_string(),
        password: password.to_string(),
    });
    let response: login_mutation::ResponseData = execute_query(&q)?;
    let token = match response.token_auth {
        Some(token_auth) => Some(token_auth.refresh_token),
        None => None,
    };
    if let Some(token) = token {
        // Save the token
        let mut config = Config::from_file()?;
        config.registry.set_login_token_for_registry(
            &config.registry.get_current_registry(),
            &token,
            UpdateRegistry::Update,
        );
        config.save()?;
    }
    Ok(())
}
