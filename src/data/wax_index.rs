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
use thiserror::Error;

use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct WaxIndex {
    base_dir: PathBuf,
    index: HashMap<String, WaxEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
struct WaxEntry {
    /// Fully qualified package name `namespace/name@version`
    package_name: String,
    /// Timestamp when wax was last updated
    last_updated: String,
}

impl WaxEntry {
    fn new(name: String, version: Version, time: String) -> Self {
        WaxEntry {
            package_name: format!("{}@{}", name, version),
            last_updated: time,
        }
    }
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
                serde_json::from_str(&index_str)?
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

        let json_str = serde_json::to_string(self)?;

        f.write_all(json_str.as_bytes())?;
        trace!("WaxIndex saved!");
        Ok(())
    }

    /// This function takes a `&mut` because it will update itself with the
    /// information that it finds.
    pub fn search_for_entry(
        &mut self,
        entry: String,
    ) -> Result<(String, Version, time::Timespec), WaxIndexError> {
        if let Some(WaxEntry {
            package_name,
            last_updated,
        }) = self.index.get(&entry)
        {
            let location = self.base_path().join(package_name);
            // check if entry still exists and if not remove it
            if location.exists() {
                trace!("Wax entry found and it still exists!");
                let mut splitter = package_name.split('@');
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
                    time::strptime(last_updated, RFC3339_FORMAT_STRING).map_err(|e| {
                        WaxIndexError::EntryCorrupt {
                            entry: e.to_string(),
                        }
                    })?;
                return Ok((package_name, version, last_seen.to_timespec()));
            }
            trace!("Wax entry found but it no longer exists, removing from registry!");
            self.index.remove(&entry);
        }
        Err(WaxIndexError::EntryNotFound {
            entry: entry.clone(),
        })
    }

    /// Package installed, add it to the index.
    ///
    /// Returns true if an existing entry was updated.
    pub fn insert_entry(&mut self, entry: String, version: Version, package_name: String) -> bool {
        let now = time::now_utc();
        let now_str = time::strftime(RFC3339_FORMAT_STRING, &now).expect("Format current time!");
        self.index
            .insert(entry, WaxEntry::new(package_name, version, now_str))
            .is_some()
    }

    /// Get path at which packages should be installed.
    pub fn base_path(&self) -> &Path {
        &self.base_dir
    }
}

#[derive(Debug, Error)]
pub enum WaxIndexError {
    #[error("Error finding Wax Index: {0}")]
    ConfigError(config::GlobalConfigError),
    #[error("Failed to operate on Wax index file: `{0}`")]
    IoError(io::Error),
    #[error("Failed to parse WaxIndex from JSON or convert WaxIndex to JSON: `{0}`")]
    SerdeError(serde_json::error::Error),
    #[error("Entry `{entry}` not found")]
    EntryNotFound { entry: String },
    #[error("Entry `{entry}` found but was corrupt")]
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

impl From<serde_json::error::Error> for WaxIndexError {
    fn from(other: serde_json::error::Error) -> Self {
        WaxIndexError::SerdeError(other)
    }
}
