use crate::config::Config;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub enum ConfigOpt {
    #[structopt(name = "set")]
    /// Sets a key
    Set(ConfigKeyValue),

    #[structopt(name = "get")]
    /// Gets a key
    Get(ConfigKey),
}

#[derive(StructOpt, Debug)]
pub struct ConfigKeyValue {
    #[structopt(parse(from_str))]
    key: String,

    #[structopt(parse(from_str))]
    value: String,
}

#[derive(StructOpt, Debug)]
pub struct ConfigKey {
    #[structopt(parse(from_str))]
    key: String,
}

#[derive(Debug, Fail)]
enum ConfigError {
    #[fail(display = "Key not found: {}", key)]
    KeyNotFound { key: String },
}

fn set(config: &mut Config, key: String, value: String) -> Result<(), failure::Error> {
    match key.as_ref() {
        "registry.url" => {
            if config.registry.url != value {
                config.registry.url = value;
                // Resets the registry token automatically
                config.registry.token = None;
            }
        }
        "registry.token" => {
            config.registry.token = Some(value);
        }
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    config.save()?;
    Ok(())
}

fn get(config: &mut Config, key: String) -> Result<&str, failure::Error> {
    let value = match key.as_ref() {
        "registry.url" => &config.registry.url,
        "registry.token" => {
            unimplemented!()
            // &(config.registry.token.as_ref().map_or("".to_string(), |n| n.to_string()).to_owned())
        }
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    Ok(value)
}

pub fn config(config_opt: ConfigOpt) -> Result<(), failure::Error> {
    let mut config = Config::from_file()?;
    match config_opt {
        ConfigOpt::Set(ConfigKeyValue { key, value }) => set(&mut config, key, value),
        ConfigOpt::Get(ConfigKey { key }) => {
            let value = get(&mut config, key)?;
            println!("{}", value);
            Ok(())
        }
    }
}
