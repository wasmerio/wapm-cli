use crate::constants::{DEFAULT_RUNTIME, WAPM_RUNTIME_ENV_KEY};
use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::graphql::execute_query;
use graphql_client::*;
use license_exprs;
use semver::Version;
use std::path::{Path, PathBuf};
use std::{env, fs, io};
use thiserror::Error;

pub static MAX_NAME_LENGTH: usize = 50;

#[derive(Debug, Error)]
pub enum NameError {
    #[error("Please enter a name")]
    Empty,
    #[error("The name \"{0}\" is too long. It must be {1} characters or fewer")]
    NameTooLong(String, usize),
    #[error(
        "The name \"{0}\" contains invalid characters. Please use alpha-numeric characters, '-', and '_'",
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

/// Checks whether a given command / runner name is acceptable or not
pub fn validate_runner(runner: &str) -> Result<String, NameError> {
    if runner.trim().is_empty() {
        Err(NameError::Empty)
    } else {
        Ok(runner.trim().to_string())
    }
}

#[derive(Debug, Error)]
pub enum LicenseError {
    #[error("\"{0}\" is not a valid SPDX license")]
    UnknownLicenseId(String),
    #[error("License should be a valid SPDX license expression (without \"LicenseRef\")")]
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

pub fn get_username() -> anyhow::Result<Option<String>> {
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
    let telemetry_str = crate::config::get(&mut config, "telemetry.enabled".to_string())
        .unwrap_or_else(|_| "true".to_string());

    // if we fail to parse, someone probably tried to turn it off
    telemetry_str.parse::<bool>().unwrap_or(false)
}

#[inline]
pub fn get_package_namespace_and_name(package_name: &str) -> anyhow::Result<(&str, &str)> {
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
    package_dir.push(fully_qualified_package_name);
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
pub fn prompt_user_for_yes(prompt: &str) -> anyhow::Result<bool> {
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

#[cfg(feature = "prehash-module")]
/// This function hashes the Wasm module to generate a key.
/// We use it to speed up the time required to run a commands
/// since it doesn't require doing the hash of the module at runtime.
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
    let hash = blake3::hash(&bytes[..]);
    let str_hash = hex::encode(&hash.as_bytes());
    Some(str_hash)
}

#[cfg(not(feature = "prehash-module"))]
pub fn get_hashed_module_key(_path: &Path) -> Option<String> {
    None
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

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum VersionComparison {
    // new > old,
    NewIsGreater,
    // new < old
    NewIsLesser,
    // new == old
    NewIsEqual,
}

/// Returns `None` if versions can't be taken out of the string.
/// Returns `Some(bool)` where `bool` is whether or not the new version
/// is greater than or equal to the old version.  This is useful for checking
/// if there needs to be an update.
pub fn compare_versions(old: &str, new: &str) -> Result<VersionComparison, semver::Error> {
    println!("compare versions: {old} - {new}");
    let old: semver::Version = old.strip_prefix('v').unwrap_or(old).parse()?;
    let new: semver::Version = new.strip_prefix('v').unwrap_or(new).parse()?;
    println!("compare versions after stripping: {old:?}, {new:?}");
    let r = match new.cmp(&old) {
        std::cmp::Ordering::Less => Ok(VersionComparison::NewIsLesser),
        std::cmp::Ordering::Equal => Ok(VersionComparison::NewIsEqual),
        std::cmp::Ordering::Greater => Ok(VersionComparison::NewIsGreater),
    };
    println!("result: {:?}", r);
    r
}

/// Returns the value of the WAPM_RUNTIME env var if it exists.
/// Otherwise returns wasmer
fn get_runtime() -> String {
    env::var(WAPM_RUNTIME_ENV_KEY).unwrap_or_else(|_| DEFAULT_RUNTIME.to_owned())
}

/// Splits the runtime from the rest of arguments
fn split_runtime_and_args(runtime: String) -> (String, Vec<String>) {
    let runtime_split = runtime.split_whitespace();
    if let Some((split_runtime, split_runtime_args)) = runtime_split
        .map(|s| s.to_string())
        .collect::<Vec<String>>()
        .split_first()
    {
        return (split_runtime.to_string(), split_runtime_args.to_vec());
    }
    (runtime, vec![])
}

/// We put this in a new function, to be clear that runtime can be both
/// 1. A string with the runtime value (eg. "wasmer")
/// 2. A string with the runtime value and the args (eg. "wasmer --backend=singlepass")
pub fn get_runtime_with_args() -> (String, Vec<String>) {
    split_runtime_and_args(get_runtime())
}

#[cfg(not(target_os = "wasi"))]
pub fn create_temp_dir() -> Result<tempfile::TempDir, std::io::Error> {
    tempfile::TempDir::new()
}

#[cfg(target_os = "wasi")]
pub fn create_temp_dir() -> Result<std::path::PathBuf, std::io::Error> {
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf)?;
    let path = format!("/tmp/{:#10x}", u32::from_be_bytes(buf));
    let ret: std::path::PathBuf = path.into();
    Ok(ret)
}

#[cfg(target_os = "wasi")]
pub fn whoami_distro() -> String {
    whoami::os().to_lowercase()
}

#[cfg(not(target_os = "wasi"))]
pub fn whoami_distro() -> String {
    whoami::distro().to_lowercase()
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(feature = "update-notifications")]
    #[test]
    pub fn compare_ver_test() {
        use super::VersionComparison::*;
        assert_eq!(compare_versions("0.1.0", "0.1.0").unwrap(), NewIsEqual);
        assert_eq!(compare_versions("1.1.0", "0.1.0").unwrap(), NewIsLesser);
        assert_eq!(compare_versions("1.0.0", "0.2.5").unwrap(), NewIsLesser);
        assert_eq!(compare_versions("1.0.0", "2.2.5").unwrap(), NewIsGreater);
        assert_eq!(compare_versions("1.0.0", "2.0.5").unwrap(), NewIsGreater);
        assert_eq!(compare_versions("1.1.0", "2.0.5").unwrap(), NewIsGreater);
        assert_eq!(compare_versions("1.1.6", "2.0.0").unwrap(), NewIsGreater);
        assert_eq!(compare_versions("0.1.1", "0.1.0").unwrap(), NewIsLesser);
        assert_eq!(compare_versions("0.1.1", "0.2.0").unwrap(), NewIsGreater);

        assert_eq!(compare_versions("v0.1.0", "v0.1.0").unwrap(), NewIsEqual);
        assert_eq!(compare_versions("v1.1.0", "v0.1.0").unwrap(), NewIsLesser);
        assert_eq!(compare_versions("v1.1.6", "v2.0.0").unwrap(), NewIsGreater);

        assert_eq!(compare_versions("0.1.0", "v0.1.0").unwrap(), NewIsEqual);
        assert_eq!(compare_versions("1.1.0", "v0.1.0").unwrap(), NewIsLesser);
        assert_eq!(compare_versions("1.1.6", "v2.0.0").unwrap(), NewIsGreater);

        assert_eq!(compare_versions("v0.1.0", "0.1.0").unwrap(), NewIsEqual);
        assert_eq!(compare_versions("v1.1.0", "0.1.0").unwrap(), NewIsLesser);
        assert_eq!(compare_versions("v1.1.6", "2.0.0").unwrap(), NewIsGreater);
        assert_eq!(compare_versions("3.0.2", "v3.0.2").unwrap(), NewIsEqual);
    }

    #[test]
    pub fn test_split_runtime_and_args() {
        assert_eq!(
            split_runtime_and_args("wasmer".to_owned()),
            ("wasmer".to_owned(), vec![])
        );
        assert_eq!(
            split_runtime_and_args("wasmer --backend=llvm".to_owned()),
            ("wasmer".to_owned(), vec!["--backend=llvm".to_owned()])
        );
        assert_eq!(
            split_runtime_and_args("wasmer run".to_owned()),
            ("wasmer".to_owned(), vec!["run".to_owned()])
        );

        // Weird spacing
        assert_eq!(
            split_runtime_and_args("  wasmer   ".to_owned()),
            ("wasmer".to_owned(), vec![])
        );
        assert_eq!(
            split_runtime_and_args("wasmer  run  ".to_owned()),
            ("wasmer".to_owned(), vec!["run".to_owned()])
        );
        assert_eq!(
            split_runtime_and_args("  wasmer  run  ".to_owned()),
            ("wasmer".to_owned(), vec!["run".to_owned()])
        );
    }
}
