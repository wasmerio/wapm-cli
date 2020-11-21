use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::{
    LockfileModule, LockfileModuleV2, LockfileModuleV3, LockfileModuleV4,
};
use crate::data::lock::{LOCKFILE_HEADER, LOCKFILE_NAME};
use semver::Version;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

pub type ModuleMapV2 = BTreeMap<String, BTreeMap<Version, BTreeMap<String, LockfileModuleV2>>>;
pub type CommandMapV2 = BTreeMap<String, LockfileCommand>;

/// The lockfile for versions 2 and below (no changes to the fields happened until version 3,
/// so these can be a singel struct)
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileV2 {
    pub modules: ModuleMapV2, // PackageName -> VersionNumber -> ModuleName -> Module
    pub commands: CommandMapV2, // CommandName -> Command
}

pub type ModuleMapV3 = BTreeMap<String, BTreeMap<Version, BTreeMap<String, LockfileModuleV3>>>;
pub type CommandMapV3 = BTreeMap<String, LockfileCommand>;

/// The latest Lockfile version
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileV3 {
    pub modules: ModuleMapV3, // PackageName -> VersionNumber -> ModuleName -> Module
    pub commands: CommandMapV3, // CommandName -> Command
}

pub type ModuleMap = BTreeMap<String, BTreeMap<Version, BTreeMap<String, LockfileModuleV4>>>;
pub type CommandMap = BTreeMap<String, LockfileCommand>;

/// The latest Lockfile version
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Lockfile {
    pub modules: ModuleMap, // PackageName -> VersionNumber -> ModuleName -> Module
    pub commands: CommandMap, // CommandName -> Command
}

pub type LockfileV4 = Lockfile;
pub type ModuleMapV4 = ModuleMap;
pub type CommandMapV4 = CommandMap;

impl<'a> Lockfile {
    /// Save the lockfile to the directory.
    pub fn save<P: AsRef<Path>>(&self, directory: P) -> anyhow::Result<()> {
        let lockfile_string = toml::to_string(self)?;
        let lockfile_string = format!("{}\n{}", LOCKFILE_HEADER, lockfile_string);
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut file = File::create(&lockfile_path)?;
        file.write_all(lockfile_string.as_bytes())?;
        Ok(())
    }

    /// Looks up the prehashed cache key based on data in the Command
    pub fn get_prehashed_cache_key_from_command(
        &self,
        command: &LockfileCommand,
    ) -> Option<String> {
        self.modules
            .get(&command.package_name)
            .and_then(|version_map| version_map.get(&command.package_version))
            .and_then(|module_map| module_map.get(&command.module))
            .and_then(|module| module.prehashed_module_key.clone())
    }

    pub fn get_command(&self, command_name: &str) -> Result<&LockfileCommand, LockfileError> {
        self.commands
            .get(command_name)
            .ok_or(LockfileError::CommandNotFound(command_name.to_string()).into())
    }

    pub fn get_module(
        &self,
        package_name: &str,
        package_version: &Version,
        module_name: &str,
    ) -> anyhow::Result<&LockfileModule> {
        let version_map = self.modules.get(package_name).ok_or::<anyhow::Error>(
            LockfileError::PackageWithVersionNotFoundWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module_map = version_map.get(package_version).ok_or::<anyhow::Error>(
            LockfileError::VersionNotFoundForPackageWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module = module_map.get(module_name).ok_or::<anyhow::Error>(
            LockfileError::ModuleForPackageVersionNotFound(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        Ok(module)
    }
}

#[derive(Debug, Error)]
pub enum LockfileError {
    #[error("Command not found: {0}")]
    CommandNotFound(String),
    #[error("module {2} in package \"{0} {1}\" was not found")]
    ModuleForPackageVersionNotFound(String, String, String),
    #[error(
        "Module \"{2}\" with package name \"{0}\" and version \"{1}\" was not found.",
    )]
    PackageWithVersionNotFoundWhenFindingModule(String, String, String),
    #[error(
        "version \"{1}\" for package \"{0}\" was not found when searching for module \"{2}\".",
    )]
    VersionNotFoundForPackageWhenFindingModule(String, String, String),
    #[error("Lockfile file not found.")]
    MissingLockfile,
    #[error("File I/O error reading lockfile. I/O error: {0:?}")]
    FileIoErrorReadingLockfile(io::Error),
    #[error(
        "Failed to parse lockfile toml. Did you modify the generated lockfile? Toml error: {0:?}",
    )]
    TomlParseError(toml::de::Error),
}
