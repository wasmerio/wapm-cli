use crate::config::Config;

pub fn logout() -> Result<(), failure::Error> {
    let mut config = Config::from_file()?;
    config.registry.token = None;
    config.save()?;
    Ok(())
}
