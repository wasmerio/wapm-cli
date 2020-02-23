use crate::constants::{DEFAULT_RUNTIME, WAPM_RUNTIME_ENV_KEY};
use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::graphql::execute_query;
use graphql_client::*;
use license_exprs;
use semver::Version;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

pub static MAX_NAME_LENGTH: usize = 50;

#[derive(Debug, Fail)]
pub enum NameError {
    #[fail(
        display = "The name \"{}\" is too long. It must be {} characters or fewer",
        _0, _1
    )]
    NameTooLong(String, usize),
    #[fail(
        display = "The name \"{}\" contains invalid characters. Please use alpha-numeric characters, '-', and '_'",
        _0
    )]
    InvalidCharacters(String),
}

/// Checks whether a given package name is acceptable or not
pub fn validate_name(name: &str) -> Result<String, NameError> {
    if name.len() > MAX_NAME_LENGTH {
        return Err(NameError::NameTooLong(name.to_string(), MAX_NAME_LENGTH));
    }

    let re = regex::Regex::new("^[-a-zA-Z0-9_]+").unwrap();

    if !re.is_match(name) {
        return Err(NameError::InvalidCharacters(name.to_string()));
    }

    Ok(name.to_owned())
}

#[derive(Debug, Fail)]
pub enum LicenseError {
    #[fail(display = "\"{}\" is not a valid SPDX license", _0)]
    UnknownLicenseId(String),
    #[fail(display = "License should be a valid SPDX license expression (without \"LicenseRef\")")]
    InvalidStructure(),
}

/// Checks whether a given package name is acceptable or not
pub fn validate_license(license: &str) -> Result<String, LicenseError> {
    match license_exprs::validate_license_expr(license) {
        Ok(_) => Ok(license.to_owned()),
        Err(license_exprs::ParseError::UnknownLicenseId(word)) => {
            Err(LicenseError::UnknownLicenseId(word.to_owned()))
        }
        Err(license_exprs::ParseError::InvalidStructure(_)) => {
            Err(LicenseError::InvalidStructure())
        }
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/whoami.graphql",
    response_derives = "Debug"
)]
struct WhoAmIQuery;

pub fn get_username() -> Result<Option<String>, failure::Error> {
    let q = WhoAmIQuery::build_query(who_am_i_query::Variables {});
    let response: who_am_i_query::ResponseData = execute_query(&q)?;
    Ok(response.viewer.map(|viewer| viewer.username))
}

#[cfg(feature = "telemetry")]
pub fn telemetry_is_enabled() -> bool {
    let mut config = if let Ok(c) = crate::config::Config::from_file() {
        c
    } else {
        // TODO: change this to false when wapm becomes more stable
        // defaulting to on is for the alpha and we should be very conservative about
        // telemetry once we have more confidence in wapm's stability/userbase size
        return true;
    };
    let telemetry_str =
        crate::config::get(&mut config, "telemetry.enabled".to_string()).unwrap_or("true");

    // if we fail to parse, someone probably tried to turn it off
    telemetry_str.parse::<bool>().unwrap_or(false)
}

#[inline]
pub fn get_package_namespace_and_name(package_name: &str) -> Result<(&str, &str), failure::Error> {
    let split: Vec<&str> = package_name.split('/').collect();
    match &split[..] {
        [namespace, name] => Ok((*namespace, *name)),
        [global_package_name] => {
            info!(
                "Interpreting unqualified global package name \"{}\" as \"_/{}\"",
                package_name, global_package_name
            );
            Ok(("_", *global_package_name))
        }
        _ => bail!("Package name is invalid"),
    }
}

#[inline]
pub fn fully_qualified_package_display_name(
    package_name: &str,
    package_version: &Version,
) -> String {
    format!("{}@{}", package_name, package_version)
}

pub fn create_package_dir(
    project_dir: &Path,
    namespace_dir: &str,
    fully_qualified_package_name: &str,
) -> Result<PathBuf, io::Error> {
    let mut package_dir = project_dir.join(PACKAGES_DIR_NAME);
    package_dir.push(namespace_dir);
    package_dir.push(&fully_qualified_package_name);
    fs::create_dir_all(&package_dir)?;
    Ok(package_dir)
}

pub fn wapm_should_print_color() -> bool {
    std::env::var("WAPM_DISABLE_COLOR")
        .map(|_| false)
        .unwrap_or(true)
}

use lazy_static::lazy_static;
use std::sync::Mutex;

#[derive(Debug, Default)]
/// A wrapper type that ensures that the inner type is only set once
pub struct SetOnce<T: Default> {
    set: bool,
    value: T,
}

