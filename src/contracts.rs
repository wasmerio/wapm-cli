use crate::database::*;
use crate::sql;

use rusqlite::{params, Connection, TransactionBehavior};

pub fn contract_exists(
    conn: &mut Connection,
    contract_name: &str,
    version: &str,
) -> Result<bool, failure::Error> {
    let mut stmt = conn.prepare(sql::WASM_CONTRACT_EXISTENCE_CHECK)?;
    Ok(stmt.exists(params![contract_name, version])?)
}

pub fn load_contract_from_db(
    conn: &mut Connection,
    contract_name: &str,
    version: &str,
) -> Result<wasm_contract::Contract, failure::Error> {
    let mut stmt = conn.prepare(sql::GET_WASM_CONTRACT)?;
    let contract_string: String =
        stmt.query_row(params![contract_name, version], |row| Ok(row.get(0)?))?;

    wasm_contract::parser::parse_contract(&contract_string).map_err(|e| {
        format_err!(
            "Failed to parse contract {} version {} in database: {}",
            contract_name,
            version,
            e
        )
    })
}

pub fn import_contract(
    conn: &mut Connection,
    contract_name: &str,
    version: &str,
    content: &str,
) -> Result<(), failure::Error> {
    // fail if we already have this contract
    {
        let mut key_check = conn.prepare(sql::WASM_CONTRACT_EXISTENCE_CHECK)?;
        let result = key_check.exists(params![contract_name, version])?;

        if result {
            return Err(format_err!(
                "Contract {}, version {} already exists",
                contract_name,
                version
            ));
        }
    }

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let time_string = get_current_time_in_format().expect("Could not get current time");

    debug!("Adding contract {:?} {:?}", contract_name, version);
    tx.execute(
        sql::INSERT_WASM_CONTRACT,
        params![contract_name, version, time_string, content],
    )?;

    tx.commit()?;
    Ok(())
}
