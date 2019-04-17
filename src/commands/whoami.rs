use crate::util;

pub fn whoami() -> Result<(), failure::Error> {
    let username = util::get_username()?.unwrap_or("(not logged in)".to_string());
    println!("{}", username);
    Ok(())
}