impl<T: Default> SetOnce<T> {
    pub fn new() -> Self {
        Self {
            set: false,
            value: T::default(),
        }
    }
    pub fn set(&mut self, value: T) -> Option<()> {
        if self.set {
            return None;
        }

        self.value = value;
        self.set = true;
        Some(())
    }

    pub fn get(&self) -> &T {
        &self.value
    }
}

lazy_static! {
    /// Global variable that determines the behavior of prompts
    pub static ref WAPM_FORCE_YES_TO_PROMPTS: Mutex<SetOnce<bool>> = Mutex::new(SetOnce::new());
}

/// If true, prompts should not ask for user input
pub fn wapm_should_accept_all_prompts() -> bool {
    let guard = WAPM_FORCE_YES_TO_PROMPTS.lock().unwrap();
    *guard.get()
}

pub fn set_wapm_should_accept_all_prompts(val: bool) -> Option<()> {
    let mut guard = WAPM_FORCE_YES_TO_PROMPTS.lock().unwrap();
    guard.set(val)
}

/// Asks the user to confirm something. Returns a boolean indicating if the user consented
/// or if the `WAPM_FORCE_YES_TO_PROMPTS` variable is set
pub fn prompt_user_for_yes(prompt: &str) -> Result<bool, failure::Error> {
    use std::io::Write;

    print!("{}\n[y/n] ", prompt);
    std::io::stdout().flush()?;
    if wapm_should_accept_all_prompts() {
        Ok(true)
    } else {
        let mut input_str = String::new();
        std::io::stdin().read_line(&mut input_str)?;
        match input_str.to_lowercase().trim_end() {
            "yes" | "y" => Ok(true),
            _ => Ok(false),
        }
    }
}

/// this function hashes the Wasm module to generate a key
pub fn get_hashed_module_key(path: &Path) -> Option<String> {
    debug!("Creating hash of wasm module at {:?}", path);
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!(
                "Could not read wasm module at {} when attempting to generate hash: {}",
                path.to_string_lossy().to_string(),
                e.to_string()
            );
            return None;
        }
    };
    let hash = wasmer_runtime_core::cache::WasmHash::generate(&bytes[..]);
    Some(hash.encode())
}

#[cfg(feature = "update-notifications")]
pub fn get_latest_runtime_version(runtime: &str) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new(runtime)
        .arg("-V")
        .output()
        .map_err(|err| err.to_string())?;
    let stdout_str = std::str::from_utf8(&output.stdout).map_err(|err| err.to_string())?;
    let mut whitespace_iter = stdout_str.split_whitespace();
    let _first = whitespace_iter.next();
    debug_assert_eq!(_first, Some(runtime));

    match whitespace_iter.next() {
        Some(v) => Ok(v.to_string()),
        None => Err("Can't find the version of wasmer".to_string()),
    }
}

#[cfg(feature = "update-notifications")]
/// Returns `None` if versions can't be taken out of the string.
/// Returns `Some(bool)` where `bool` is whether or not the old version
/// is greater than or equal to the old version.  This is useful for checking
/// if there needs to be an update.
pub fn compare_versions(old: &str, new: &str) -> Option<bool> {
    let old_ver_pieces = old.split('.').collect::<Vec<&str>>();
    let new_ver_pieces = new.split('.').collect::<Vec<&str>>();

    if !(old_ver_pieces.len() == 3 && new_ver_pieces.len() == 3) {
        return None;
    }
    let parse = |pieces: Vec<&str>| -> Option<(usize, usize, usize)> {
        Some((
            pieces[0].parse::<usize>().ok()?,
            pieces[1].parse::<usize>().ok()?,
            pieces[2].parse::<usize>().ok()?,
        ))
    };
    if let (Some(old_ver), Some(new_ver)) = (parse(old_ver_pieces), parse(new_ver_pieces)) {
        Some(old_ver >= new_ver)
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(feature = "update-notifications")]
    #[test]
    pub fn compare_ver_test() {
        assert_eq!(compare_versions("0.1.0", "0.1.0"), Some(true));
        assert_eq!(compare_versions("1.1.0", "0.1.0"), Some(true));
        assert_eq!(compare_versions("1.0.0", "0.2.5"), Some(true));
        assert_eq!(compare_versions("1.0.0", "2.2.5"), Some(false));
        assert_eq!(compare_versions("1.0.0", "2.0.5"), Some(false));
        assert_eq!(compare_versions("1.1.0", "2.0.5"), Some(false));
        assert_eq!(compare_versions("1.1.6", "2.0.0"), Some(false));
        assert_eq!(compare_versions("0.1.1", "0.1.0"), Some(true));
        assert_eq!(compare_versions("0.1.1", "0.2.0"), Some(false));
    }
}

pub fn get_runtime() -> String {
    env::var(WAPM_RUNTIME_ENV_KEY).unwrap_or(DEFAULT_RUNTIME.to_owned())
}
