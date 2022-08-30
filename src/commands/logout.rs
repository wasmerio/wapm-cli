use crate::config::Config;

pub fn logout() -> anyhow::Result<()> {
    let mut config = Config::from_file()?;
    config.registry.clear_current_registry_token();
    config.save()?;
    Ok(())
}
