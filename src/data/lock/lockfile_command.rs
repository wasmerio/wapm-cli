use crate::data::manifest::Command;
use semver::Version;
use thiserror::Error;

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileCommand {
    pub name: String,
    pub package_name: String,
    pub package_version: Version,
    pub module: String,
    pub is_top_level_dependency: bool,
    pub main_args: Option<String>,
}

impl<'a> LockfileCommand {
    pub fn from_command(
        local_package_name: &str,
        local_package_version: Version,
        command: &'a Command,
    ) -> Result<Self, Error> {
        // split the "package" field of the command if it exists
        // otherwise assume that this is a command for a local module
        // extract the package name and version for this command and insert into the lockfile command
        let package = command.get_package();
        let (package_name, package_version): (&str, Version) = match &package {
            Some(package_string) => {
                let split = package_string.as_str().split(' ').collect::<Vec<_>>();
                match &split[..] {
                    [package_name, package_version] => {
                        // this string must be parsed again because the package field on a command is a concatenated string
                        // e.g. "_/pkg 1.0.0"
                        let package_version = Version::parse(package_version).unwrap();
                        (package_name, package_version)
                    },
                    [package_name] => {
                        (package_name, local_package_version.clone())
                    },
                    _ => {
                        return Err(Error::CouldNotParsePackageVersionForCommand(
                            package_string.clone(),
                            command.get_name(),
                        ));
                    }
                }
            }
            None => (local_package_name, local_package_version),
        };

        let lockfile_command = LockfileCommand {
            name: command.get_name(),
            package_name: package_name.to_string(),
            package_version,
            module: command.get_module(),
            main_args: command.get_main_args(),
            is_top_level_dependency: true,
        };
        Ok(lockfile_command)
    }
}

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
    #[error("Could not parse the package name and version \"{0}\" for the command \"{1}\".")]
    CouldNotParsePackageVersionForCommand(String, String),
}
