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
    pub fn default() -> Self {
        Self {
            registry: Registry {
                url: "https://registry.wapm.dev".to_string(),
                token: None,
            },
        }
    }

    fn get_file_location() -> Option<String> {
        let path = match env::var("WASMER_DIR") {
            Ok(dir) => Some(format!("{}/wapm.toml", dir)),
            Err(_) => None,
        };
        path
    }

    pub fn from_file() -> Self {
        let path = match Self::get_file_location() {
            Some(dir) => dir,
            None => {
                // error!("Could not find config file, using default!");
                return Config::default();
            }
        };

        let mut config_toml = String::new();

        let mut file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => {
                // error!("Could not find config file, using default!");
                return Config::default();
            }
        };

        file.read_to_string(&mut config_toml)
            .unwrap_or_else(|err| panic!("Error while reading config: [{}]", err));

        let config: Config = toml::from_str(&config_toml).unwrap();
        config
    }

    pub fn save(self: &Self) -> Result<(), failure::Error> {
        let path = match Self::get_file_location() {
            Some(dir) => dir,
            None => {
                // error!("Could not find config file, using default!");
                // return Config::default();
                panic!("Don't know where to save the file");
            }
        };
        let config_serialized = toml::to_string(&self)?;
        let mut file = File::create(path)?;
        file.write_all(config_serialized.as_bytes())?;
        Ok(())
        // println!("CONFIG SERIALIZED {}", config_serialized);
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
