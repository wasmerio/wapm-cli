use crate::manifest::Command;
use crate::dependency_resolver::Dependency;

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileCommand<'a> {
    pub name: &'a str,
    pub package_name: &'a str,
    pub package_version: &'a str,
    pub module: &'a str,
    pub is_top_level_dependency: bool,
    pub emscripten_arguments: Option<String>,
}

impl<'a> LockfileCommand<'a> {
    pub fn from_command(package_name: &'a str, package_version: &'a str, command: &'a Command) -> Self {
        let lockfile_command = LockfileCommand {
            name: command.name.as_str(),
            package_name,
            package_version,
            module: command.module.as_str(),
            emscripten_arguments: command.emscripten_call_arguments.clone(),
            is_top_level_dependency: true,
        };
        lockfile_command
    }

    pub fn from_dependency(dependency: &'a Dependency) -> Result<Vec<Self>, failure::Error> {
        if let None = dependency.manifest.command {
            return Ok(vec![]);
        }
        let commands = dependency.manifest.command.as_ref().unwrap().iter().map(|c| {
            let package_name = dependency.manifest.package.name.as_str();
            let package_version = dependency.manifest.package.version.as_str();
            LockfileCommand::from_command(package_name, package_version, &c)
        }).collect::<Vec<_>>();
        Ok(commands)
    }
}

#[derive(Debug, Fail)]
pub enum LockfileCommandError {
    #[fail(display = "The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
}
