#![cfg_attr(
    not(feature = "full"),
    allow(dead_code, unused_imports, unused_variables)
)]
pub const GET_PERSONAL_KEYS: &str = include_str!("queries/get_personal_keys.sql");
pub const GET_WAPM_PUBLIC_KEYS: &str = include_str!("queries/get_wapm_public_keys.sql");
pub const DELETE_PERSONAL_KEY_PAIR: &str = include_str!("queries/delete_personal_key_pair.sql");
pub const PERSONAL_PUBLIC_KEY_VALUE_EXISTENCE_CHECK: &str =
    include_str!("queries/personal_public_key_value_existence_check.sql");
pub const PERSONAL_PRIVATE_KEY_VALUE_EXISTENCE_CHECK: &str =
    include_str!("queries/personal_private_key_value_existence_check.sql");
pub const WAPM_PUBLIC_KEY_EXISTENCE_CHECK: &str =
    include_str!("queries/wapm_public_key_existence_check.sql");
pub const INSERT_AND_ACTIVATE_PERSONAL_KEY_PAIR: &str =
    include_str!("queries/insert_and_activate_personal_key_pair.sql");
pub const INSERT_WAPM_PUBLIC_KEY: &str = include_str!("queries/insert_wapm_public_key.sql");
pub const INSERT_USER: &str = include_str!("queries/insert_user.sql");
pub const GET_LATEST_PUBLIC_KEY_FOR_USER: &str =
    include_str!("queries/get_latest_public_key_for_user.sql");
pub const WASM_INTERFACE_EXISTENCE_CHECK: &str =
    include_str!("queries/wasm_interface_existence_check.sql");
pub const INSERT_WASM_INTERFACE: &str = include_str!("queries/insert_interface.sql");
pub const GET_WASM_INTERFACE: &str = include_str!("queries/get_interface.sql");

#[cfg(feature = "full")]
#[cfg(test)]
mod test {
    use super::*;
    use rusqlite::params;

    fn open_db() -> rusqlite::Connection {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::database::apply_migrations(&mut conn).unwrap();
        conn
    }

    const DATE_STR: &str = "2019-05-10T16:12:34-00.000";

    #[test]
    fn sql_tests() {
        let conn = open_db();
        let public_key_id = "79EC1A7316BFD5A";
        let public_key_value = "RWRa/Wsxp8GeB4bcA7v0HAdbYYR00QKwAb5kN8yN+uuyugf51XGuYqWD";
        conn.execute(
            INSERT_AND_ACTIVATE_PERSONAL_KEY_PAIR,
            params![
                public_key_id,
                public_key_value,
                "1",
                "/dog/face",
                DATE_STR,
                "minisign",
            ],
        )
        .unwrap();

        let mut get_pk = conn.prepare(GET_PERSONAL_KEYS).unwrap();
        let pks = get_pk
            .query_map(params![], |row| row.get(5))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();
        assert_eq!(pks, vec![public_key_id.to_string()]);

        let mut key_check = conn
            .prepare(PERSONAL_PUBLIC_KEY_VALUE_EXISTENCE_CHECK)
            .unwrap();
        let result = key_check
            .query_map(params![public_key_id, public_key_value], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();
        assert_eq!(result, vec![public_key_id.to_string()]);

        conn.execute(DELETE_PERSONAL_KEY_PAIR, params![public_key_value])
            .unwrap();

        let result = key_check
            .query_map(params![public_key_id, public_key_value], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();
        assert!(result.is_empty());

        conn.execute(INSERT_USER, params!["ZinedineZidane"])
            .unwrap();
        conn.execute(
            INSERT_WAPM_PUBLIC_KEY,
            params![
                "ZinedineZidane",
                public_key_id,
                public_key_value,
                "minisign",
                DATE_STR,
            ],
        )
        .unwrap();

        let mut key_check = conn.prepare(GET_LATEST_PUBLIC_KEY_FOR_USER).unwrap();
        let result = key_check
            .query_map(params!["ZinedineZidane"], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<String>, _>>()
            .unwrap();
        assert_eq!(result, vec![public_key_id.to_string()]);

        let mut stmt = conn.prepare(WAPM_PUBLIC_KEY_EXISTENCE_CHECK).unwrap();
        let result = stmt
            .query_map(params![public_key_id, public_key_value], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<(String, String)>, _>>()
            .unwrap();
        assert_eq!(
            result,
            vec![("ZinedineZidane".to_string(), public_key_id.to_string())]
        );

        let mut stmt = conn.prepare(GET_WAPM_PUBLIC_KEYS).unwrap();
        let result = stmt
            .query_map(params![], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<Vec<(String, String)>, _>>()
            .unwrap();
        assert_eq!(
            result,
            vec![("ZinedineZidane".to_string(), public_key_value.to_string())]
        );

        conn.execute(
            INSERT_WASM_INTERFACE,
            params![
                "test_interface",
                "0.0.0",
                DATE_STR,
                "this is where the interface data goes!"
            ],
        )
        .unwrap();

        let mut stmt = conn.prepare(WASM_INTERFACE_EXISTENCE_CHECK).unwrap();
        let result = stmt.exists(params!["test_interface", "0.0.0"]).unwrap();
        assert!(result);
    }
}
