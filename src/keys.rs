//! Functions for dealing with package verification keys
#![cfg_attr(
    not(feature = "full"),
    allow(dead_code, unused_imports, unused_variables)
)]
use crate::constants::*;
#[cfg(feature = "full")]
use crate::database::*;
use crate::sql;
use crate::util;
#[cfg(feature = "full")]
use rusqlite::{params, Connection, TransactionBehavior};
use std::{fs, path::PathBuf};
use thiserror::Error;
use time::Timespec;

const MINISIGN_TAG_LENGTH: usize = 16;

/// Information about one of the user's keys
#[derive(Debug)]
pub struct PersonalKey {
    /// Flag saying if the key will be used (there can only be one active key at a time)
    pub active: bool,
    /// The public key's tag. Used to identify the key pair
    pub public_key_id: String,
    /// The raw value of the public key in base64
    pub public_key_value: String,
    /// The location in the file system of the private key
    pub private_key_location: Option<String>,
    /// The type of private/public key this is
    pub key_type_identifier: String,
    /// The time at which the key was registered with wapm
    pub date_created: Timespec,
}

/// Information about a public key downloaded from wapm
#[derive(Debug)]
pub struct WapmPublicKey {
    /// The user whose key this is
    pub user_name: String,
    /// The public key's tag. Used to identify the key pair
    pub public_key_id: String,
    /// The raw value of the public key in base64
    pub public_key_value: String,
    /// The type of private/public key this is
    pub key_type_identifier: String,
    /// The time at which the key was seen by the user's instance of wapm
    pub date_created: Timespec,
}

/// Information about a package signature downloaded from the registry
#[derive(Debug, Clone)]
pub struct WapmPackageSignature {
    pub public_key_id: String,
    pub public_key: String,
    pub signature_data: String,
    pub date_created: Timespec,
    pub revoked: bool,
    pub owner: String,
}

