use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

pub static GLOBAL_CONFIG_FILE_NAME: &str = "wapm.toml";
pub static GLOBAL_CONFIG_FOLDER_NAME: &str = ".wasmer";
pub static GLOBAL_WAX_INDEX_FILE_NAME: &str = ".wax_index.toml";
pub static GLOBAL_CONFIG_DATABASE_FILE_NAME: &str = "wapm.sqlite";
pub static GLOBAL_CONFIG_FOLDER_ENV_VAR: &str = "WASMER_DIR";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Config {
    pub registry: Registry,
    #[cfg(feature = "telemetry")]
    #[serde(default)]
    pub telemetry: Telemetry,
    #[cfg(feature = "update-notifications")]
    #[serde(default)]
    pub update_notifications: UpdateNotifications,
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

#[cfg(feature = "update-notifications")]
#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct UpdateNotifications {
    pub enabled: String,
}

#[cfg(feature = "update-notifications")]
impl Default for UpdateNotifications {
    fn default() -> UpdateNotifications {
        Self {
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
            #[cfg(feature = "update-notifications")]
            update_notifications: UpdateNotifications::default(),
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

    fn get_file_location() -> Result<PathBuf, GlobalConfigError> {
        Ok(Self::get_folder()?.join(GLOBAL_CONFIG_FILE_NAME))
    }

    pub fn get_wax_file_path() -> Result<PathBuf, GlobalConfigError> {
        Config::get_folder()
            .map(|config_folder| config_folder.join(GLOBAL_WAX_INDEX_FILE_NAME))
    }

    pub fn get_database_file_path() -> Result<PathBuf, GlobalConfigError> {
        Config::get_folder()
            .map(|config_folder| config_folder.join(GLOBAL_CONFIG_DATABASE_FILE_NAME))
    }

    /// Load the config from a file
    #[cfg(not(feature = "integration_tests"))]
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

    /// A mocked version of the standard function for integration tests
    #[cfg(feature = "integration_tests")]
    pub fn from_file() -> Result<Self, GlobalConfigError> {
        crate::integration_tests::data::RAW_CONFIG_DATA.with(|rcd| {
            if let Some(ref config_toml) = *rcd.borrow() {
                toml::from_str(&config_toml).map_err(|e| GlobalConfigError::Toml(e))
            } else {
                Ok(Self::default())
            }
        })
    }

    pub fn get_globals_directory() -> Result<PathBuf, GlobalConfigError> {
        Self::get_folder().map(|p| p.join("globals"))
    }

    /// Save the config to a file
    #[cfg(not(feature = "integration_tests"))]
    pub fn save(self: &Self) -> Result<(), failure::Error> {
        let path = Self::get_file_location()?;
        let config_serialized = toml::to_string(&self)?;
        let mut file = File::create(path)?;
        file.write_all(config_serialized.as_bytes())?;
        Ok(())
    }

    /// A mocked version of the standard function for integration tests
    #[cfg(feature = "integration_tests")]
    pub fn save(self: &Self) -> Result<(), failure::Error> {
        let config_serialized = toml::to_string(&self)?;
        crate::integration_tests::data::RAW_CONFIG_DATA.with(|rcd| {
            *rcd.borrow_mut() = Some(config_serialized);
        });

        Ok(())
    }

    #[cfg(feature = "update-notifications")]
    pub fn update_notifications_enabled() -> bool {
        Self::from_file()
            .map(|c| c.update_notifications.enabled == "true")
            .unwrap_or(true)
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
        #[cfg(feature = "update-notifications")]
        "update-notifications.enabled" => {
            config.update_notifications.enabled = value;
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
        #[cfg(feature = "update-notifications")]
        "update-notifications.enabled" => &config.update_notifications.enabled,
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
