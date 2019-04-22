use crate::dataflow::installed_manifest_packages::InstalledManifestPackages;
use crate::dataflow::{Error, PackageKey};
use crate::cfg_toml::lock::lockfile::{Lockfile};
use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::cfg_toml::lock::LOCKFILE_NAME;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::fs;
use std::path::Path;

/// A wrapper type around an optional source string.
pub struct LockfileSource {
    source: Option<String>,
}

impl LockfileSource {
    /// Will contain a Some of the file is found and readable.
    /// Unable to read the file will result in None.
    pub fn new<P: AsRef<Path>>(directory: P) -> Self {
        let directory = directory.as_ref();
        if !directory.is_dir() {
            return Self { source: None };
        }
        let lockfile_path_buf = directory.join(LOCKFILE_NAME);
        let source = fs::read_to_string(&lockfile_path_buf).ok();
        Self { source }
    }
}

/// A ternary for a lockfile: Some, None, Error.
#[derive(Debug)]
pub enum LockfileResult<'a> {
    Lockfile(Lockfile<'a>),
    NoLockfile,
    LockfileError(Error),
}

impl<'a> LockfileResult<'a> {
    pub fn from_source(source: &'a LockfileSource) -> LockfileResult {
        source
            .source
            .as_ref()
            .map(|source| match toml::from_str::<Lockfile>(source) {
                Ok(l) => LockfileResult::Lockfile(l),
                Err(e) => LockfileResult::LockfileError(Error::LockfileTomlParseError(
                    e.to_string(),
                )),
            })
            .unwrap_or(LockfileResult::NoLockfile)
    }
}

impl<'a> Default for LockfileResult<'a> {
    fn default() -> Self {
        LockfileResult::NoLockfile
    }
}

/// A convenient structure containing all modules and commands for a package stored lockfile.
#[derive(Clone, Debug)]
pub struct LockfilePackage<'a> {
    pub modules: Vec<LockfileModule<'a>>,
    pub commands: Vec<LockfileCommand<'a>>,
}

/// A wrapper around a map of key -> lockfile package.
#[derive(Clone, Debug)]
pub struct LockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage<'a>>,
}

impl<'a> LockfilePackages<'a> {
    pub fn from_installed_packages(
        installed_manifest_packages: &'a InstalledManifestPackages<'a>,
    ) -> Self {
        let packages: HashMap<PackageKey<'a>, LockfilePackage<'a>> = installed_manifest_packages
            .packages
            .iter()
            .map(|(k, m, download_url)| {
                let modules: Vec<LockfileModule> = match m.module {
                    Some(ref modules) => modules
                        .iter()
                        .map(|m| {
                            LockfileModule::from_module(
                                k.name.clone(),
                                k.version.clone(),
                                m,
                                std::borrow::Cow::Borrowed(download_url),
                            )
                        })
                        .collect(),
                    _ => vec![],
                };
                let commands: Vec<LockfileCommand> = match m.command {
                    Some(ref modules) => modules
                        .iter()
                        .map(|c| {
                            LockfileCommand::from_command(k.name.clone(), k.version.clone(), c)
                        })
                        .collect(),
                    _ => vec![],
                };
                (
                    PackageKey::WapmPackage(k.clone()),
                    LockfilePackage { modules, commands },
                )
            })
            .collect();
        Self { packages }
    }

    pub fn new_from_result(result: LockfileResult<'a>) -> Result<Self, Error> {
        match result {
            LockfileResult::Lockfile(l) => Ok(Self::new_from_lockfile(l)),
            LockfileResult::NoLockfile => Ok(Self {
                packages: HashMap::new(),
            }),
            LockfileResult::LockfileError(e) => return Err(e),
        }
    }

    fn new_from_lockfile(lockfile: Lockfile<'a>) -> LockfilePackages {
        let (raw_lockfile_modules, raw_lockfile_commands) = (lockfile.modules, lockfile.commands);

        let mut lockfile_commands_map: HashMap<PackageKey, Vec<LockfileCommand>> = HashMap::new();
        for (_name, command) in raw_lockfile_commands {
            let command: LockfileCommand<'a> = command;
            let id = PackageKey::new_registry_package(
                command.package_name.clone(),
                command.package_version.clone(),
            );
            let command_vec = lockfile_commands_map.entry(id).or_default();
            command_vec.push(command);
        }

        let packages: HashMap<PackageKey, LockfilePackage> = raw_lockfile_modules
            .into_iter()
            .map(|(pkg_name, pkg_versions)| {
                pkg_versions
                    .into_iter()
                    .map(|(pkg_version, modules)| {
                        let id =
                            PackageKey::new_registry_package(pkg_name.clone(), pkg_version.clone());
                        let lockfile_modules = modules
                            .into_iter()
                            .map(|(_module_name, module)| module)
                            .collect::<Vec<_>>();
                        let lockfile_commands = lockfile_commands_map.remove(&id).unwrap_or(vec![]);
                        let package_data = LockfilePackage {
                            modules: lockfile_modules,
                            commands: lockfile_commands,
                        };
                        (id, package_data)
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<HashMap<_, _>>();

        Self { packages }
    }

    pub fn package_keys(&self) -> HashSet<PackageKey<'a>> {
        self.packages.keys().cloned().collect()
    }
}
