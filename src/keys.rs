//! Functions for dealing with package verification keys
//!
//! Internal information:
//! Schema updates are handled by applying migrations based on the `pragma user_version`
//! in sqlite.
//!
//! To create a new migration:
//!   - add an entry applying the desired changes for the CURRENT_DATA_VERSION in `apply_migration`
//!   - increment `CURRENT_DATA_VERSION`
//!
//! Every data version must leave the database in a valid state.
//! Prefer additive changes over destructive ones

//TODO: Clean up this file
//  - abstract db
//  - separate out SQL and have test that ensures all SQL is _at least_ syntactically correct
//  - reuse more code

use crate::config::Config;
use crate::sql;
use crate::util;
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use std::{fs, path::PathBuf};
use time::Timespec;

pub const RFC3339_FORMAT_STRING: &'static str = "%Y-%m-%dT%H:%M:%S-%f";
pub const CURRENT_DATA_VERSION: i32 = 1;
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

// TODO: make this more generic
/// Opens an exclusive read/write connection to the database, creating it if it does not exist
pub fn open_keys_db() -> Result<Connection, failure::Error> {
    let db_path = Config::get_database_file_path()?;
    let mut conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )?;

    apply_migrations(&mut conn)?;
    Ok(conn)
}

/// Applies migrations to the database
pub fn apply_migrations(conn: &mut Connection) -> Result<(), failure::Error> {
    let user_version = conn.pragma_query_value(None, "user_version", |val| val.get(0))?;
    for data_version in user_version..CURRENT_DATA_VERSION {
        debug!("Applying migration {}", data_version);
        apply_migration(conn, data_version)?;
    }
    Ok(())
}

