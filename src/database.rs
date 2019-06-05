//! Functions and data for dealing with the local (sqlite) database
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
use rusqlite::{Connection, OpenFlags, TransactionBehavior};

pub const RFC3339_FORMAT_STRING: &'static str = "%Y-%m-%dT%H:%M:%S-%f";
pub const RFC3339_FORMAT_STRING_WITH_TIMEZONE: &'static str = "%Y-%m-%dT%H:%M:%S.%f+%Z";
/// The current version of the database.  Update this to perform a migration
pub const CURRENT_DATA_VERSION: i32 = 2;

/// Gets the current time in our standard format
pub fn get_current_time_in_format() -> Option<String> {
    let cur_time = time::now();
    time::strftime(RFC3339_FORMAT_STRING, &cur_time).ok()
}

/// Opens an exclusive read/write connection to the database, creating it if it does not exist
pub fn open_db() -> Result<Connection, failure::Error> {
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
        1 => {
            tx.execute_batch(include_str!("sql/migrations/0001.sql"))
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
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
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
