#![cfg_attr(
    not(feature = "full"),
    allow(dead_code, unused_imports, unused_variables)
)]
use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use graphql_client::GraphQLQuery;
use thiserror::Error;

pub static GLOBAL_CONFIG_FILE_NAME: &str = if cfg!(target_os = "wasi") {
    "/.private/wapm.toml"
} else {
    "wapm.toml"
};

pub static GLOBAL_CONFIG_FOLDER_NAME: &str = ".wasmer";
pub static GLOBAL_WAX_INDEX_FILE_NAME: &str = ".wax_index.json";
pub static GLOBAL_CONFIG_DATABASE_FILE_NAME: &str = "wapm.sqlite";
pub static GLOBAL_CONFIG_FOLDER_ENV_VAR: &str = "WASMER_DIR";

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub struct Config {
    /// The number of seconds to wait before checking the registry for a new
    /// version of the package.
    #[serde(default = "wax_default_cooldown")]
    pub wax_cooldown: i32,

    /// The registry that wapm will connect to.
    pub registry: Registries,

    /// Whether or not telemetry is enabled.
    #[cfg(feature = "telemetry")]
    #[serde(default)]
    pub telemetry: Telemetry,

    /// Whether or not updated notifications are enabled.
    #[cfg(feature = "update-notifications")]
    #[serde(default)]
    pub update_notifications: UpdateNotifications,

    /// The proxy to use when connecting to the Internet.
    #[serde(default)]
    pub proxy: Proxy,
}

/// The default cooldown for wax.
pub const fn wax_default_cooldown() -> i32 {
    5 * 60
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[serde(untagged)]
pub enum Registries {
    Single(Registry),
    Multi(MultiRegistry),
}

impl Default for Registries {
    fn default() -> Self {
        Registries::Single(Registry {
            url: format_graphql("https://registry.wapm.io"),
            token: None,
        })
    }
}

#[test]
fn test_registries_switch_token() {
    let mut registries = Registries::default();

    registries.set_current_registry("https://registry.wapm.dev");
    assert_eq!(registries.get_current_registry(), "https://registry.wapm.dev/graphql".to_string());
    registries.set_login_token_for_registry(
        "https://registry.wapm.io",
        "token1",
        UpdateRegistry::LeaveAsIs,
    );
    assert_eq!(registries.get_current_registry(), "https://registry.wapm.dev/graphql".to_string());
    assert_eq!(registries.get_login_token_for_registry(&registries.get_current_registry()), None);
    registries.set_current_registry("https://registry.wapm.io");
    assert_eq!(registries.get_login_token_for_registry(&registries.get_current_registry()), Some("token1".to_string()));
    registries.clear_current_registry_token();
    assert_eq!(registries.get_login_token_for_registry(&registries.get_current_registry()), None);
}

fn format_graphql(registry: &str) -> String {
    if registry.ends_with("/graphql") {
        registry.to_string()
    } else if registry.ends_with("/") {
        format!("{}graphql", registry)
    } else {
        format!("{}/graphql", registry)
    }
}

impl Registries {
    /// Gets the current (active) registry URL
    pub fn clear_current_registry_token(&mut self) {
        match self {
            Registries::Single(s) => {
                s.token = None;
            }
            Registries::Multi(m) => {
                m.tokens.remove(&m.current);
                m.tokens.remove(&format_graphql(&m.current));
            }
        }
    }

    pub fn get_graphql_url(&self) -> String {
        let registry = self.get_current_registry();
        format_graphql(&registry)
    }

    /// Gets the current (active) registry URL
    pub fn get_current_registry(&self) -> String {
        match self {
            Registries::Single(s) => format_graphql(&s.url),
            Registries::Multi(m) => format_graphql(&m.current),
        }
    }

    /// Sets the current (active) registry URL
    pub fn set_current_registry(&mut self, registry: &str) {
        let registry = format_graphql(registry);
        if let Err(e) = test_if_registry_present(&registry) {
            println!("Error when trying to ping registry {registry:?}: {e}");
            if registry.contains("wapm.dev") {
                println!("Note: The correct URL for wapm.dev is https://registry.wapm.dev, not {registry}");
            } else if registry.contains("wapm.io") {
                println!("Note: The correct URL for wapm.io is https://registry.wapm.io, not {registry}");
            }
            println!("WARNING: Registry {registry:?} will be used, but commands may not succeed.");
        }
        match self {
            Registries::Single(s) => s.url = registry,
            Registries::Multi(m) => m.current = registry,
        }
    }

    /// Returns the login token for the registry
    pub fn get_login_token_for_registry(&self, registry: &str) -> Option<String> {
        match self {
            Registries::Single(s) if s.url == registry || format_graphql(registry) == s.url => s.token.clone(),
            Registries::Multi(m) => m.tokens.get(registry).or_else(|| m.tokens.get(&format_graphql(registry))).cloned(),
            _ => None,
        }
    }

    /// Sets the login token for the registry URL
    pub fn set_login_token_for_registry(
        &mut self,
        registry: &str,
        token: &str,
        update_current_registry: UpdateRegistry,
    ) {
        let new_map = match self {
            Registries::Single(s) => {
                if s.url == registry {
                    Registries::Single(Registry {
                        url: format_graphql(registry),
                        token: Some(token.to_string()),
                    })
                } else {
                    let mut map = BTreeMap::new();
                    if let Some(token) = s.token.clone() {
                        map.insert(format_graphql(&s.url), token);
                    }
                    map.insert(format_graphql(registry), token.to_string());
                    Registries::Multi(MultiRegistry {
                        current: format_graphql(&s.url),
                        tokens: map,
                    })
                }
            }
            Registries::Multi(m) => {
                m.tokens.insert(format_graphql(registry), token.to_string());
                if update_current_registry == UpdateRegistry::Update {
                    m.current = format_graphql(registry);
                }
                Registries::Multi(m.clone())
            }
        };
        *self = new_map;
    }
}


#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/test_if_registry_present.graphql",
    response_derives = "Debug"
)]
struct TestIfRegistryPresent;

