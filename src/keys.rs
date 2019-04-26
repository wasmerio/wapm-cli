//! Functions for dealing with package verification keys

use crate::config::Config;
use rusqlite::{Connection, NO_PARAMS};
use std::{fs, path::PathBuf};
use time::Timespec;

pub const RFC3339_FORMAT_STRING: &'static str = "%Y-%m-%dT%H:%M:%S-%f";

#[derive(Debug)]
pub struct PersonalKey {
    pub active: bool,
    pub public_key_value: String,
    pub private_key_location: Option<String>,
    pub date_created: Timespec,
}

pub fn open_keys_db() -> Result<Connection, failure::Error> {
    let db_path = Config::get_database_file_path()?;
    let conn = Connection::open(db_path)?;

    conn.execute(
        "create table if not exists personal_keys
(
    id integer primary key,
    active integer not null,
    public_key_value text not null,
    private_key_location text,
    date_added text not null
)",
        NO_PARAMS,
    )?;

    Ok(conn)
}

pub fn get_personal_keys_from_database(
    conn: &Connection,
) -> Result<Vec<PersonalKey>, failure::Error> {
    let mut stmt = conn.prepare(
        "SELECT active, public_key_value, private_key_location, date_added from personal_keys;",
    )?;

    let result = stmt.query_map(NO_PARAMS, |row| {
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

    conn.execute("INSERT INTO personal_keys (public_key_value, active, private_key_location, date_added) values (?1, ?2, ?3, ?4)",
                 &[&public_key_value, "0", &private_key, &time_string]).unwrap();

    Ok(())
}
