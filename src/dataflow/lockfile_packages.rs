use crate::data::lock::lockfile::Lockfile;
use crate::data::lock::lockfile_command::{Error, LockfileCommand};
use crate::data::lock::lockfile_module::LockfileModule;
use crate::data::lock::migrate::{
    convert_lockfilev2_to_v3, convert_lockfilev3_to_v4, fix_up_v1_package_names, LockfileVersion,
};
use crate::data::lock::LOCKFILE_NAME;
use crate::dataflow::installed_packages::InstalledPackages;
use crate::dataflow::removed_packages::RemovedPackages;
use crate::dataflow::{PackageKey, WapmPackageKey};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum LockfileError {
    #[error("Could not parse lockfile because {0}.")]
    LockfileTomlParseError(String),
    #[error("Could not parse lockfile because {0}.")]
    IoError(String),
    #[error("Could not parse lockfile because of issue parsing command. {0}")]
    CommandPackageVersionParseError(Error),
    #[error("Lockfile version is missing or invalid. Delete `wapm.lock`.")]
    InvalidOrMissingVersion,
    #[error("Lockfile version is too high, update wapm or delete `wapm.lock` and try again.")]
    VersionTooHigh,
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
            return LockfileResult::LockfileError(LockfileError::IoError(
                format!("LockfileResult: Manifest must be a file named `wapm.toml` (directory.is_dir() failed on {})", directory.display()),
            ));
        }
        let lockfile_path_buf = directory.join(LOCKFILE_NAME);
        if !lockfile_path_buf.is_file() {
            return LockfileResult::LockfileError(LockfileError::IoError(
                format!("LockfileResult: Manifest must be a file named `wapm.toml` (lockfile_path_buf.is_file() failed on {})", lockfile_path_buf.display()),
            ));
        }
        let source = match fs::read_to_string(&lockfile_path_buf) {
            Ok(s) => s,
            Err(_) => return LockfileResult::NoLockfile,
        };
        let mut lockfile_version = match LockfileVersion::from_lockfile_string(&source) {
            Ok(lv) => lv,
            Err(e) => return LockfileResult::LockfileError(e),
        };
        loop {
            lockfile_version = match lockfile_version {
                LockfileVersion::V1(mut lockfile_v1) => {
                    fix_up_v1_package_names(&mut lockfile_v1);
                    LockfileVersion::V2(lockfile_v1)
                }
                LockfileVersion::V2(lockfile_v2) => {
                    LockfileVersion::V3(convert_lockfilev2_to_v3(lockfile_v2))
                }
                LockfileVersion::V3(lockfile_v3) => {
                    LockfileVersion::V4(convert_lockfilev3_to_v4(lockfile_v3, directory))
                }
                LockfileVersion::V4(lockfile_v4) => return LockfileResult::Lockfile(lockfile_v4),
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
    pub fn from_installed_packages(
        installed_manifest_packages: &'a InstalledPackages<'a>,
    ) -> Result<Self, LockfileError> {
        let mut packages = HashMap::default();
        for (k, manifest, download_url) in installed_manifest_packages.packages.iter() {
            let modules: Vec<LockfileModule> = match manifest.module {
                Some(ref modules) => modules
                    .iter()
                    .map(|module| {
                        LockfileModule::from_module(
                            &manifest.base_directory_path,
                            k.name.as_ref(),
                            &k.version,
                            module,
                            download_url,
                        )
                    })
                    .collect(),
                _ => vec![],
            };
            let commands: Vec<LockfileCommand> = match manifest.command {
                Some(ref modules) => modules
                    .iter()
                    .map(|c| LockfileCommand::from_command(&k.name, k.version.clone(), c))
                    .collect::<Result<Vec<LockfileCommand>, Error>>()
                    .map_err(LockfileError::CommandPackageVersionParseError)?,
                _ => vec![],
            };
            packages.insert(
                PackageKey::WapmPackage(k.clone()),
                LockfilePackage { modules, commands },
            );
        }
        Ok(Self { packages })
    }

    pub fn new_from_result(result: LockfileResult) -> Result<Self, LockfileError> {
        match result {
            LockfileResult::Lockfile(l) => Ok(Self::new_from_lockfile(l)),
            LockfileResult::NoLockfile => Ok(Self {
                packages: HashMap::new(),
            }),
            LockfileResult::LockfileError(e) => Err(e),
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
            .flat_map(|(pkg_name, pkg_versions)| {
                pkg_versions
                    .into_iter()
                    .map(|(pkg_version, modules)| {
                        let id = PackageKey::new_registry_package(pkg_name.clone(), pkg_version);
                        let lockfile_modules = modules.into_values().collect::<Vec<_>>();
                        let lockfile_commands =
                            lockfile_commands_map.remove(&id).unwrap_or_default();
                        let package_data = LockfilePackage {
                            modules: lockfile_modules,
                            commands: lockfile_commands,
                        };
                        (id, package_data)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<HashMap<_, _>>();

        Self { packages }
    }

    pub fn package_keys(&self) -> HashSet<PackageKey<'a>> {
        self.packages.keys().cloned().collect()
    }

    pub fn find_missing_packages(&self, directory: &Path) -> HashSet<PackageKey<'a>> {
        let missing_packages: HashSet<PackageKey<'a>> = self
            .packages
            .iter()
            .filter_map(|(key, data)| {
                if data.modules.iter().any(|module| {
                    let path = module.get_canonical_source_path_from_lockfile_dir(directory.into());
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

    pub fn remove_packages(&mut self, removed_packages: RemovedPackages<'a>) {
        let removed_package_keys = removed_packages
            .packages
            .into_iter()
            .flat_map(|pkg_name| {
                self.packages
                    .keys()
                    .cloned()
                    .filter(|package_key| match package_key {
                        PackageKey::WapmPackage(WapmPackageKey { name, .. }) => name == &pkg_name,
                        _ => false,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        for removed_package_key in removed_package_keys {
            self.packages.remove(&removed_package_key);
        }
    }

    pub fn extend(&mut self, other_packages: LockfilePackages<'a>) {
        self.packages.extend(other_packages.packages);
    }
}
