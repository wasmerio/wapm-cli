use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::cfg_toml::lock::{LOCKFILE_HEADER, LOCKFILE_NAME};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::Path;

pub type ModuleMap<'a> =
    BTreeMap<Cow<'a, str>, BTreeMap<Cow<'a, str>, BTreeMap<Cow<'a, str>, LockfileModule<'a>>>>;
pub type CommandMap<'a> = BTreeMap<Cow<'a, str>, LockfileCommand<'a>>;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Lockfile<'a> {
    #[serde(borrow = "'a")]
    pub modules: ModuleMap<'a>, // PackageName -> VersionNumber -> ModuleName -> Module
    #[serde(borrow = "'a")]
    pub commands: CommandMap<'a>, // CommandName -> Command
}

impl<'a> Lockfile<'a> {
    pub fn open<P: AsRef<Path>>(
        directory: P,
        lockfile_string: &'a mut String,
    ) -> Result<Lockfile<'a>, LockfileError> {
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut lockfile_file =
            File::open(lockfile_path).map_err(|_| LockfileError::MissingLockfile)?;
        lockfile_file
            .read_to_string(lockfile_string)
            .map_err(|e| LockfileError::FileIoErrorReadingLockfile(e))?;
        toml::from_str(lockfile_string.as_str()).map_err(|e| LockfileError::TomlParseError(e))
    }

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
        package_name: Cow<'a, str>,
        package_version: Cow<'a, str>,
        module_name: Cow<'a, str>,
    ) -> Result<&LockfileModule, failure::Error> {
        let version_map = self.modules.get(&package_name).ok_or::<failure::Error>(
            LockfileError::PackageWithVersionNotFoundWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module_map = version_map.get(&package_version).ok_or::<failure::Error>(
            LockfileError::VersionNotFoundForPackageWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module = module_map.get(&module_name).ok_or::<failure::Error>(
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
