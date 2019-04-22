use crate::bonjour::{
    BonjourError, InstalledManifestPackages, LockfilePackage, PackageKey, WapmPackageKey,
};
use crate::cfg_toml::lock::lockfile::{CommandMap, Lockfile, ModuleMap};
use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::cfg_toml::lock::LOCKFILE_NAME;
use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::fs;
use std::path::Path;

pub struct LockfileSource {
    source: Option<String>,
}

impl LockfileSource {
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

#[derive(Debug)]
pub enum LockfileResult<'a> {
    Lockfile(Lockfile<'a>),
    NoLockfile,
    LockfileError(BonjourError),
}

impl<'a> LockfileResult<'a> {
    pub fn from_source(source: &'a LockfileSource) -> LockfileResult {
        source
            .source
            .as_ref()
            .map(|source| match toml::from_str::<Lockfile>(source) {
                Ok(l) => LockfileResult::Lockfile(l),
                Err(e) => LockfileResult::LockfileError(BonjourError::LockfileTomlParseError(
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

#[derive(Clone, Debug)]
pub struct LockfileData<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage<'a>>,
}

impl<'a> LockfileData<'a> {
    pub fn merge(self, mut other: LockfileData<'a>) -> LockfileData {
        let keys: HashSet<_> = self.packages.keys().cloned().collect();
        let other_keys: HashSet<_> = other.packages.keys().cloned().collect();
        let removed_keys: Vec<_> = other_keys.difference(&keys).collect();
        for removed_key in removed_keys {
            other.packages.remove(removed_key);
        }
        for (key, data) in self.packages {
            other.packages.insert(key, data);
        }
        other
    }

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

    pub fn new_from_result(result: LockfileResult<'a>) -> Result<Self, BonjourError> {
        match result {
            LockfileResult::Lockfile(l) => Ok(Self::new_from_lockfile(l)),
            LockfileResult::NoLockfile => Ok(Self {
                packages: HashMap::new(),
            }),
            LockfileResult::LockfileError(e) => return Err(e),
        }
    }

    fn new_from_lockfile(lockfile: Lockfile<'a>) -> LockfileData {
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

    pub fn generate_lockfile(self, directory: &'a Path) -> Result<(), BonjourError> {
        let mut modules: ModuleMap<'a> = BTreeMap::new();
        let mut commands: CommandMap<'a> = BTreeMap::new();
        for (key, package) in self.packages {
            match key {
                PackageKey::WapmPackage(WapmPackageKey { name, version }) => {
                    let versions = modules.entry(name).or_default();
                    let modules = versions.entry(version).or_default();
                    for module in package.modules {
                        let name = module.name.clone();
                        modules.insert(name, module);
                    }
                    for command in package.commands {
                        let name = command.name.clone();
                        commands.insert(name, command);
                    }
                }
                PackageKey::LocalPackage { .. } => panic!("Local packages are not supported yet."),
                PackageKey::GitUrl { .. } => panic!("Git url packages are not supported yet."),
            }
        }

        let lockfile = Lockfile { modules, commands };

        lockfile
            .save(directory)
            .map_err(|e| BonjourError::InstallError(e.to_string()))?;
        Ok(())
    }
}
