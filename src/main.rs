// extern crate dotenv;
// extern crate envy;
#[macro_use]
extern crate failure;
extern crate graphql_client;
#[macro_use]
// extern crate log;
// extern crate env_logger;
extern crate reqwest;
extern crate serde;
// extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate structopt;
// #[macro_use]
// extern crate prettytable;
extern crate rpassword;
extern crate toml;

use structopt::StructOpt;

mod commands;
mod graphql;
mod config;

#[derive(StructOpt, Debug)]
enum Command {
    #[structopt(name = "whoami")]
    /// Prints the current user (if authed) in the stdout
    WhoAmI,

    #[structopt(name = "login")]
    /// Logins into wapm, saving the token locally for future commands
    Login,

    #[structopt(name = "logout")]
    /// Remove the token for the registry
    Logout,

    #[structopt(name = "config")]
    /// Config related subcommands
    Config(commands::ConfigOpt),
}


fn main() -> Result<(), failure::Error> {
    // dotenv::dotenv().ok();
    // env_logger::init();
    // let config: Env = envy::from_env()?;

    let args = Command::from_args();
    match args {
        Command::WhoAmI => commands::whoami(),
        Command::Login => commands::login(),
        Command::Logout => commands::logout(),
        Command::Config(config) => commands::config(config),
    }
}
