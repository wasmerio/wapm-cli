//! The Wax index is where temporary commands are tracked for use with the `wax`
//! command.

use crate::config;
use crate::constants::RFC3339_FORMAT_STRING;
use semver::Version;
use std::convert::From;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct WaxIndex {
    base_dir: PathBuf,
    #[serde(serialize_with = "toml::ser::tables_last")]
    index: HashMap<String, (String, String)>,
}

impl WaxIndex {
    /// Read the `WaxIndex` from disk
    pub fn open() -> Result<Self, WaxIndexError> {
        trace!("Loading WaxIndex!");
        let wax_path = config::Config::get_wax_file_path()?;
        let wax_index = if wax_path.exists() {
            let mut f = fs::OpenOptions::new().read(true).open(wax_path)?;

            let index_str = {
                let mut s = String::new();
                f.read_to_string(&mut s)?;
                s
            };

            if index_str.is_empty() {
                WaxIndex {
                    index: Default::default(),
                    base_dir: env::temp_dir().join("wax"),
                }
            } else {
                toml::from_str(&index_str)?
            }
        } else {
            WaxIndex {
                index: Default::default(),
                base_dir: env::temp_dir().join("wax"),
            }
        };

        // ensure the directory exists
        fs::create_dir_all(&wax_index.base_dir)?;
        trace!("WaxIndex created!");

        Ok(wax_index)
    }

    /// Save the `WaxIndex` to disk
    pub fn save(&self) -> Result<(), WaxIndexError> {
        trace!("Saving WaxIndex!");
        let wax_path = config::Config::get_wax_file_path()?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(wax_path)?;

        let toml_str = toml::to_string(self)?;

        f.write_all(toml_str.as_bytes())?;
        trace!("WaxIndex saved!");
        Ok(())
    }

    /// This function takes a `&mut` because it will update itself with the
    /// information that it finds.
    pub fn search_for_entry(
        &mut self,
        entry: String,
    ) -> Result<(String, Version, time::Timespec), WaxIndexError> {
        if let Some((found_entry, found_time)) = self.index.get(&entry) {
            let location = self.base_path().join(found_entry);
            // check if entry still exists and if not remove it
            if location.exists() {
                trace!("Wax entry found and it still exists!");
                let mut splitter = found_entry.split('@');
                let package_name = splitter
                    .next()
                    .ok_or_else(|| WaxIndexError::EntryCorrupt {
                        entry: entry.clone(),
                    })?
                    .to_string();
                let version = splitter
                    .next()
                    .and_then(|v| Version::parse(v).ok())
                    .ok_or_else(|| WaxIndexError::EntryCorrupt {
                        entry: entry.clone(),
                    })?;
                let last_seen =
                    time::strptime(&found_time, RFC3339_FORMAT_STRING).map_err(|e| {
                        WaxIndexError::EntryCorrupt {
                            entry: format!("{}", e),
                        }
                    })?;
                return Ok((package_name, version, last_seen.to_timespec()));
            }
            trace!("Wax entry found but it no longer exists, removing from registry!");
            self.index.remove(&entry);
        }
        return Err(WaxIndexError::EntryNotFound {
            entry: entry.clone(),
        }
        .into());
    }

    /// Package installed, add it to the index.
    ///
    /// Returns the existing entry as a `package_name@version` String if one exists
    pub fn insert_entry(
        &mut self,
        entry: String,
        version: Version,
        package_name: String,
    ) -> Option<(String, String)> {
        let now = time::now();
        let now_str = time::strftime(RFC3339_FORMAT_STRING, &now).ok()?;
        self.index
            .insert(entry, (format!("{}@{}", package_name, version), now_str))
    }

    /// Get path at which packages should be installed.
    pub fn base_path(&self) -> &Path {
        &self.base_dir
    }
}

#[derive(Debug, Fail)]
pub enum WaxIndexError {
    #[fail(display = "Error finding Wax Index: {}", _0)]
    ConfigError(config::GlobalConfigError),
    #[fail(display = "Failed to operate on Wax index file: `{}`", _0)]
    IoError(io::Error),
    #[fail(display = "Failed to parse WaxIndex from toml: `{}`", _0)]
    IndexParseError(toml::de::Error),
    #[fail(display = "Failed to covert WaxIndex to toml: `{}`", _0)]
    IndexConvertError(toml::ser::Error),
    #[fail(display = "Entry `{}` not found", entry)]
    EntryNotFound { entry: String },
    #[fail(display = "Entry `{}` found but was corrupt", entry)]
    EntryCorrupt { entry: String },
}

impl From<config::GlobalConfigError> for WaxIndexError {
    fn from(other: config::GlobalConfigError) -> Self {
        WaxIndexError::ConfigError(other)
    }
}

impl From<io::Error> for WaxIndexError {
    fn from(other: io::Error) -> Self {
        WaxIndexError::IoError(other)
    }
}

impl From<toml::de::Error> for WaxIndexError {
    fn from(other: toml::de::Error) -> Self {
        WaxIndexError::IndexParseError(other)
    }
}

impl From<toml::ser::Error> for WaxIndexError {
    fn from(other: toml::ser::Error) -> Self {
        WaxIndexError::IndexConvertError(other)
    }
}
