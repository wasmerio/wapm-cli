use crate::lock::lockfile::LockfileError;
use crate::lock::{Lockfile, LockfileCommand, LockfileModule, LOCKFILE_NAME};
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
use std::cmp::Ordering;
use std::collections::btree_map::BTreeMap;
use std::collections::btree_set::BTreeSet;
use std::path::Path;
use std::{env, fs};
use toml::Value;

#[derive(Clone, Debug, Fail)]
pub enum BonjourError {
    #[fail(display = "e")]
    E,
}

#[derive(Clone, Debug, Eq, PartialOrd, PartialEq)]
pub enum PackageId<'a> {
    LocalPackage { directory: &'a Path },
    WapmRegistryPackage { name: &'a str, version: &'a str },
    //    GitUrl {
    //        url: &'a str,
    //    },
    //    TestPackage {
    //        name: &'a str,
    //        version: &'a str,
    //    }
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

pub enum PackageData<'a> {
    LockfilePackage {
        modules: Vec<LockfileModule<'a>>,
        commands: Vec<LockfileCommand<'a>>,
    },
    ManifestDependencyPackage {},
    ManifestPackage {},
}

#[derive(Clone, Debug)]
struct ManifestData {
    pub value: Option<Manifest>,
}

impl<'a> ManifestData {
    pub fn new_from_string(source: &String) -> Result<Self, BonjourError> {
        toml::from_str::<Manifest>(source)
            .map(|m| Self { value: Some(m) })
            .map_err(|_| BonjourError::E)
    }

    pub fn get_packages(&'a self) -> Result<Option<BTreeSet<PackageId<'a>>>, BonjourError> {
        match self.value {
            Some(Manifest { ref package, dependencies: Some(ref dependencies), .. }) => {
                // TODO, also pass along local package key
                let package_ids = dependencies.iter().map(|(name, value)| {
                    match value {
                        Value::String(version) => Ok(PackageId::WapmRegistryPackage {
                            name,
                            version: &version,
                        }),
                        _ => Err(BonjourError::E),
                    }
                }).collect::<Result<BTreeSet<_>, BonjourError>>()
                .map(|d| {
                    if d.is_empty() {
                        None
                    }
                    else {
                        Some(d)
                    }
                });
                package_ids
            },
            _ => {
                Ok(None)
            },
        }
    }
}


struct LockfileData<'a> {
    pub value: Option<Lockfile<'a>>,
}

impl<'a> LockfileData<'a> {
    pub fn new_from_string(source: &'a String) -> Result<Self, BonjourError> {
        toml::from_str::<Lockfile>(source)
            .map(|l| Self { value: Some(l) })
            .map_err(|_| BonjourError::E)
    }

    pub fn new_from_packages(packages: BTreeMap<PackageId<'a>, PackageData<'a>>) -> Self {
        unimplemented!()
    }

    pub fn get_packages_and_package_data(
        &'a self,
    ) -> Option<(
        BTreeSet<PackageId<'a>>,
        BTreeMap<PackageId<'a>, PackageData<'a>>,
    )> {
        self.value.as_ref().map(|l| &l.modules).map(|modules| {
            let package_ids = modules
                .keys()
                .cloned()
                .into_iter()
                .map(|n| {
                    modules
                        .get(n)
                        .unwrap()
                        .keys()
                        .map(|v| PackageId::WapmRegistryPackage {
                            name: n.clone(),
                            version: v,
                        })
                        .collect::<Vec<PackageId<'a>>>()
                })
                .flatten()
                .collect::<BTreeSet<PackageId<'a>>>();
            (package_ids, BTreeMap::new())
        })
    }

    pub fn save(self) -> Result<(), BonjourError> {
        Ok(())
    }
}

fn open_manifest_file(directory: &Path) -> Result<String, BonjourError> {
    let manifest_path_buf = directory.join(MANIFEST_FILE_NAME);
    fs::read_to_string(&manifest_path_buf).map_err(|_| BonjourError::E)
}

fn open_lockfile(directory: &Path) -> Result<String, BonjourError> {
    let lockfile_path_buf = directory.join(LOCKFILE_NAME);
    fs::read_to_string(&lockfile_path_buf).map_err(|_| BonjourError::E)
}

fn calculate_differences<'a>(manifest_packages_set: Option<BTreeSet<PackageId<'a>>>, lockfile_packages_set: Option<BTreeSet<PackageId<'a>>>) -> (Vec<PackageId<'a>>, Vec<PackageId<'a>>, Vec<PackageId<'a>>) {
    match (manifest_packages_set, lockfile_packages_set)
        {
            (Some(manifest_pkgs), Some(lockfile_pkgs)) => {
                let added = manifest_pkgs
                    .difference(&lockfile_pkgs)
                    .cloned()
                    .collect::<Vec<_>>();
                let removed = lockfile_pkgs
                    .difference(&manifest_pkgs)
                    .cloned()
                    .collect::<Vec<_>>();
                let unchanged = manifest_pkgs
                    .union(&lockfile_pkgs)
                    .cloned()
                    .collect::<Vec<_>>();
                (added, removed, unchanged)
            }
            (Some(manifest_pkgs), None) => {
                let added = manifest_pkgs.into_iter().collect::<Vec<_>>();
                (added, vec![], vec![])
            }
            (None, Some(lockfile_pkgs)) => {
                let unchanged = lockfile_pkgs.into_iter().collect::<Vec<_>>();
                (vec![], vec![], unchanged)
            }
            (None, None) => (vec![], vec![], vec![]),
        }
}

pub fn update() -> Result<(), BonjourError> {
    let directory = env::current_dir().map_err(|_| BonjourError::E)?;
    // TODO add handler for case when manifest does not exist and a case when manifest is invalid
    let manifest_string = open_manifest_file(&directory);
    // TODO add handler for case when the lockfile is malformed, notify, but continue
    let lockfile_string = open_lockfile(&directory);
    // deserialize and create manifest data
    let manifest_data = manifest_string
        .as_ref()
        .map(ManifestData::new_from_string)
        .map_err(BonjourError::clone)?;
    // collect manifest dependency and local package keys, used later for diffing
    let manifest_packages = manifest_data
        .as_ref()
        .map(ManifestData::get_packages)
        .unwrap_or(Ok(None))?;
    // deserialize and create lockfile data
    let lockfile_data = lockfile_string
        .as_ref()
        .map(LockfileData::new_from_string)
        .map_err(BonjourError::clone)?
        .ok();
    // collect lockfile packages keys and package datas
    let lockfile_packages = lockfile_data
        .as_ref()
        .map(LockfileData::get_packages_and_package_data)
        .unwrap_or_default(); // flatten

    let (lockfile_packages, lockfile_packages_map) = match lockfile_packages {
        Some((lockfile_packages, lockfile_packages_map)) => (Some(lockfile_packages), Some(lockfile_packages_map)),
        None => (None, None),
    };
    // calculate diffs
    let (added, removed, _unchanged) = calculate_differences(manifest_packages, lockfile_packages);

    let mut packages_map = lockfile_packages_map.unwrap_or_default();

    // prune dependencies that have been removed from dependencies list
    removed.iter().map(|p| packages_map.remove(p)).for_each(drop);
    // fetch and insert added packages
    // TODO!

    // serialize
    let lockfile_data = LockfileData::new_from_packages(packages_map);
    lockfile_data.save()?;
    Ok(())
}
