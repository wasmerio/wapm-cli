use std::env;
use std::fs::File;
use std::io::prelude::*;

#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    pub registry: Registry,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Registry {
    pub url: String,
    pub token: Option<String>,
}

impl Config {
    fn get_file_location() -> Result<String, GlobalConfigError> {
        let path = match env::var("WASMER_DIR") {
            Ok(dir) => Ok(format!("{}/wapm.toml", dir)),
            Err(_) => Err(GlobalConfigError::MissingWasmerDir),
        };
        path
    }

    pub fn from_file() -> Result<Self, GlobalConfigError> {
        let path = Self::get_file_location()?;
        let mut config_toml = String::new();
        let mut file = File::open(&path).map_err(|_| GlobalConfigError::MissingWasmerDir)?;
        file.read_to_string(&mut config_toml).map_err(|e| GlobalConfigError::Io(e))?;
        toml::from_str(&config_toml).map_err(|e| GlobalConfigError::Toml(e))
    }

    pub fn save(self: &Self) -> Result<(), failure::Error> {
        let path = Self::get_file_location()?;
        let config_serialized = toml::to_string(&self)?;
        let mut file = File::create(path)?;
        file.write_all(config_serialized.as_bytes())?;
        Ok(())
    }
}

impl Registry {
    pub fn get_graphql_url(self: &Self) -> String {
        let url = &self.url;
        if url.ends_with("/") {
            format!("{}graphql", url)
        } else {
            format!("{}/graphql", url)
        }
    }
}

#[derive(Debug, Fail)]
pub enum GlobalConfigError {
    #[fail(display = "\nThe Wasmer directory is missing. I Wasmer not installed?\nInstall Wasmer at https://wasmer.io")]
    MissingWasmerDir,
    #[fail(display = "Error while reading config: [{}]", _0)]
    Io(std::io::Error),
    #[fail(display = "Error while reading config: [{}]", _0)]
    Toml(toml::de::Error),
}
