use std::cmp::Ordering;
use std::collections::btree_map::BTreeMap;
use std::collections::btree_set::BTreeSet;
use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::bonjour::differences::PackageDataDifferences;
use crate::bonjour::lockfile::{LockfileData, LockfileResult};
use crate::bonjour::manifest::{ManifestData, ManifestResult};
use crate::dependency_resolver::{Dependency, PackageRegistry, PackageRegistryLike};
use crate::lock::{Lockfile, LockfileCommand, LockfileModule, LOCKFILE_NAME};
use crate::manifest::MANIFEST_FILE_NAME;

pub mod differences;
pub mod lockfile;
pub mod manifest;

#[derive(Clone, Debug, Fail)]
pub enum BonjourError {
    #[fail(display = "Could not parse manifest because {}.", _0)]
    ManifestTomlParseError(String),
    #[fail(display = "Could not parse lockfile because {}.", _0)]
    LockfileTomlParseError(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
    #[fail(display = "Could not install added packages. {}.", _0)]
    InstallError(String),
    #[fail(display = "Could not save lockfile. {}.", _0)]
    LockfileSaveError(String),
}

#[derive(Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum PackageId<'a> {
    LocalPackage { directory: &'a Path },
    WapmRegistryPackage { name: &'a str, version: &'a str },
    //    GitUrl { url: &'a str, },
}

impl<'a> PackageId<'a> {
    fn new_registry_package(name: &'a str, version: &'a str) -> Self {
        PackageId::WapmRegistryPackage { name, version }
    }
}

impl<'a> Ord for PackageId<'a> {
    fn cmp(&self, other: &PackageId<'a>) -> Ordering {
        match (self, other) {
            (
                PackageId::WapmRegistryPackage { name, version },
                PackageId::WapmRegistryPackage {
                    name: other_name,
                    version: other_version,
                },
            ) => {
                let name_cmp = name.cmp(other_name);
                let version_cmp = version.cmp(other_version);
                match (name_cmp, version_cmp) {
                    (Ordering::Equal, _) => version_cmp,
                    _ => name_cmp,
                }
            }
            (
                PackageId::LocalPackage { directory },
                PackageId::LocalPackage {
                    directory: other_directory,
                },
            ) => directory.cmp(other_directory),
            (PackageId::LocalPackage { .. }, _) => Ordering::Less,
            (PackageId::WapmRegistryPackage { .. }, _) => Ordering::Greater,
        }
    }
}

#[derive(Debug)]
pub enum PackageData<'a> {
    LockfilePackage {
        modules: Vec<LockfileModule<'a>>,
        commands: Vec<LockfileCommand<'a>>,
    },
    ManifestDependencyPackage,
//    ResolvedManifestDependencyPackage(Dependency),
//    ManifestPackage,
}

fn open_manifest_file(directory: &Path) -> Option<String> {
    if !directory.is_dir() {
        return None;
    }
    let manifest_path_buf = directory.join(MANIFEST_FILE_NAME);
    fs::read_to_string(&manifest_path_buf).ok()
}

fn open_lockfile(directory: &Path) -> Option<String> {
    if !directory.is_dir() {
        return None;
    }
    let lockfile_path_buf = directory.join(LOCKFILE_NAME);
    fs::read_to_string(&lockfile_path_buf).ok()
}

fn install_added_dependencies<'a>(
    added_set: BTreeSet<PackageId<'a>>,
    registry: &'a mut PackageRegistry,
) -> Result<Vec<&'a Dependency>, BonjourError> {
    // get added wapm registry packages
    let added_package_ids: Vec<(&str, &str)> = added_set
        .iter()
        .cloned()
        .filter_map(|id| match id {
            PackageId::WapmRegistryPackage { name, version } => Some((name, version)),
            _ => None,
        })
        .collect();

    // sync and install missing dependencies
    registry
        .get_all_dependencies(added_package_ids)
        .map_err(|e| BonjourError::InstallError(e.to_string()))
}

fn generate_lockfile<'a>(
    differences: &PackageDataDifferences<'a>,
    directory: &'a Path,
) -> Result<(), BonjourError> {
    let mut lockfile = Lockfile {
        modules: BTreeMap::new(),
        commands: BTreeMap::new(),
    };

    differences
        .new_state
        .iter()
        .map(|(id, data)| match (id, data) {
            (
                PackageId::WapmRegistryPackage { name, version },
                PackageData::LockfilePackage { modules, commands },
            ) => {
                for module in modules {
                    let versions: &mut BTreeMap<&str, BTreeMap<&str, LockfileModule>> =
                        lockfile.modules.entry(name).or_default();
                    let modules: &mut BTreeMap<&str, LockfileModule> =
                        versions.entry(version).or_default();
                    modules.insert(module.name.clone(), module.clone());
                }
                for command in commands {
                    lockfile
                        .commands
                        .insert(command.name.clone(), command.clone());
                }
            }
            _ => {}
        })
        .for_each(drop);

    lockfile
        .save(&directory)
        .map_err(|e| BonjourError::LockfileSaveError(e.to_string()))
}

pub fn update(added_packages: Vec<(&str, &str)>) -> Result<(), BonjourError> {
    let directory: PathBuf = env::current_dir().unwrap(); // TODO: will panic, move this up later
    // get manifest data
    let manifest_string_source: Option<String> = open_manifest_file(&directory);
    let manifest_result: ManifestResult = ManifestResult::from_optional_source(&manifest_string_source);
    let mut manifest_data: Option<ManifestData> = ManifestData::new_from_result(&manifest_result)?;
    // add additional dependencies to deserialized manifest data
    if let Some(ref mut manifest_data) = manifest_data {
        for (name, version) in added_packages {
            let id = PackageId::new_registry_package(name, version);
            manifest_data.package_data.insert(id, PackageData::ManifestDependencyPackage);
        }
    }
    // get lockfile data
    let lockfile_string: Option<String> = open_lockfile(&directory); // None indicates file is unavailable
    let lockfile_result: LockfileResult = LockfileResult::from_optional_source(&lockfile_string);
    let lockfile_data: Option<LockfileData> = LockfileData::new_from_result(lockfile_result)?;
    // construct a pacakge registry for accessing external dependencies
    let mut registry = PackageRegistry::new();
    // create a differences object. It has added, removed, and unchanged package ids.
    let mut differences =
        PackageDataDifferences::calculate_differences(manifest_data, lockfile_data);
    // install added dependencies
    let added_set = differences.added_set.clone();
    let dependencies = install_added_dependencies(added_set, &mut registry)?;
    differences.insert_dependencies_as_lockfile_packages(&dependencies);
    // generate and save a lockfile
    generate_lockfile(&differences, &directory)?;
    Ok(())
}
