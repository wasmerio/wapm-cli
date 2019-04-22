use crate::cfg_toml::manifest::Command;
use std::borrow::Cow;

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileCommand<'a> {
    pub name: Cow<'a, str>,
    pub package_name: Cow<'a, str>,
    pub package_version: Cow<'a, str>,
    pub module: Cow<'a, str>,
    pub is_top_level_dependency: bool,
    pub main_args: Option<&'a str>,
}

impl<'a> LockfileCommand<'a> {
    pub fn from_command(
        local_package_name: Cow<'a, str>,
        local_package_version: Cow<'a, str>,
        command: &'a Command,
    ) -> Self {
        // split the "package" field of the command if it exists
        // otherwise assume that this is a command for a local module
        // extract the package name and version for this command and insert into the lockfile command
        let (package_name, package_version): (Cow<'a, str>, Cow<'a, str>) = match &command.package {
            Some(package_string) => {
                let split = package_string.as_str().split(' ').collect::<Vec<_>>();
                match &split[..] {
                    [package_name, package_version] => {
                        (Cow::Borrowed(package_name), Cow::Borrowed(package_version))
                    }
                    _ => {
                        panic!("invalid package name: {}", package_string);
                    }
                }
            }
            None => (local_package_name, local_package_version),
        };

        let lockfile_command = LockfileCommand {
            name: Cow::Borrowed(command.name.as_str()),
            package_name,
            package_version,
            module: Cow::Borrowed(command.module.as_str()),
            main_args: command.main_args.as_ref().map(|s| s.as_str()).clone(),
            is_top_level_dependency: true,
        };
        lockfile_command
    }
}

#[derive(Debug, Fail)]
pub enum LockfileCommandError {
    #[fail(display = "The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
}