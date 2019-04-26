use crate::data::lock::lockfile::Lockfile;
use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::LockfileModule;
use crate::data::lock::LOCKFILE_NAME;
use crate::dataflow::installed_packages::InstalledPackages;
use crate::dataflow::PackageKey;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Fail)]
pub enum LockfileError {
    #[fail(display = "Could not parse lockfile because {}.", _0)]
    LockfileTomlParseError(String),
    #[fail(display = "Could not parse lockfile because {}.", _0)]
    IoError(String),
}

/// A ternary for a lockfile: Some, None, Error.
#[derive(Debug)]
pub enum LockfileResult {
    Lockfile(Lockfile),
    NoLockfile,
    LockfileError(LockfileError),
}

impl LockfileResult {
    pub fn find_in_directory<P: AsRef<Path>>(directory: P) -> Self {
        let directory = directory.as_ref();
        if !directory.is_dir() {
            LockfileResult::LockfileError(LockfileError::IoError(
                "Manifest must be a file named `wapm.toml`.".to_string(),
            ));
        }
        let lockfile_path_buf = directory.join(LOCKFILE_NAME);
        if !lockfile_path_buf.is_file() {
            LockfileResult::LockfileError(LockfileError::IoError(
                "Manifest must be a file named `wapm.toml`.".to_string(),
            ));
        }
        let source = match fs::read_to_string(&lockfile_path_buf) {
            Ok(s) => s,
            Err(_) => return LockfileResult::NoLockfile,
        };
        match toml::from_str::<Lockfile>(&source) {
            Ok(l) => LockfileResult::Lockfile(l),
            Err(e) => {
                LockfileResult::LockfileError(LockfileError::LockfileTomlParseError(e.to_string()))
            }
        }
    }
}

impl Default for LockfileResult {
    fn default() -> Self {
        LockfileResult::NoLockfile
    }
}

/// A convenient structure containing all modules and commands for a package stored lockfile.
#[derive(Clone, Debug, Default)]
pub struct LockfilePackage {
    pub modules: Vec<LockfileModule>,
    pub commands: Vec<LockfileCommand>,
}

/// A wrapper around a map of key -> lockfile package.
#[derive(Clone, Debug, Default)]
pub struct LockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage>,
}

impl<'a> LockfilePackages<'a> {
    pub fn from_installed_packages(installed_manifest_packages: &'a InstalledPackages<'a>) -> Self {
        let packages: HashMap<PackageKey<'a>, LockfilePackage> = installed_manifest_packages
            .packages
            .iter()
            .map(|(k, m, download_url)| {
                let modules: Vec<LockfileModule> = match m.module {
                    Some(ref modules) => modules
                        .iter()
                        .map(|m| {
                            LockfileModule::from_module(
                                k.name.as_ref(),
                                &k.version,
                                m,
                                download_url,
                            )
                        })
                        .collect(),
                    _ => vec![],
                };
                let commands: Vec<LockfileCommand> = match m.command {
                    Some(ref modules) => modules
                        .iter()
                        .map(|c| LockfileCommand::from_command(&k.name, k.version.clone(), c))
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

    pub fn new_from_result(result: LockfileResult) -> Result<Self, LockfileError> {
        match result {
            LockfileResult::Lockfile(l) => Ok(Self::new_from_lockfile(l)),
            LockfileResult::NoLockfile => Ok(Self {
                packages: HashMap::new(),
            }),
            LockfileResult::LockfileError(e) => return Err(e),
        }
    }

    fn new_from_lockfile(lockfile: Lockfile) -> LockfilePackages<'a> {
        let (raw_lockfile_modules, raw_lockfile_commands) = (lockfile.modules, lockfile.commands);

        let mut lockfile_commands_map: HashMap<PackageKey, Vec<LockfileCommand>> = HashMap::new();
        for (_name, command) in raw_lockfile_commands {
            let command: LockfileCommand = command;
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

    pub fn find_missing_packages<P: AsRef<Path>>(&self, directory: P) -> HashSet<PackageKey<'a>> {
        let missing_packages: HashSet<PackageKey<'a>> = self
            .packages
            .iter()
            .filter_map(|(key, data)| {
                if data.modules.iter().any(|module| {
                    let path = directory.as_ref().join(&module.entry);
                    !path.exists()
                }) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();
        missing_packages
    }

    pub fn extend(&mut self, other_packages: LockfilePackages<'a>) {
        self.packages.extend(other_packages.packages);
    }
}
