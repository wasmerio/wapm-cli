#[cfg(feature = "update-notifications")]
use crate::constants::*;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

pub static GLOBAL_CONFIG_FILE_NAME: &str = "wapm.toml";
pub static GLOBAL_CONFIG_FOLDER_NAME: &str = ".wasmer";
pub static GLOBAL_CONFIG_DATABASE_FILE_NAME: &str = "wapm.sqlite";
pub static GLOBAL_CONFIG_FOLDER_ENV_VAR: &str = "WASMER_DIR";
#[cfg(feature = "update-notifications")]
pub static GLOBAL_LAST_UPDATED_TIMESTAMP_FILE: &str = ".last_time_checked_for_update.txt";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Config {
    pub registry: Registry,
    #[cfg(feature = "telemetry")]
    #[serde(default)]
    pub telemetry: Telemetry,
    #[serde(default)]
    pub proxy: Proxy,
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

#[derive(Deserialize, Serialize, Debug, PartialEq, Default)]
pub struct Proxy {
    pub url: Option<String>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            registry: Registry {
                url: "https://registry.wapm.io".to_string(),
                token: None,
            },
            #[cfg(feature = "telemetry")]
            telemetry: Telemetry::default(),
            proxy: Proxy::default(),
        }
    }
}

impl Config {
    pub fn get_folder() -> Result<PathBuf, GlobalConfigError> {
        Ok(
            if let Some(folder_str) = env::var(GLOBAL_CONFIG_FOLDER_ENV_VAR)
                .ok()
                .filter(|s| !s.is_empty())
            {
                PathBuf::from(folder_str)
            } else {
                let home_dir =
                    dirs::home_dir().ok_or(GlobalConfigError::CannotFindHomeDirectory)?;
                let mut folder = PathBuf::from(home_dir);
                folder.push(GLOBAL_CONFIG_FOLDER_NAME);
                folder
            },
        )
    }

    #[cfg(feature = "update-notifications")]
    pub fn get_last_update_checked_time() -> Option<time::Tm> {
        let mut path = Self::get_folder().ok()?;
        path.push(GLOBAL_LAST_UPDATED_TIMESTAMP_FILE);

        if !path.exists() {
            // create a file with the current time if it doesn't exist
            // this is done here rather than after making a request to ensure
            // our failure case is as silent as possible.  It's better to fail
            // early, than to spawn a thread and make a network request on
            // every run of wapm
            let mut f = std::fs::File::create(&path).ok()?;
            let now = time::now();
            f.write(format!("{}", now.rfc3339()).as_bytes()).ok()?;
            None
        } else {
            let time_as_str = std::fs::read_to_string(&path).ok()?;
            time::strptime(&time_as_str, RFC3339_FORMAT_STRING).ok()
        }
    }

    #[cfg(feature = "update-notifications")]
    pub fn set_last_update_checked_time() -> Option<()> {
        let mut path = Self::get_folder().ok()?;
        path.push(GLOBAL_LAST_UPDATED_TIMESTAMP_FILE);
        let mut f = std::fs::OpenOptions::new().write(true).open(path).ok()?;
        let now = time::now();
        f.write(format!("{}", now.rfc3339()).as_bytes()).ok()?;
        Some(())
    }

    fn get_file_location() -> Result<PathBuf, GlobalConfigError> {
        Ok(Self::get_folder()?.join(GLOBAL_CONFIG_FILE_NAME))
    }

    pub fn get_database_file_path() -> Result<PathBuf, GlobalConfigError> {
        Config::get_folder()
            .map(|config_folder| config_folder.join(GLOBAL_CONFIG_DATABASE_FILE_NAME))
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

    pub fn get_globals_directory() -> Result<PathBuf, GlobalConfigError> {
        Self::get_folder().map(|p| p.join("globals"))
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
    #[fail(display = "Error while reading config: [{}]", _0)]
    Io(std::io::Error),
    #[fail(display = "Error while reading config: [{}]", _0)]
    Toml(toml::de::Error),
    #[fail(
        display = "While falling back to the default location for WASMER_DIR, could not resolve the user's home directory"
    )]
    CannotFindHomeDirectory,
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
        "proxy.url" => {
            config.proxy.url = if value.is_empty() { None } else { Some(value) };
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
        "proxy.url" => {
            if let Some(url) = &config.proxy.url {
                &url
            } else {
                "No proxy configured"
            }
        }
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    Ok(value)
}

#[cfg(test)]
mod test {
    use crate::config::{Config, GLOBAL_CONFIG_FILE_NAME, GLOBAL_CONFIG_FOLDER_ENV_VAR};
    use std::fs::*;
    use std::io::Write;

    #[test]
    fn get_config_and_wasmer_dir_does_not_exist() {
        // remove WASMER_DIR
        let _ = std::env::remove_var(GLOBAL_CONFIG_FOLDER_ENV_VAR);
        let config_result = Config::from_file();
        assert!(
            !config_result.is_err(),
            "Config file created by falling back to default"
        );
    }

    #[test]
    fn get_non_existent_config() {
        let tmp_dir = tempdir::TempDir::new("get_non_existent_config").unwrap();
        // set the env var to our temp dir
        std::env::set_var(
            GLOBAL_CONFIG_FOLDER_ENV_VAR,
            tmp_dir.path().display().to_string(),
        );
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
        std::env::set_var(
            GLOBAL_CONFIG_FOLDER_ENV_VAR,
            tmp_dir.path().display().to_string(),
        );
        let config_result = Config::from_file();
        assert!(config_result.is_ok(), "Config not found.");
    }
}
