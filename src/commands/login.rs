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
    /// Username
    user: Option<String>,
    /// Password
    password: Option<String>,
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
        if let Some(s) = crate::util::get_username().ok().and_then(|o| o) {
            println!("Login for WAPM user {:?} saved", s);
        } else {
            println!("Login for WAPM user saved");
        }
        return Ok(());
    }

    let username = if let Some(u) = login_options.user.as_ref() {
        u.to_string()
    } else {
        print!("Username: ");
        stdout().flush().ok().expect("Could not flush stdout");
    
        let buffer = &mut String::new();
        stdin().read_line(buffer)?;
        buffer.trim_end().to_string()
    };

    let password = if let Some(p) = login_options.password.as_ref() {
        p.to_string()
    } else {
        rpassword::prompt_password("Password: ")
        .expect("Can't get password")
    };

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
        if let Some(u) = crate::util::get_username().ok().and_then(|o| o) {
            println!("Successfully logged into registry {:?} as user {:?}",  config.registry.get_current_registry(), u);
        }
    }
    Ok(())
}
