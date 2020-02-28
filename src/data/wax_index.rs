//! The Wax index is where temporary commands are tracked for use with the `wax`
//! command.

use crate::config;
use std::convert::From;
use std::fs;
use std::io::{Read, Write, self};
use semver::Version;
use std::path::PathBuf;

use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct WaxIndex {
    index: HashMap<String, WaxIndexEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WaxIndexEntry {
    pub location: PathBuf,
    pub version: Version,
}

impl WaxIndex {
    /// Read the `WaxIndex` from disk
    pub fn open() -> Result<Self, failure::Error> {
        let wax_path = config::Config::get_wax_file_path()?;
        let mut f = fs::OpenOptions::new().read(true).create(true).open(wax_path)?;

        let index_str = {
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            s
        };

        if index_str.is_empty() {
            return Ok(WaxIndex {
                index: Default::default(),
            });
        }

        Ok(toml::from_str(&index_str)?)
    }

    /// Save the `WaxIndex` to disk
    pub fn save(&self) -> Result<(), failure::Error> {
        let wax_path = config::Config::get_wax_file_path()?;
        let mut f = fs::OpenOptions::new().write(true).create(true).truncate(true).open(wax_path)?;

        let toml_str = toml::to_string(self)?;

        f.write_all(toml_str.as_bytes())?;
        Ok(())
    }

    /// This function takes a `&mut` because it will update itself with the
    /// information that it finds.
    pub fn search_for_entry(&mut self, entry: String) -> Result<WaxIndexEntry, failure::Error> {
        if let Some(found_entry) = self.index.get(&entry) {
            // check if entry still exists and if not remove it
            if found_entry.location.exists() {
                return Ok(found_entry.clone());
            }
            self.index.remove(&entry);
        }
        return Err(WaxIndexError::EntryNotFound { entry: entry.clone() }.into());
    }

    /// Package installed, add it to the index.
    ///
    /// Returns the `WaxIndexEntry` at `entry` if one exists
    pub fn insert_entry(&mut self, entry: String, version: Version, location: PathBuf) -> Option<WaxIndexEntry> {
        self.index.insert(entry, WaxIndexEntry {
            version,
            location
        })
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