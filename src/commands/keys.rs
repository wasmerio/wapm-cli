//! Subcommand to deal with keys for signing wapm packages

use crate::database;
use crate::graphql::{self, DateTime};
use crate::keys::*;
use crate::util;

use graphql_client::*;
use prettytable::{format, Table};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum KeyOpt {
    #[structopt(name = "list")]
    /// List keys registered with wapm
    List(List),

    #[structopt(name = "register")]
    /// Register a personal key pair with wapm
    Register(Register),

    #[structopt(name = "import")]
    /// Import a public key from somewhere
    Import(Import),

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

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/publish_public_key.graphql",
    response_derives = "Debug"
)]
struct PublishPublicKeyMutation;

/// Deletes a key to wapm
#[derive(StructOpt, Debug)]
pub struct Delete {
    /// The identifier of the public key
    public_key_id: String,
}

/// Import a public key from somewhere else
#[derive(StructOpt, Debug)]
pub struct Import {
    #[structopt(long = "user-name")]
    user_name: String,
    public_key_value: String,
}

pub fn keys(options: KeyOpt) -> Result<(), failure::Error> {
    let mut key_db = database::open_db()?;
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
            let (pk_id, pk_v, tx) = add_personal_key_pair_to_database(
                &mut key_db,
                public_key_location.clone(),
                private_key_location.clone(),
            )?;
            let q = PublishPublicKeyMutation::build_query(publish_public_key_mutation::Variables {
                key_id: pk_id.clone(),
                key: pk_v,
                verifying_signature_id: None,
            });
            let response_or_err: Result<publish_public_key_mutation::ResponseData, _> =
                graphql::execute_query(&q);
            match response_or_err {
                Ok(_) => {
                    tx.commit().map_err(|e| {
                        format_err!(
                            "Failed to store key pair in local database: {}",
                            e.to_string()
                        )
                    })?;
                    println!("Key pair successfully added!")
                }
                Err(e) => {
                    error!("Failed to upload public key to server: {}", e);
                    #[cfg(feature = "telemetry")]
                    sentry::integrations::failure::capture_error(&e);
                }
            };
        }
        KeyOpt::Delete(Delete { public_key_id }) => {
            let full_public_key =
                get_full_personal_public_key_by_id(&key_db, public_key_id.clone())?;
            warn!(
                "You are about to delete the key pair associated with {:?} from wapm.\nThis cannot be undone.",
                &full_public_key
            );
            let user_confirmed_key_deletion = util::prompt_user_for_yes(
                "Please confirm that you want to permanently delete this key pair from wapm:",
            )?;
            if user_confirmed_key_deletion {
                delete_key_pair(&mut key_db, full_public_key)?;
            } else {
                println!("Aborting");
            }
        }
        KeyOpt::Import(Import {
            user_name,
            public_key_value,
        }) => {
            let user_name = user_name.trim().to_string();
            let (pk_id, pkv) = normalize_public_key(public_key_value)?;
            import_public_key(&mut key_db, &pk_id, &pkv, user_name)?;
        }
    }

    Ok(())
}

pub fn create_personal_key_table(keys: Vec<PersonalKey>) -> Result<String, failure::Error> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row![
        "TAG",
        "ACTIVE",
        "KEY",
        "PRIVATE KEY LOCATION",
        "DATE ADDED"
    ]);
    for key in keys {
        table.add_row(row![
            key.public_key_id,
            key.active,
            key.public_key_value,
            key.private_key_location.unwrap_or("None".to_string()),
            time::strftime("%Y-%m-%d", &time::at(key.date_created))?
        ]);
    }
    Ok(format!("{}", table))
}

pub fn create_wapm_public_key_table(keys: Vec<WapmPublicKey>) -> Result<String, failure::Error> {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row!["USER", "TAG", "KEY", "DATE ADDED"]);
    for key in keys {
        table.add_row(row![
            key.user_name,
            key.public_key_id,
            key.public_key_value,
            time::strftime("%Y-%m-%d", &time::at(key.date_created))?
        ]);
    }
    Ok(format!("{}", table))
}