/// Gets the user's keys from the database
#[cfg(feature = "full")]
pub fn get_personal_keys_from_database(conn: &Connection) -> anyhow::Result<Vec<PersonalKey>> {
    let mut stmt = conn.prepare(sql::GET_PERSONAL_KEYS)?;

    let result = stmt.query_map(params![], |row| {
        Ok(PersonalKey {
            active: row.get(0)?,
            public_key_value: row.get(1)?,
            private_key_location: row.get(2)?,
            date_created: {
                let time_str: String = row.get(3)?;
                time::strptime(&time_str, RFC3339_FORMAT_STRING)
                    .unwrap_or_else(|_| panic!("Failed to parse time string {}", &time_str))
                    .to_timespec()
            },
            key_type_identifier: row.get(4)?,
            public_key_id: row.get(5)?,
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// Gets all public keys the user has seen from WAPM from the database
#[cfg(feature = "full")]
pub fn get_wapm_public_keys_from_database(conn: &Connection) -> anyhow::Result<Vec<WapmPublicKey>> {
    let mut stmt = conn.prepare(sql::GET_WAPM_PUBLIC_KEYS)?;
    let result = stmt.query_map(params![], |row| {
        Ok(WapmPublicKey {
            user_name: row.get(0)?,
            public_key_value: row.get(1)?,
            date_created: {
                let time_str: String = row.get(2)?;
                time::strptime(&time_str, RFC3339_FORMAT_STRING)
                    .unwrap_or_else(|_| panic!("Failed to parse time string {}", &time_str))
                    .to_timespec()
            },
            key_type_identifier: row.get(3)?,
            public_key_id: row.get(4)?,
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// Get the public key in base64 from its id from the local database
#[cfg(feature = "full")]
pub fn get_full_personal_public_key_by_id(
    conn: &Connection,
    public_key_id: String,
) -> anyhow::Result<String> {
    let mut stmt =
        conn.prepare(
            "SELECT public_key_value FROM personal_keys WHERE public_key_id = (?1) ORDER BY date_added LIMIT 1",
        )?;
    let result = stmt
        .query_row(params![public_key_id], |row| row.get(0))
        .map_err(|_| anyhow!("No public key matching pattern {} found", &public_key_id))?;

    Ok(result)
}

#[cfg(feature = "full")]
pub fn get_active_personal_key(conn: &Connection) -> anyhow::Result<PersonalKey> {
    let mut stmt = conn.prepare(
        "SELECT active, public_key_value, private_key_location, date_added, key_type_identifier, public_key_id FROM personal_keys 
         where active = 1",
    )?;

    let result = stmt
        .query_map(params![], |row| {
            Ok(PersonalKey {
                active: row.get(0)?,
                public_key_value: row.get(1)?,
                private_key_location: row.get(2)?,
                date_created: {
                    let time_str: String = row.get(3)?;
                    time::strptime(&time_str, RFC3339_FORMAT_STRING)
                        .unwrap_or_else(|_| panic!("Failed to parse time string {}", &time_str))
                        .to_timespec()
                },
                key_type_identifier: row.get(4)?,
                public_key_id: row.get(5)?,
            })
        })?
        .next();

    if let Some(res) = result {
        Ok(res?)
    } else {
        Err(anyhow!("No active key found"))
    }
}

#[cfg(feature = "full")]
pub fn delete_key_pair(conn: &mut Connection, public_key: String) -> anyhow::Result<()> {
    conn.execute(sql::DELETE_PERSONAL_KEY_PAIR, params![public_key])?;
    Ok(())
}

/// This function takes the raw output from Minisign and returns the key's tag
/// and the key's value in base64
#[cfg(feature = "full")]
pub fn normalize_public_key(pk: String) -> anyhow::Result<(String, String)> {
    let mut lines = pk.lines();
    let first_line = lines
        .next()
        .ok_or_else(|| anyhow!("Empty public key value"))?;
    let second_line = lines
        .next()
        .ok_or_else(|| anyhow!("Public key value must have two lines"))?;

    let tag = first_line
        .trim()
        .chars()
        .rev()
        .take(MINISIGN_TAG_LENGTH)
        .collect::<Vec<char>>()
        .iter()
        .rev()
        .filter(|c| !c.is_whitespace())
        .collect();

    Ok((tag, second_line.to_string()))
}

/// Adds a public/private key pair to the database (storing the public key directly
/// and a path to a file containing the private key)
/// Returns the public key ID and the public key value on success
#[cfg(feature = "full")]
pub fn add_personal_key_pair_to_database(
    conn: &mut Connection,
    public_key_location: String,
    private_key_location: String,
) -> anyhow::Result<(String, String, rusqlite::Transaction)> {
    let (public_key_id, public_key_value) = normalize_public_key(
        fs::read_to_string(&public_key_location)
            .map_err(|e| anyhow!("Could not read public key: {}", e))?,
    )?;
    info!("Adding public key {:?} to local database", public_key_id);
    let private_key_path = PathBuf::from(&private_key_location).canonicalize()?;
    let private_key_location = private_key_path.to_string_lossy().to_string();
    {
        if !private_key_path.exists() {
            error!(
                "Private key file not found at path: {}",
                &private_key_location
            );
            if !util::prompt_user_for_yes("Would you like to add the key anyway?")? {
                return Err(anyhow!(
                    "Private key file not found at path: {}",
                    &private_key_location
                ));
            }
        }
    }

    let time_string = get_current_time_in_format().expect("Could not get the current time");

    // fail if we already have the key
    {
        let mut key_check = conn.prepare(sql::PERSONAL_PUBLIC_KEY_VALUE_EXISTENCE_CHECK)?;
        let result =
            key_check.query_map(params![public_key_id, public_key_value], |row| row.get(0))?;

        if let [existing_key] = &result.collect::<Result<Vec<String>, _>>()?[..] {
            return Err(PersonalKeyError::PublicKeyAlreadyExists(existing_key.to_string()).into());
        }

        // check the private key path too
        let mut private_key_check =
            conn.prepare(sql::PERSONAL_PRIVATE_KEY_VALUE_EXISTENCE_CHECK)?;
        let result = private_key_check.query_map(params![private_key_location], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        if let [(_, existing_public_key)] =
            &result.collect::<Result<Vec<(Option<String>, String)>, _>>()?[..]
        {
            return Err(PersonalKeyError::PrivateKeyAlreadyRegistered(
                private_key_location,
                existing_public_key.to_string(),
            )
            .into());
        }
    }

    // transact the now validated key pair
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    // deactivate all keys
    tx.execute("UPDATE personal_keys SET active = 0", params![])?;
    // insert the key, and activate it
    tx.execute(
        sql::INSERT_AND_ACTIVATE_PERSONAL_KEY_PAIR,
        params![
            public_key_id,
            public_key_value,
            "1",
            private_key_location,
            time_string,
            "minisign"
        ],
    )?;
    Ok((public_key_id, public_key_value, tx))
}

/// Parses a public key out of the given string and adds it to the database of
/// trusted keys associated with the given user
#[cfg(feature = "full")]
pub fn import_public_key(
    conn: &mut Connection,
    public_key_id: &str,
    public_key_value: &str,
    user_name: String,
) -> anyhow::Result<()> {
    // fail if we already have the key
    {
        let mut key_check = conn.prepare(sql::WAPM_PUBLIC_KEY_EXISTENCE_CHECK)?;
        let result = key_check.query_map(params![public_key_id, public_key_value], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        if let [(user_name, existing_key)] =
            &result.collect::<Result<Vec<(String, String)>, _>>()?[..]
        {
            return Err(WapmPublicKeyError::PublicKeyAlreadyExists(
                existing_key.to_string(),
                user_name.to_string(),
            )
            .into());
        }
    }

    // transact the new key
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    tx.execute(sql::INSERT_USER, params![user_name])?;

    let time_string = get_current_time_in_format().expect("Could not get current time");

    info!(
        "Importing key {:?} for user {:?}",
        &public_key_id, &user_name
    );
    tx.execute(
        sql::INSERT_WAPM_PUBLIC_KEY,
        params![
            user_name,
            public_key_id,
            public_key_value,
            "minisign",
            time_string
        ],
    )?;

    tx.commit()?;
    Ok(())
}

#[cfg(feature = "full")]
pub fn get_latest_public_key_for_user(
    conn: &Connection,
    user_name: &str,
) -> anyhow::Result<Option<WapmPublicKey>> {
    let mut stmt = conn.prepare(sql::GET_LATEST_PUBLIC_KEY_FOR_USER)?;

    match stmt.query_row(params![user_name], |row| {
        Ok(Some(WapmPublicKey {
            user_name: user_name.to_string(),
            public_key_id: row.get(0)?,
            public_key_value: row.get(1)?,
            date_created: {
                let time_str: String = row.get(2)?;
                time::strptime(&time_str, RFC3339_FORMAT_STRING)
                    .unwrap_or_else(|_| panic!("Failed to parse time string {}", &time_str))
                    .to_timespec()
            },
            key_type_identifier: row.get(3)?,
        }))
    }) {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow!("Internal database error: {}", e)),
    }
}

/*pub fn validate_key_history_and_return_latest_key(
    conn: &Connection,
    user_name: String,
    key_history: Vec<(String, Option<String>)>,
) -> anyhow::Result<WapmPublicKey> {
    let mut stmt = conn.prepare(
        "SELECT public_key_value
FROM wapm_public_keys
JOIN wapm_users wu ON user_key = wu.id
WHERE public_key_id = (?1)
  AND wu.name = (?2)",
    )?;

    for (key, signature) in key_history.iter().rev() {
        let (key_id, key_val) = normalize_public_key(key.clone());
        let result = stmt
            .query_row(params![key_id, user_name], |row| Ok(row.get(0)?))
            .collect::<Result<_, _>>();
        if let Ok(pkv) = result {
            if pkv != key_val {
                panic!("Critical error: key ID collision detected: {:?} and {:?} do not match but have the same key ID: {:?}.
\nThis may be due to invalid data in your local wapm database. Please file a bug report and include your $WASMER_DIR/wapm.sqlite file",
                       pkv, key_val, key_id);
            }
        } else {
            // key not found in database try the one before it
        }
    }
    conn
}*/

#[derive(Debug, Error)]
pub enum PersonalKeyError {
    #[error("A public key matching {0:?} already exists in the local database")]
    PublicKeyAlreadyExists(String),
    #[error(
        "The private key at {0:?} is already assoicated with public key {1:?} in the local database",
    )]
    PrivateKeyAlreadyRegistered(String, String),
}

#[derive(Debug, Error)]
pub enum WapmPublicKeyError {
    #[error("A public key matching {0:?} already exists on user {1:?} in the local database")]
    PublicKeyAlreadyExists(String, String),
}
