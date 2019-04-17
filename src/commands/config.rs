use crate::config::{get, set, Config};
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
