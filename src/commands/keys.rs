//! Subcommand to deal with keys for signing wapm packages

use crate::keys::*;
use prettytable::{format, Table};
use std::io::Write;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum KeyOpt {
    #[structopt(name = "list")]
    /// List keys registered with wapm
    List(List),

    #[structopt(name = "register")]
    /// Register a key with wapm
    Register(Register),

    #[structopt(name = "delete")]
    /// Delete a keypair from wapm
    Delete(Delete),
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
    public_key_location: String,

    /// The location of the private key to add
    #[structopt(long = "private")]
    private_key_location: String,
}

/// Deletes a key to wapm
#[derive(StructOpt, Debug)]
pub struct Delete {
    /// The identifier of the public key
    public_key: String,
}

pub fn keys(options: KeyOpt) -> Result<(), failure::Error> {
    let mut key_db = open_keys_db()?;
    match options {
        KeyOpt::List(List { all }) => {
            // query server?
            let keys = get_personal_keys_from_database(&key_db)?;
            if all {
                let wapm_public_keys = get_wapm_public_keys_from_database(&key_db)?;
                match (keys.is_empty(), wapm_public_keys.is_empty()) {
                    (true, true) => println!("No keys found"),
                    (true, false) => {
                        println!("{}", create_wapm_public_key_table(wapm_public_keys)?);
                    }
                    (false, true) => {
                        println!("{}", create_personal_key_table(keys)?);
                    }
                    (false, false) => {
                        println!("PERSONAL KEYS:\n{}", create_personal_key_table(keys)?);
                        println!(
                            "\nWAPM PUBLIC KEYS:\n{}",
                            create_wapm_public_key_table(wapm_public_keys)?
                        );
                    }
                }
            } else {
                if keys.is_empty() {
                    println!("No personal keys found");
                } else {
                    println!("{}", create_personal_key_table(keys)?);
                }
            }
        }
        KeyOpt::Register(Register {
            public_key_location,
            private_key_location,
        }) => {
            // mutate server
            add_personal_key_pair_to_database(
                &mut key_db,
                public_key_location.clone(),
                private_key_location.clone(),
            )?;
            println!("Key pair successfully added!")
        }
        KeyOpt::Delete(Delete { public_key }) => {
            let full_public_key =
                get_full_personal_public_key_by_pattern(&key_db, public_key.clone())?;
            warn!(
                "You are about to delete the key pair associated with {:?} from wapm.\nThis cannot be undone.",
                &full_public_key
            );
            print!("Please confirm that you want to permanently delete this key pair from wapm:\n[y/n] ");
            std::io::stdout().flush()?;
            let mut input_str = String::new();
            std::io::stdin().read_line(&mut input_str)?;
            match input_str.to_lowercase().trim_end() {
                "yes" | "y" => {
                    delete_key_pair(&mut key_db, full_public_key)?;
                }
                _ => {
                    println!("Aborting");
                }
            }
        }
    }

    Ok(())
}

pub fn create_personal_key_table(keys: Vec<PersonalKey>) -> Result<String, failure::Error> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row!["KEY", "ACTIVE", "PRIVATE KEY LOCATION", "DATE ADDED"]);
    for key in keys {
        table.add_row(row![
            key.public_key_value,
            key.active,
            key.private_key_location.unwrap_or("None".to_string()),
            time::strftime("%Y-%m-%d", &time::at(key.date_created))?
        ]);
    }
    Ok(format!("{}", table))
}

pub fn create_wapm_public_key_table(keys: Vec<WapmPublicKey>) -> Result<String, failure::Error> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row!["USER", "KEY", "DATE ADDED"]);
    for key in keys {
        table.add_row(row![
            key.user_name,
            key.public_key_value,
            time::strftime("%Y-%m-%d", &time::at(key.date_created))?
        ]);
    }
    Ok(format!("{}", table))
}
