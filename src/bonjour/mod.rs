use std::cmp::Ordering;
use std::collections::btree_set::BTreeSet;
use std::path::Path;

use crate::bonjour::differences::PackageDataDifferences;
use crate::bonjour::lockfile::{LockfileData, LockfileResult, LockfileSource};
use crate::bonjour::manifest::{ManifestData, ManifestResult, ManifestSource};
use crate::dependency_resolver::{Dependency, PackageRegistry, PackageRegistryLike};
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::cfg_toml::lock::lockfile_command::LockfileCommand;

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

pub fn update<P: AsRef<Path>>(
    added_packages: &Vec<(&str, &str)>,
    directory: P,
) -> Result<(), BonjourError> {
    let directory = directory.as_ref();
    // get manifest data
    let manifest_source = ManifestSource::new(&directory);
    let manifest_result = ManifestResult::from_source(&manifest_source);
    let mut manifest_data = ManifestData::new_from_result(&manifest_result)?;
    // add the extra packages
    manifest_data.add_additional_packages(added_packages);
    // get lockfile data
    let lockfile_string = LockfileSource::new(&directory);
    let lockfile_result: LockfileResult = LockfileResult::from_source(&lockfile_string);
    let lockfile_data = LockfileData::new_from_result(lockfile_result)?;
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
    differences.generate_lockfile(&directory)?;
    Ok(())
}
