use anyhow::Context;

use crate::util;

pub fn whoami() -> anyhow::Result<()> {
    let username = util::get_username()?.context("(not logged in)")?;
    println!("{username}");
    Ok(())
}
