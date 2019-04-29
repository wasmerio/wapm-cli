use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::graphql::execute_query;
use graphql_client::*;
use semver::Version;
use std::path::{Path, PathBuf};
use std::{fs, io};

pub static MAX_PACKAGE_NAME_LENGTH: usize = 50;

#[derive(Debug, Fail)]
pub enum PackageNameError {
    #[fail(
        display = "Package name, \"{}\", is too long, name must be {} characters or fewer",
        _0, _1
    )]
    NameTooLong(String, usize),
    #[fail(
        display = "Package name, \"{}\", contains invalid characters.  Please use alpha-numeric characters, '-', and '_'",
        _0
    )]
    InvalidCharacters(String),
}

/// Checks whether a given package name is acceptable or not
pub fn validate_package_name(package_name: &str) -> Result<(), PackageNameError> {
    if package_name.len() > MAX_PACKAGE_NAME_LENGTH {
        return Err(PackageNameError::NameTooLong(
            package_name.to_string(),
            MAX_PACKAGE_NAME_LENGTH,
        ));
    }

    let re = regex::Regex::new("^[-a-zA-Z0-9_]+").unwrap();

    if !re.is_match(package_name) {
        return Err(PackageNameError::InvalidCharacters(
            package_name.to_string(),
        ));
    }

    Ok(())
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

pub fn create_package_dir<P: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P,
    namespace_dir: P2,
    fully_qualified_package_name: &str,
) -> Result<PathBuf, io::Error> {
    let mut package_dir = project_dir.as_ref().join(PACKAGES_DIR_NAME);
    package_dir.push(namespace_dir);
    package_dir.push(&fully_qualified_package_name);
    fs::create_dir_all(&package_dir)?;
    Ok(package_dir)
}
