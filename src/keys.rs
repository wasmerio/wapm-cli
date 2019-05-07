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

/// Information about one of the user's keys
#[derive(Debug)]
pub struct PersonalKey {
    /// Flag saying if the key will be used (there can only be one active key at a time)
    pub active: bool,
    /// The raw value of the Minisign public key
    pub public_key_value: String,
    /// The location in the file system of the private key
    pub private_key_location: Option<String>,
    /// The time at which the key was registered with wapm
    pub date_created: Timespec,
}

/// Information about a public key downloaded from wapm
#[derive(Debug)]
pub struct WapmPublicKey {
    /// The user whose key this is
    pub user_name: String,
    /// The raw value of the Minisign public key
    pub public_key_value: String,
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
        "SELECT active, public_key_value, private_key_location, date_added FROM personal_keys 
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
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// Gets all public keys the user has seen from WAPM from the database
pub fn get_wapm_public_keys_from_database(
    conn: &Connection,
) -> Result<Vec<WapmPublicKey>, failure::Error> {
    let mut stmt = conn.prepare(
        "SELECT user_name, public_key_value, date_added FROM wapm_public_keys ORDER BY date_added;",
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
        })
    })?;

    result.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
}

/// Adds a public/private key pair to the database (storing the public key directly
/// and a path to a file containing the private key)
pub fn add_personal_key_pair_to_database(
    conn: &mut Connection,
    public_key: String,
    private_key: String,
) -> Result<(), failure::Error> {
    let public_key_value = fs::read_to_string(public_key)
        .map_err(|e| format_err!("Could not read public key: {}", e))?;
    {
        let private_key_path = PathBuf::from(&private_key);
        if !private_key_path.exists() {
            error!("Private key file not found at path: {}", &private_key);
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
    }

    // deactivate all keys
    conn.execute("UPDATE personal_keys SET active = 0", params![])
        .unwrap();

    // insert the key and active it
    conn.execute("INSERT INTO personal_keys (public_key_value, active, private_key_location, date_added) values (?1, ?2, ?3, ?4)",
                 params![public_key_value, "1", private_key, time_string]).unwrap();

    Ok(())
}

#[derive(Debug, Fail)]
pub enum PersonalKeyError {
    #[fail(display = "The public key {:?} already exists, cannot insert", _0)]
    PublicKeyAlreadyExists(String),
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
}

/// Applies migrations to the database and updates the `user_version` pragma.
/// Every migration must leave the database in a valid state.
fn apply_migration(conn: &mut Connection, migration_number: i32) -> Result<(), failure::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    match migration_number {
        0 => {
            tx.execute(
                "create table personal_keys
(
    id integer primary key,
    active integer not null,
    public_key_value text not null UNIQUE,
    private_key_location text UNIQUE,
    date_added text not null
)",
                params![],
            )?;

            tx.execute(
                "create table wapm_public_keys
(
    id integer primary key,
    user_name text not null UNIQUE,
    public_key_value text not null UNIQUE,
    date_added text not null
)",
                params![],
            )?;
        }
        _ => {
            return Err(MigrationError::MigrationNumberDoesNotExist(
                migration_number,
                CURRENT_DATA_VERSION,
            )
            .into());
        }
    }
    tx.pragma_update(None, "user_version", &(migration_number + 1))?;
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
}
