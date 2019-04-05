use crate::manifest::Command;

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileCommand {
    pub module: String,
    emscripten_arguments: Option<String>,
}

impl LockfileCommand {
    pub fn from_command(module: &str, command: &Command) -> Self {
        let lockfile_command = LockfileCommand {
            module: module.to_string(),
            emscripten_arguments: command.emscripten_call_arguments.clone(),
        };
        lockfile_command
    }
}

#[derive(Debug, Fail)]
pub enum LockfileCommandError {
    #[fail(display = "The module for this command does not exist. Did you modify the wapm.lock?")]
    ModuleForCommandDoesNotExist,
}
