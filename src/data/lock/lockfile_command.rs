use crate::data::manifest::Command;
use semver::Version;

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
        let (package_name, package_version): (&str, Version) = match &command.package {
            Some(package_string) => {
                let split = package_string.as_str().split(' ').collect::<Vec<_>>();
                match &split[..] {
                    [package_name, package_version] => {
                        // this string must be parsed again because the package field on a command is a concatenated string
                        // e.g. "_/pkg 1.0.0"
                        let package_version = Version::parse(package_version).unwrap();
                        (package_name, package_version)
                    }
                    _ => {
                        return Err(Error::CouldNotParsePackageVersionForCommand(
                            package_string.clone(),
                            command.name.clone(),
                        ));
                    }
                }
            }
            None => (local_package_name, local_package_version),
        };

        let lockfile_command = LockfileCommand {
            name: command.name.to_string(),
            package_name: package_name.to_string(),
            package_version,
            module: command.module.to_string(),
            main_args: command.main_args.clone(),
            is_top_level_dependency: true,
        };
        Ok(lockfile_command)
    }
}

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
    #[fail(
        display = "Could not parse the package name and version \"{}\" for the command \"{}\".",
        _0, _1
    )]
    CouldNotParsePackageVersionForCommand(String, String),
}
