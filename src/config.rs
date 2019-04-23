use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

pub static GLOBAL_CONFIG_FILE_NAME: &str = "wapm.toml";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Config {
    pub registry: Registry,
    #[cfg(feature = "telemetry")]
    #[serde(default)]
    pub telemetry: Telemetry,
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Registry {
    pub url: String,
    pub token: Option<String>,
}

#[cfg(feature = "telemetry")]
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Telemetry {
    pub enabled: String,
}

#[cfg(feature = "telemetry")]
impl Default for Telemetry {
    fn default() -> Telemetry {
        Telemetry {
            enabled: "true".to_string(),
        }
    }
}

impl Config {
    pub fn default() -> Self {
        Self {
            registry: Registry {
                url: "https://registry.wapm.io".to_string(),
                token: None,
            },
            #[cfg(feature = "telemetry")]
            telemetry: Telemetry::default(),
        }
    }

    fn get_file_location() -> Result<PathBuf, GlobalConfigError> {
        let path = match env::var("WASMER_DIR") {
            Ok(dir) => Ok(PathBuf::from(dir).join(GLOBAL_CONFIG_FILE_NAME)),
            Err(_) => Err(GlobalConfigError::MissingWasmerDir),
        };
        path
    }

    pub fn from_file() -> Result<Self, GlobalConfigError> {
        let path = Self::get_file_location()?;
        match File::open(&path) {
            Ok(mut file) => {
                let mut config_toml = String::new();
                file.read_to_string(&mut config_toml)
                    .map_err(|e| GlobalConfigError::Io(e))?;
                toml::from_str(&config_toml).map_err(|e| GlobalConfigError::Toml(e))
            }
            Err(_e) => Ok(Self::default()),
        }
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
    #[fail(
        display = "\nThe Wasmer directory is missing. Is Wasmer not installed?\nInstall Wasmer at https://wasmer.io."
    )]
    MissingWasmerDir,
    #[fail(display = "Error while reading config: [{}]", _0)]
    Io(std::io::Error),
    #[fail(display = "Error while reading config: [{}]", _0)]
    Toml(toml::de::Error),
}

#[derive(Debug, Fail)]
pub enum ConfigError {
    #[fail(display = "Key not found: {}", key)]
    KeyNotFound { key: String },
}

pub fn set(config: &mut Config, key: String, value: String) -> Result<(), failure::Error> {
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
        #[cfg(feature = "telemetry")]
        "telemetry.enabled" => {
            config.telemetry.enabled = value;
        }
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    config.save()?;
    Ok(())
}

pub fn get(config: &mut Config, key: String) -> Result<&str, failure::Error> {
    let value = match key.as_ref() {
        "registry.url" => &config.registry.url,
        "registry.token" => {
            unimplemented!()
            // &(config.registry.token.as_ref().map_or("".to_string(), |n| n.to_string()).to_owned())
        }
        #[cfg(feature = "telemetry")]
        "telemetry.enabled" => &config.telemetry.enabled,
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    Ok(value)
}

#[cfg(test)]
mod test {
    use crate::config::{Config, GLOBAL_CONFIG_FILE_NAME};
    use std::fs::*;
    use std::io::Write;

    #[test]
    fn get_config_and_wasmer_dir_does_not_exist() {
        // explicitly remove it
        let _ = std::env::remove_var("WASMER_DIR");
        let config_result = Config::from_file();
        assert!(
            config_result.is_err(),
            "Found config file when it does not exist."
        );
    }

    #[test]
    fn get_non_existent_config() {
        let tmp_dir = tempdir::TempDir::new("get_non_existent_config").unwrap();
        // set the env var to our temp dir
        std::env::set_var("WASMER_DIR", tmp_dir.path().display().to_string());
        let config_result = Config::from_file();
        assert!(config_result.is_ok(), "Did not find the default config.");
        let actual_config = config_result.unwrap();
        let expected_config = Config::default();
        assert_eq!(
            expected_config, actual_config,
            "Found config is not the default config."
        );
    }

    #[test]
    fn get_global_config() {
        let tmp_dir = tempdir::TempDir::new("get_global_config").unwrap();
        let manifest_absolute_path = tmp_dir.path().join(GLOBAL_CONFIG_FILE_NAME);
        let mut file = File::create(&manifest_absolute_path).unwrap();
        let config = Config::default();
        let config_string = toml::to_string(&config).unwrap();
        file.write_all(config_string.as_bytes()).unwrap();
        // set the env var to our temp dir
        std::env::set_var("WASMER_DIR", tmp_dir.path().display().to_string());
        let config_result = Config::from_file();
        assert!(config_result.is_ok(), "Config not found.");
    }
}
