use crate::util;

pub fn whoami() -> anyhow::Result<()> {
    let username = util::get_username()?.unwrap_or("(not logged in)".to_string());
    println!("{}", username);
    Ok(())
}