fn test_if_registry_present(registry: &str) -> Result<(), String> {
    let q = TestIfRegistryPresent::build_query(test_if_registry_present::Variables {});
    let _: test_if_registry_present::ResponseData = 
        crate::graphql::execute_query_custom_registry(registry, &q)
        .map_err(|e| format!("{e}"))?;
    Ok(())
}

#[derive(PartialEq, Copy, Clone)]
pub enum UpdateRegistry {
    Update,
    LeaveAsIs,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct MultiRegistry {
    /// Currently active registry
    pub current: String,
    /// Map from "RegistryUrl" to "LoginToken", in order to
    /// be able to be able to easily switch between registries
    pub tokens: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
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
            registry: Registries::default(),
            #[cfg(feature = "telemetry")]
            telemetry: Telemetry::default(),
            #[cfg(feature = "update-notifications")]
            update_notifications: UpdateNotifications::default(),
            proxy: Proxy::default(),
            wax_cooldown: wax_default_cooldown(),
        }
    }
}

impl Config {
    pub fn get_current_dir() -> std::io::Result<PathBuf> {
        #[cfg(target_os = "wasi")]
        if let Some(pwd) = std::env::var("PWD").ok() {
            return Ok(PathBuf::from(pwd));
        }
        Ok(std::env::current_dir()?)
    }

    pub fn get_folder() -> Result<PathBuf, GlobalConfigError> {
        Ok(
            if let Some(folder_str) = env::var(GLOBAL_CONFIG_FOLDER_ENV_VAR)
                .ok()
                .filter(|s| !s.is_empty())
            {
                let folder = PathBuf::from(folder_str);
                std::fs::create_dir_all(folder.clone())
                    .map_err(|e| GlobalConfigError::CannotCreateConfigDirectory(e))?;
                folder
            } else {
                #[allow(unused_variables)]
                let default_dir = Self::get_current_dir()
                    .ok()
                    .unwrap_or_else(|| PathBuf::from("/".to_string()));
                #[cfg(feature = "dirs")]
                let home_dir =
                    dirs::home_dir().ok_or(GlobalConfigError::CannotFindHomeDirectory)?;
                #[cfg(not(feature = "dirs"))]
                let home_dir = std::env::var("HOME")
                    .ok()
                    .unwrap_or_else(|| default_dir.to_string_lossy().to_string());
                let mut folder = PathBuf::from(home_dir);
                folder.push(GLOBAL_CONFIG_FOLDER_NAME);
                std::fs::create_dir_all(folder.clone())
                    .map_err(|e| GlobalConfigError::CannotCreateConfigDirectory(e))?;
                folder
            },
        )
    }

    fn get_file_location() -> Result<PathBuf, GlobalConfigError> {
        Ok(Self::get_folder()?.join(GLOBAL_CONFIG_FILE_NAME))
    }

