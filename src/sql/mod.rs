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

#[cfg(test)]
mod test {
    fn open_db() -> rusqlite::Connection {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::keys::apply_migrations(&mut conn).unwrap();
        conn
    }

    #[test]
    fn sql_compiles() {
        let conn = open_db();
        let mut sql_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        sql_dir.push("src/sql/queries");
        for entry in sql_dir.read_dir().unwrap() {
            let entry = entry.unwrap();
            let sql_str = std::fs::read_to_string(entry.path()).unwrap();

            assert!(conn.prepare(&sql_str).unwrap().finalize().is_ok());
        }
    }
}
