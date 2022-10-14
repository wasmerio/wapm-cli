use crate::data::manifest::Command;
use semver::Version;
use thiserror::Error;

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LockfileCommand {
    pub name: String,
    pub package_name: String,
    pub package_version: Version,
    pub module: String,
    pub is_top_level_dependency: bool,
    pub main_args: Option<String>,
}

#[test]
fn test_lockfile_command_ok() {
    use wapm_toml::CommandV1;
    assert_eq!(
        LockfileCommand::from_command(
            "Micheal-F-Bryan/wit-pack",
            Version::new(1, 0, 0),
            &Command::V1(CommandV1 {
                name: "wit-pack".to_string(),
                module: "wit-pack".to_string(),
                main_args: None,
                package: None,
            })
        ),
        Ok(LockfileCommand {
            name: "wit-pack".to_string(),
            package_name: "Micheal-F-Bryan/wit-pack".to_string(),
            package_version: Version::new(1, 0, 0),
            module: "wit-pack".to_string(),
            is_top_level_dependency: true,
            main_args: None,
        })
    )
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
                    }
                    [package_name] => (package_name, local_package_version),
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

#[derive(Clone, PartialEq, Eq, Debug, Error)]
pub enum Error {
    #[error("The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
    #[error("Could not parse the package name and version \"{0}\" for the command \"{1}\".")]
    CouldNotParsePackageVersionForCommand(String, String),
}
