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

use crate::config::Config;
use rusqlite::{params, Connection, TransactionBehavior};
use std::{fs, path::PathBuf};
use time::Timespec;

pub const RFC3339_FORMAT_STRING: &'static str = "%Y-%m-%dT%H:%M:%S-%f";
pub const CURRENT_DATA_VERSION: i32 = 1;
pub const MINISIGN_UNTRUSTED_COMMENT_PREFIX: &'static str =
    "untrusted comment: minisign public key ";

/// Information about one of the user's keys
#[derive(Debug)]
pub struct PersonalKey {
    /// Flag saying if the key will be used (there can only be one active key at a time)
    pub active: bool,
    /// The raw value of the public key
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
    /// The raw value of the public key
    pub public_key_value: String,
    /// The type of private/public key this is
    pub key_type_identifier: String,
    /// The time at which the key was seen by the user's instance of wapm
    pub date_created: Timespec,
}

// TODO: make this more generic
/// Connects to the database
pub fn open_keys_db() -> Result<Connection, failure::Error> {
    let db_path = Config::get_database_file_path()?;
    let mut conn = Connection::open(db_path)?;

    let user_version = conn.pragma_query_value(None, "user_version", |val| val.get(0))?;
    for data_version in user_version..CURRENT_DATA_VERSION {
        debug!("Applying migration {}", data_version);
        apply_migration(&mut conn, data_version)?;
    }

    Ok(conn)
}

/// Gets the user's keys from the database
pub fn get_personal_keys_from_database(
    conn: &Connection,
) -> Result<Vec<PersonalKey>, failure::Error> {
    let mut stmt = conn.prepare(
        "SELECT active, public_key_value, private_key_location, date_added, key_type_identifier FROM personal_keys 
         ORDER BY date_added;",
    )?;

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
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// Gets all public keys the user has seen from WAPM from the database
pub fn get_wapm_public_keys_from_database(
    conn: &Connection,
) -> Result<Vec<WapmPublicKey>, failure::Error> {
    let mut stmt = conn.prepare(
        "SELECT user_name, public_key_value, date_added, key_type_identifier FROM wapm_public_keys ORDER BY date_added;",
    )?;

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
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

pub fn get_full_personal_public_key_by_pattern(
    conn: &Connection,
    public_key_id: String,
) -> Result<String, failure::Error> {
    if public_key_id.contains('\'') {
        return Err(format_err!("Invalid public key pattern: pattern cannot contain the ' character"));
    }
    let mut stmt = 
        conn.prepare(
            &format!("SELECT public_key_value FROM personal_keys WHERE public_key_value LIKE '{}%' ORDER BY date_added LIMIT 1", public_key_id)
        )?;
    let result = stmt.query_map(
        params![],
        |row| Ok(row.get(0)?)
    )?.next();

    if let Some(Ok(full_public_key)) = result {
        Ok(full_public_key)
    } else {
        Err(format_err!("No public key matching pattern {} found", &public_key_id))
    }
}

pub fn delete_key_pair(conn: &mut Connection, public_key: String) -> Result<(), failure::Error> {
    conn.execute(
        "DELETE FROM personal_keys WHERE public_key_value = (?1)",
        params![public_key],
    )?;
    Ok(())
}

/// Adds a public/private key pair to the database (storing the public key directly
/// and a path to a file containing the private key)
pub fn add_personal_key_pair_to_database(
    conn: &mut Connection,
    public_key_location: String,
    private_key_location: String,
) -> Result<(), failure::Error> {
    let public_key_value = fs::read_to_string(&public_key_location)
        .map_err(|e| format_err!("Could not read public key: {}", e))?
        .trim_start_matches(MINISIGN_UNTRUSTED_COMMENT_PREFIX)
        .to_owned();
    {
        let private_key_path = PathBuf::from(&private_key_location);
        if !private_key_path.exists() {
            error!(
                "Private key file not found at path: {}",
                &private_key_location
            );
        }
    }
    let cur_time = time::now();
    let time_string = time::strftime(RFC3339_FORMAT_STRING, &cur_time)
        .map_err(|e| format_err!("Corrupt value in database: {}", e))?;

    // fail if we already have the key
    {
        let mut key_check = conn
            .prepare("SELECT public_key_value FROM personal_keys WHERE public_key_value = (?1)")?;
        let result = key_check.query_map(params![public_key_value], |row| Ok(row.get(0)?))?;

        if let [existing_key] = &result.collect::<Result<Vec<String>, _>>()?[..] {
            return Err(PersonalKeyError::PublicKeyAlreadyExists(existing_key.to_string()).into());
        }

        // check the private key path too
        let mut private_key_check = conn.prepare(
            "SELECT private_key_location, public_key_value FROM personal_keys WHERE private_key_location = (?1)",
        )?;
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

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    // deactivate all keys
    tx.execute("UPDATE personal_keys SET active = 0", params![])?;

    // insert the key and activate it
    tx.execute("INSERT INTO personal_keys (public_key_value, active, private_key_location, date_added, key_type_identifier) values (?1, ?2, ?3, ?4, ?5)",
                 params![public_key_value, "1", private_key_location, time_string, "minisign"])?;
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Fail)]
pub enum PersonalKeyError {
    #[fail(display = "The public key {:?} already exists, cannot insert", _0)]
    PublicKeyAlreadyExists(String),
    #[fail(
        display = "The private key at {:?} is already assoicated with public key {:?}",
        _0, _1
    )]
    PrivateKeyAlreadyRegistered(String, String),
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
            tx.execute(
                "create table personal_keys
(
    id integer primary key,
    active integer not null,
    public_key_value text not null UNIQUE,
    private_key_location text UNIQUE,
    key_type_identifier text not null,
    date_added text not null
)",
                params![],
            )
            .map_err(|e| MigrationError::TransactionFailed(migration_number, format!("{}", e)))?;

            tx.execute(
                "create table wapm_public_keys
(
    id integer primary key,
    user_name text not null UNIQUE,
    public_key_value text not null UNIQUE,
    key_type_identifier text not null,
    date_added text not null
)",
                params![],
            )
            .map_err(|e| MigrationError::TransactionFailed(migration_number, format!("{}", e)))?;
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
