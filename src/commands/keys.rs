//! Subcommand to deal with keys for signing wapm packages

use crate::config::Config;
use crate::keys::*;
use prettytable::{format, Table};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum KeyOpt {
    #[structopt(name = "list")]
    /// List keys registered with wapm
    List(List),

    #[structopt(name = "register")]
    /// Register a key with wapm
    Register(Register),
}

/// Print the keys wapm knows about in a table
#[derive(StructOpt, Debug)]
pub struct List {
    #[structopt(long = "all", short = "a")]
    /// Show keys downloaded from WAPM too
    all: bool,
}

/// Adds a key to wapm
#[derive(StructOpt, Debug)]
pub struct Register {
    /// The location of the public key to add
    #[structopt(long = "public")]
    public_key: String,

    /// The location of the private key to add
    #[structopt(long = "private")]
    private_key: String,
}

pub fn keys(options: KeyOpt) -> Result<(), failure::Error> {
    let mut key_db = open_keys_db()?;
    match options {
        KeyOpt::List(List { all }) => {
            // query server?
            let keys = get_personal_keys_from_database(&key_db)?;
            if keys.is_empty() {
                println!("No personal keys found");
            } else {
                println!("{}", create_public_key_table(keys)?);
            }
        }
        KeyOpt::Register(Register {
            public_key,
            private_key,
        }) => {
            // mutate server
            add_personal_key_pair_to_database(
                &mut key_db,
                public_key.clone(),
                private_key.clone(),
            )?;
            println!("Key pair successfully added!")
        }
    }

    Ok(())
}

pub fn create_public_key_table(keys: Vec<PersonalKey>) -> Result<String, failure::Error> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row!["KEY", "ACTIVE", "DATE ADDED"]);
    for key in keys {
        table.add_row(row![
            key.public_key_value,
            key.active,
            time::strftime("%Y-%m-%d", &time::at(key.date_created))?
        ]);
    }
    Ok(format!("{}", table))
}
