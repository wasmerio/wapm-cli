use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::LockfileModule;
use crate::data::lock::{LOCKFILE_HEADER, LOCKFILE_NAME};
use semver::Version;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;

pub type ModuleMap = BTreeMap<String, BTreeMap<Version, BTreeMap<String, LockfileModule>>>;
pub type CommandMap = BTreeMap<String, LockfileCommand>;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Lockfile {
    pub modules: ModuleMap, // PackageName -> VersionNumber -> ModuleName -> Module
    pub commands: CommandMap, // CommandName -> Command
}

impl<'a> Lockfile {
    /// Save the lockfile to the directory.
    pub fn save<P: AsRef<Path>>(&self, directory: P) -> Result<(), failure::Error> {
        let lockfile_string = toml::to_string(self)?;
        let lockfile_string = format!("{}\n{}", LOCKFILE_HEADER, lockfile_string);
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut file = File::create(&lockfile_path)?;
        file.write_all(lockfile_string.as_bytes())?;
        Ok(())
    }

    pub fn get_command(&self, command_name: &str) -> Result<&LockfileCommand, failure::Error> {
        self.commands
            .get(command_name)
            .ok_or(LockfileError::CommandNotFound(command_name.to_string()).into())
    }

    pub fn get_module(
        &self,
        package_name: &str,
        package_version: &Version,
        module_name: &str,
    ) -> Result<&LockfileModule, failure::Error> {
        let version_map = self.modules.get(package_name).ok_or::<failure::Error>(
            LockfileError::PackageWithVersionNotFoundWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module_map = version_map.get(package_version).ok_or::<failure::Error>(
            LockfileError::VersionNotFoundForPackageWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module = module_map.get(module_name).ok_or::<failure::Error>(
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

#[derive(Debug, Fail)]
pub enum LockfileError {
    #[fail(display = "Command not found: {}", _0)]
    CommandNotFound(String),
    #[fail(display = "module {} in package \"{} {}\" was not found", _2, _0, _1)]
    ModuleForPackageVersionNotFound(String, String, String),
    #[fail(
        display = "Package \"{}\" with version \"{}\" was nto found searching for module \"{}\"",
        _0, _1, _2
    )]
    PackageWithVersionNotFoundWhenFindingModule(String, String, String),
    #[fail(
        display = "version \"{}\" for package \"{}\" was not found when searching for module \"{}\".",
        _1, _0, _2
    )]
    VersionNotFoundForPackageWhenFindingModule(String, String, String),
    #[fail(display = "Lockfile file not found.")]
    MissingLockfile,
    #[fail(display = "File I/O error reading lockfile. I/O error: {:?}", _0)]
    FileIoErrorReadingLockfile(io::Error),
    #[fail(
        display = "Failed to parse lockfile toml. Did you modify the generated lockfile? Toml error: {:?}",
        _0
    )]
    TomlParseError(toml::de::Error),
}
