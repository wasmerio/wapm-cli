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
    #[fail(display = "Manifest file not found.")]
    MissingManifest,
    #[fail(display = "Could not parse manifest because {}.", _0)]
    ManifestTomlParseError(String),
    #[fail(display = "Could not parse lockfile because {}.", _0)]
    LockfileTomlParseError(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
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

pub enum PackageData<'a> {
    LockfilePackage {
        modules: Vec<LockfileModule<'a>>,
        commands: Vec<LockfileCommand<'a>>,
    },
    ManifestDependencyPackage {},
    ManifestPackage {},
}

struct ManifestData<'a> {
//    pub value: Option<Manifest>,
    pub package_data: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> ManifestData<'a> {
    pub fn new_from_string(source: &String) -> Result<Self, BonjourError> {
        if let Manifest { package, dependencies: Some(dependencies), .. } = toml::from_str::<Manifest>(source)
            .map(|m| m)
            .map_err(|e| BonjourError::ManifestTomlParseError(e.to_string()))? {
            let package_ids = dependencies
                .iter()
                .map(|(name, value)| match value {
                    Value::String(version) => Ok(PackageId::WapmRegistryPackage {
                        name,
                        version: &version,
                    }),
                    _ => Err(BonjourError::DependencyVersionMustBeString(
                        name.to_string(),
                    )),
                })
                .collect::<Result<BTreeSet<_>, BonjourError>>()
                .map(|d| if d.is_empty() { None } else { Some(d) });
            package_ids
        };
        unimplemented!()
    }

    pub fn get_packages(&'a self) -> Result<Option<BTreeSet<PackageId<'a>>>, BonjourError> {
        match self.value {
            Some(Manifest {
                ref package,
                dependencies: Some(ref dependencies),
                ..
            }) => {
                // TODO, also pass along local package key
                let package_ids = dependencies
                    .iter()
                    .map(|(name, value)| match value {
                        Value::String(version) => Ok(PackageId::WapmRegistryPackage {
                            name,
                            version: &version,
                        }),
                        _ => Err(BonjourError::DependencyVersionMustBeString(
                            name.to_string(),
                        )),
                    })
                    .collect::<Result<BTreeSet<_>, BonjourError>>()
                    .map(|d| if d.is_empty() { None } else { Some(d) });
                package_ids
            }
            _ => Ok(None),
        }
    }
}

struct LockfileData<'a> {
    pub package_data: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> LockfileData<'a> {
    pub fn new_from_string(source: &'a String) -> Result<Self, BonjourError> {
        // get that lockfile
        let lockfile: Lockfile = toml::from_str::<Lockfile>(source)
            .map_err(|e| BonjourError::LockfileTomlParseError(e.to_string()))?;

        let (raw_lockfile_modules, raw_lockfile_commands) = (lockfile.modules, lockfile.commands);

        let mut lockfile_commands_map: BTreeMap<PackageId, Vec<LockfileCommand>> = BTreeMap::new();
        for (name, command) in raw_lockfile_commands {
            let command: LockfileCommand<'a> = command;
            let id = PackageId::new_registry_package(command.package_name, command.package_version);
            let command_vec = lockfile_commands_map.entry(id).or_default();
            command_vec.push(command);
        }

        let package_data: BTreeMap<PackageId, PackageData> = raw_lockfile_modules.into_iter().map(|(pkg_name, pkg_versions)| {
            pkg_versions.into_iter().map(|(pkg_version, modules)| {
                let id = PackageId::new_registry_package(pkg_name.clone(), pkg_version.clone());
                let lockfile_modules = modules.into_iter().map(|(module_name, module)| module).collect::<Vec<_>>();
                let lockfile_commands = lockfile_commands_map.remove(&id).unwrap();
                let package_data = PackageData::LockfilePackage { modules: lockfile_modules, commands: lockfile_commands };
                (id, package_data)
            }).collect::<Vec<_>>()
        }).flatten().collect::<BTreeMap<_, _>>();

        Ok(Self {
            package_data,
        })
    }

    pub fn new_from_packages(packages: BTreeMap<PackageId<'a>, PackageData<'a>>) -> Self {
        unimplemented!()
    }

    pub fn save(self, path: &Path) -> Result<(), BonjourError> {
        Ok(())
    }
}

fn open_manifest_file(directory: &Path) -> Option<String> {
    let manifest_path_buf = directory.join(MANIFEST_FILE_NAME);
    if !manifest_path_buf.is_dir() {
        return None;
    }
    fs::read_to_string(&manifest_path_buf).ok()
}

fn open_lockfile(directory: &Path) -> Option<String> {
    let lockfile_path_buf = directory.join(LOCKFILE_NAME);
    if !lockfile_path_buf.is_dir() {
        return None;
    }
    fs::read_to_string(&lockfile_path_buf).ok()
}

fn calculate_differences<'a>(
    manifest_packages_set: Option<BTreeSet<PackageId<'a>>>,
    lockfile_packages_set: Option<BTreeSet<PackageId<'a>>>,
) -> (Vec<PackageId<'a>>, Vec<PackageId<'a>>, Vec<PackageId<'a>>) {
    match (manifest_packages_set, lockfile_packages_set) {
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
    let directory = env::current_dir().unwrap(); // TODO: will panic, move this up later
    let manifest_string = open_manifest_file(&directory);
    let lockfile_string = open_lockfile(&directory); // None indicates file is unavailable
                                                     // deserialize and create manifest data
    let manifest_data = match manifest_string.as_ref().map(ManifestData::new_from_string) {
        Some(result) => Some(result?),
        None => None,
    };
    // collect manifest dependency and local package keys, used later for diffing
    let manifest_packages = manifest_data
        .as_ref()
        .map(ManifestData::get_packages)
        .unwrap_or(Ok(None))?;
    // deserialize and create lockfile data
    let lockfile_data = match lockfile_string.as_ref().map(LockfileData::new_from_string) {
        Some(result) => Some(result?),
        None => None,
    };
    let lockfile_packages = lockfile_data.as_ref().map(|ld| ld.package_data.keys().cloned().collect::<BTreeSet<_>>());

    // calculate diffs
    let (added, removed, _unchanged) = calculate_differences(manifest_packages, lockfile_packages);

    let mut packages_map = lockfile_data.map(|ld| ld.package_data).unwrap_or_default();

    // prune dependencies that have been removed from dependencies list
    removed
        .iter()
        .map(|p| packages_map.remove(p))
        .for_each(drop);
    // fetch and insert added packages
    // TODO!

    // serialize
    let lockfile_data = LockfileData::new_from_packages(packages_map);
    lockfile_data.save(&directory)?;
    Ok(())
}
