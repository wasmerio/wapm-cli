use crate::config::Config;

pub fn logout() -> anyhow::Result<()> {
    let mut config = Config::from_file()?;
    config.registry.token = None;
    config.save()?;
    Ok(())
}
