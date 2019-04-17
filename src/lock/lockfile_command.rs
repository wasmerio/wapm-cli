use crate::dependency_resolver::Dependency;
use crate::manifest::Command;

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileCommand<'a> {
    pub name: &'a str,
    pub package_name: &'a str,
    pub package_version: &'a str,
    pub module: &'a str,
    pub is_top_level_dependency: bool,
    pub main_args: Option<&'a str>,
}

impl<'a> LockfileCommand<'a> {
    pub fn from_command(
        local_package_name: &'a str,
        local_package_version: &'a str,
        command: &'a Command,
    ) -> Self {
        // split the "package" field of the command if it exists
        // otherwise assume that this is a command for a local module
        // extract the package name and version for this command and insert into the lockfile command
        let (package_name, package_version): (&'a str, &'a str) = match &command.package {
            Some(package_string) => {
                let split = package_string.as_str().split(' ').collect::<Vec<_>>();
                match &split[..] {
                    [package_name, package_version] => (package_name, package_version),
                    _ => {
                        panic!("invalid package name: {}", package_string);
                    }
                }
            }
            None => (local_package_name, local_package_version),
        };

        let lockfile_command = LockfileCommand {
            name: command.name.as_str(),
            package_name,
            package_version,
            module: command.module.as_str(),
            main_args: command.main_args.as_ref().map(|s| s.as_str()).clone(),
            is_top_level_dependency: true,
        };
        lockfile_command
    }

    pub fn from_dependency(dependency: &'a Dependency) -> Result<Vec<Self>, failure::Error> {
        if let None = dependency.manifest.command {
            return Ok(vec![]);
        }
        let commands = dependency
            .manifest
            .command
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| {
                let package_name = dependency.manifest.package.name.as_str();
                let package_version = dependency.manifest.package.version.as_str();
                LockfileCommand::from_command(package_name, package_version, &c)
            })
            .collect::<Vec<_>>();
        Ok(commands)
    }
}

#[derive(Debug, Fail)]
pub enum LockfileCommandError {
    #[fail(display = "The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
}