    pub fn get_wax_file_path() -> Result<PathBuf, GlobalConfigError> {
        Config::get_folder().map(|config_folder| config_folder.join(GLOBAL_WAX_INDEX_FILE_NAME))
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
    pub fn save(self: &Self) -> anyhow::Result<()> {
        let path = Self::get_file_location()?;
        let config_serialized = toml::to_string(&self)?;
        let mut file = File::create(path)?;
        file.write_all(config_serialized.as_bytes())?;
        Ok(())
    }

    /// A mocked version of the standard function for integration tests
    #[cfg(feature = "integration_tests")]
    pub fn save(self: &Self) -> anyhow::Result<()> {
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

#[derive(Debug, Error)]
pub enum GlobalConfigError {
    #[error("Error while reading config: [{0}]")]
    Io(std::io::Error),
    #[error("Error while reading config: [{0}]")]
    Toml(toml::de::Error),
    #[error(
        "While falling back to the default location for WASMER_DIR, could not resolve the user's home directory"
    )]
    CannotFindHomeDirectory,
    #[error("Error while creating config directory: [{0}]")]
    CannotCreateConfigDirectory(std::io::Error),
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Key not found: {key}")]
    KeyNotFound { key: String },
    #[error("Failed to parse value `{value}` for key `{key}`")]
    CanNotParse { value: String, key: String },
}

pub fn set(config: &mut Config, key: String, value: String) -> anyhow::Result<()> {
    match key.as_ref() {
        "registry.url" => {
            if config.registry.get_current_registry() != value {
                config.registry.set_current_registry(&value);
            }
            if let Some(u) = crate::util::get_username().ok().and_then(|o| o) {
                println!("Successfully logged into registry {:?} as user {:?}",  config.registry.get_current_registry(), u);
            }
        }
        "registry.token" => {
            config.registry.set_login_token_for_registry(
                &config.registry.get_current_registry(),
                &value,
                UpdateRegistry::LeaveAsIs,
            );
            if let Some(u) = crate::util::get_username().ok().and_then(|o| o) {
                println!("Successfully logged into registry {:?} as user {:?}",  config.registry.get_current_registry(), u);
            }
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
        "wax.cooldown" => {
            let num = value.parse::<i32>().map_err(|_| ConfigError::CanNotParse {
                value: value.clone(),
                key: key.clone(),
            })?;
            config.wax_cooldown = num;
        }
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    config.save()?;
    Ok(())
}

pub fn get(config: &mut Config, key: String) -> anyhow::Result<String> {
    let value = match key.as_ref() {
        "registry.url" => config.registry.get_current_registry(),
        "registry.token" => config
            .registry
            .get_login_token_for_registry(&config.registry.get_current_registry())
            .ok_or(anyhow::anyhow!(
                "Not logged into {:?}",
                config.registry.get_current_registry()
            ))?,
        #[cfg(feature = "telemetry")]
        "telemetry.enabled" => config.telemetry.enabled.clone(),
        #[cfg(feature = "update-notifications")]
        "update-notifications.enabled" => config.update_notifications.enabled.clone(),
        "proxy.url" => {
            if let Some(url) = &config.proxy.url {
                url.clone()
            } else {
                "No proxy configured".to_owned()
            }
        }
        "wax.cooldown" => format!("{}", config.wax_cooldown),
        _ => {
            return Err(ConfigError::KeyNotFound { key }.into());
        }
    };
    Ok(value)
}

#[cfg(test)]
mod test {
    use crate::config::{Config, GLOBAL_CONFIG_FILE_NAME, GLOBAL_CONFIG_FOLDER_ENV_VAR};
    use crate::util::create_temp_dir;
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
        let tmp_dir = create_temp_dir().unwrap();
        let tmp_dir_path: &std::path::Path = tmp_dir.as_ref();
        // set the env var to our temp dir
        std::env::set_var(
            GLOBAL_CONFIG_FOLDER_ENV_VAR,
            tmp_dir_path.display().to_string(),
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
        let tmp_dir = create_temp_dir().unwrap();
        let tmp_dir_path: &std::path::Path = tmp_dir.as_ref();
        let manifest_absolute_path = tmp_dir_path.join(GLOBAL_CONFIG_FILE_NAME);
        let mut file = File::create(&manifest_absolute_path).unwrap();
        let config = Config::default();
        let config_string = toml::to_string(&config).unwrap();
        file.write_all(config_string.as_bytes()).unwrap();
        // set the env var to our temp dir
        std::env::set_var(
            GLOBAL_CONFIG_FOLDER_ENV_VAR,
            tmp_dir_path.display().to_string(),
        );
        let config_result = Config::from_file();
        assert!(config_result.is_ok(), "Config not found.");
    }
}
