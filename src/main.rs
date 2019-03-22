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

use structopt::StructOpt;

mod commands;
mod graphql;

#[derive(StructOpt)]
enum Command {
    #[structopt(name = "whoami")]
    /// Prints the current user (if authed) in the stdout
    WhoAmI,
}


fn main() -> Result<(), failure::Error> {
    // dotenv::dotenv().ok();
    // env_logger::init();
    // let config: Env = envy::from_env()?;

    let args = Command::from_args();
    match args {
        Command::WhoAmI => {
            commands::whoami()
        }
    }
}