/// Gets the user's keys from the database
pub fn get_personal_keys_from_database(
    conn: &Connection,
) -> Result<Vec<PersonalKey>, failure::Error> {
    let mut stmt = conn.prepare(sql::GET_PERSONAL_KEYS)?;

    let result = stmt.query_map(params![], |row| {
        Ok(PersonalKey {
            active: row.get(0)?,
            public_key_value: row.get(1)?,
            private_key_location: row.get(2)?,
            date_created: {
                let time_str: String = row.get(3)?;
                time::strptime(&time_str, RFC3339_FORMAT_STRING)
                    .expect(&format!("Failed to parse time string {}", &time_str))
                    .to_timespec()
            },
            key_type_identifier: row.get(4)?,
            public_key_id: row.get(5)?,
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

fn get_current_time_in_format() -> Option<String> {
    let cur_time = time::now();
    time::strftime(RFC3339_FORMAT_STRING, &cur_time).ok()
}

/// Gets all public keys the user has seen from WAPM from the database
pub fn get_wapm_public_keys_from_database(
    conn: &Connection,
) -> Result<Vec<WapmPublicKey>, failure::Error> {
    let mut stmt = conn.prepare(sql::GET_WAPM_PUBLIC_KEYS)?;
    let result = stmt.query_map(params![], |row| {
        Ok(WapmPublicKey {
            user_name: row.get(0)?,
            public_key_value: row.get(1)?,
            date_created: {
                let time_str: String = row.get(2)?;
                time::strptime(&time_str, RFC3339_FORMAT_STRING)
                    .expect(&format!("Failed to parse time string {}", &time_str))
                    .to_timespec()
            },
            key_type_identifier: row.get(3)?,
            public_key_id: row.get(4)?,
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// Get the public key in base64 from its id from the local database
pub fn get_full_personal_public_key_by_id(
    conn: &Connection,
    public_key_id: String,
) -> Result<String, failure::Error> {
    let mut stmt =
        conn.prepare(
            "SELECT public_key_value FROM personal_keys WHERE public_key_id = (?1) ORDER BY date_added LIMIT 1",
        )?;
    let result = stmt
        .query_row(params![public_key_id], |row| Ok(row.get(0)?))
        .map_err(|_| format_err!("No public key matching pattern {} found", &public_key_id))?;

    Ok(result)
}

pub fn get_active_personal_key(conn: &Connection) -> Result<PersonalKey, failure::Error> {
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
                        .expect(&format!("Failed to parse time string {}", &time_str))
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
        Err(format_err!("No active key found"))
    }
}

pub fn delete_key_pair(conn: &mut Connection, public_key: String) -> Result<(), failure::Error> {
    conn.execute(sql::DELETE_PERSONAL_KEY_PAIR, params![public_key])?;
    Ok(())
}

/// This function takes the raw output from Minisign and returns the key's tag
/// and the key's value in base64
pub fn normalize_public_key(pk: String) -> Result<(String, String), failure::Error> {
    dbg!(&pk);
    let mut lines = pk.lines();
    let first_line = dbg!(lines.next().ok_or(format_err!("Empty public key value"))?);
    let second_line = lines
        .next()
        .ok_or(format_err!("Public key value must have two lines"))?;

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
pub fn add_personal_key_pair_to_database(
    conn: &mut Connection,
    public_key_location: String,
    private_key_location: String,
) -> Result<(), failure::Error> {
    let (public_key_id, public_key_value) = normalize_public_key(
        fs::read_to_string(&public_key_location)
            .map_err(|e| format_err!("Could not read public key: {}", e))?,
    )?;
    println!("{:?}", public_key_id);
    {
        let private_key_path = PathBuf::from(&private_key_location);
        if !private_key_path.exists() {
            error!(
                "Private key file not found at path: {}",
                &private_key_location
            );
            if !util::prompt_user_for_yes("Would you like to add the key anyway?")? {
                return Err(format_err!(
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
        let result = key_check.query_map(params![public_key_id, public_key_value], |row| {
            Ok(row.get(0)?)
        })?;

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
    tx.commit()?;
    Ok(())
}

/// Parses a public key out of the given string and adds it to the database of
/// trusted keys associated with the given user
pub fn import_public_key(
    conn: &mut Connection,
    public_key_string: String,
    user_name: String,
) -> Result<(), failure::Error> {
    let (public_key_id, public_key_value) = normalize_public_key(public_key_string)?;

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

pub fn get_latest_public_key_for_user(
    conn: &Connection,
    user_name: &str,
) -> Result<Option<WapmPublicKey>, failure::Error> {
    let mut stmt = conn.prepare(sql::GET_LATEST_PUBLIC_KEY_FOR_USER)?;

    match stmt.query_row(params![user_name], |row| {
        Ok(Some(WapmPublicKey {
            user_name: user_name.to_string(),
            public_key_id: row.get(0)?,
            public_key_value: row.get(1)?,
            date_created: {
                let time_str: String = row.get(2)?;
                time::strptime(&time_str, RFC3339_FORMAT_STRING)
                    .expect(&format!("Failed to parse time string {}", &time_str))
                    .to_timespec()
            },
            key_type_identifier: row.get(3)?,
        }))
    }) {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format_err!("Internal database error: {}", e)),
    }
}

/*pub fn validate_key_history_and_return_latest_key(
    conn: &Connection,
    user_name: String,
    key_history: Vec<(String, Option<String>)>,
) -> Result<WapmPublicKey, failure::Error> {
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

#[derive(Debug, Fail)]
pub enum PersonalKeyError {
    #[fail(display = "A public key matching {:?} already exists", _0)]
    PublicKeyAlreadyExists(String),
    #[fail(
        display = "The private key at {:?} is already assoicated with public key {:?}",
        _0, _1
    )]
    PrivateKeyAlreadyRegistered(String, String),
}

#[derive(Debug, Fail)]
pub enum WapmPublicKeyError {
    #[fail(
        display = "A public key matching {:?} already exists on user {:?}",
        _0, _1
    )]
    PublicKeyAlreadyExists(String, String),
}

#[derive(Debug, Fail)]
pub enum MigrationError {
    #[fail(
        display = "Critical internal error: the data version {} is not handleded; current data version: {}",
        _0, _1
    )]
    MigrationNumberDoesNotExist(i32, i32),
    #[fail(
        display = "Critical internal error: failed to commit trasaction migrating to data version {}",
        _0
    )]
    CommitFailed(i32),
    #[fail(
        display = "Critical internal error: transaction failed on migration number {}: {}",
        _0, _1
    )]
    TransactionFailed(i32, String),
}

/// Applies migrations to the database and updates the `user_version` pragma.
/// Every migration must leave the database in a valid state.
fn apply_migration(conn: &mut Connection, migration_number: i32) -> Result<(), MigrationError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| MigrationError::TransactionFailed(migration_number, format!("{}", e)))?;
    match migration_number {
        0 => {
            tx.execute_batch(include_str!("sql/migrations/0000.sql"))
                .map_err(|e| {
                    MigrationError::TransactionFailed(migration_number, format!("{}", e))
                })?;
        }
        _ => {
            return Err(MigrationError::MigrationNumberDoesNotExist(
                migration_number,
                CURRENT_DATA_VERSION,
            )
            .into());
        }
    }
    tx.pragma_update(None, "user_version", &(migration_number + 1))
        .map_err(|e| MigrationError::TransactionFailed(migration_number, format!("{}", e)))?;
    tx.commit()
        .map_err(|_| MigrationError::CommitFailed(migration_number).into())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        let tmp_dir = tempdir::TempDir::new("DB").unwrap().path().to_owned();
        let mut conn = Connection::open(tmp_dir).unwrap();
        for data_version in 0..CURRENT_DATA_VERSION {
            apply_migration(&mut conn, data_version).unwrap();
        }
        let user_version: i32 = conn
            .pragma_query_value(None, "user_version", |val| val.get(0))
            .unwrap();
        assert_eq!(user_version, CURRENT_DATA_VERSION);
    }

    #[test]
    fn data_version_was_updated() {
        let tmp_dir = tempdir::TempDir::new("DB").unwrap().path().to_owned();
        let mut conn = Connection::open(tmp_dir).unwrap();
        if let Err(MigrationError::MigrationNumberDoesNotExist { .. }) =
            apply_migration(&mut conn, CURRENT_DATA_VERSION)
        {
            // failed for the correct reason
        } else {
            panic!("Migration for CURRENT_DATA_VERSION ({}) found!  Did you forget to increment CURRENT_DATA_VERSION?", CURRENT_DATA_VERSION);
        }
    }
}
