use crate::database::*;
use crate::sql;

use rusqlite::{params, Connection, TransactionBehavior};

pub fn interface_exists(
    conn: &mut Connection,
    interface_name: &str,
    version: &str,
) -> Result<bool, failure::Error> {
    let mut stmt = conn.prepare(sql::WASM_INTERFACE_EXISTENCE_CHECK)?;
    Ok(stmt.exists(params![interface_name, version])?)
}

pub fn load_interface_from_db(
    conn: &mut Connection,
    interface_name: &str,
    version: &str,
) -> Result<wasm_interface::Interface, failure::Error> {
    let mut stmt = conn.prepare(sql::GET_WASM_INTERFACE)?;
    let interface_string: String =
        stmt.query_row(params![interface_name, version], |row| Ok(row.get(0)?))?;

    wasm_interface::parser::parse_interface(&interface_string).map_err(|e| {
        format_err!(
            "Failed to parse interface {} version {} in database: {}",
            interface_name,
            version,
            e
        )
    })
}

pub fn import_interface(
    conn: &mut Connection,
    interface_name: &str,
    version: &str,
    content: &str,
) -> Result<(), failure::Error> {
    // fail if we already have this interface
    {
        let mut key_check = conn.prepare(sql::WASM_INTERFACE_EXISTENCE_CHECK)?;
        let result = key_check.exists(params![interface_name, version])?;

        if result {
            return Err(format_err!(
                "Interface {}, version {} already exists",
                interface_name,
                version
            ));
        }
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let time_string = get_current_time_in_format().expect("Could not get current time");

    debug!("Adding interface {:?} {:?}", interface_name, version);
    tx.execute(
        sql::INSERT_WASM_INTERFACE,
        params![interface_name, version, time_string, content],
    )?;

    tx.commit()?;
    Ok(())
}
