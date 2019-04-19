use crate::lock::lockfile::LockfileError;
use crate::lock::{Lockfile, LockfileCommand, LockfileModule, LOCKFILE_NAME};
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
use std::cmp::Ordering;
use std::collections::btree_map::BTreeMap;
use std::collections::btree_set::BTreeSet;
use std::path::Path;
use std::{env, fs};
use toml::Value;
use crate::bonjour::PackageData::ManifestDependencyPackage;

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
    ManifestDependencyPackage,
    ManifestPackage,
}

struct ManifestData<'a> {
    pub package_data: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> ManifestData<'a> {
    pub fn new_from_manifest(manifest: &'a Manifest) -> Result<Self, BonjourError> {
        let package_data = if let Manifest { package, dependencies: Some(ref dependencies), .. } = manifest {
            dependencies
                .iter()
                .map(|(name, value)| match value {
                    Value::String(ref version) => Ok((PackageId::WapmRegistryPackage {
                        name,
                        version,
                    }, PackageData::ManifestDependencyPackage)),
                    _ => Err(BonjourError::DependencyVersionMustBeString(
                        name.to_string(),
                    )),
                })
                .collect::<Result<BTreeMap<PackageId, PackageData>, BonjourError>>()?
        } else {
            BTreeMap::new()
        };
        Ok(ManifestData { package_data })
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

    pub fn new_from_packages(packages: &BTreeMap<PackageId<'a>, PackageData<'a>>) -> Self {
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
    manifest_data: Option<ManifestData<'a>>,
    lockfile_data: Option<LockfileData<'a>>,
) -> (BTreeSet<PackageId<'a>>, BTreeSet<PackageId<'a>>, BTreeSet<PackageId<'a>>, BTreeMap<PackageId<'a>, PackageData<'a>>) {
    match (manifest_data, lockfile_data) {
        (Some(manifest_data), Some(lockfile_data)) => {
            let manifest_packages_set: BTreeSet<PackageId> = manifest_data.package_data.keys().cloned().collect();
            let lockfile_packages_set: BTreeSet<PackageId> = lockfile_data.package_data.keys().cloned().collect();
            let added: BTreeSet<PackageId> = manifest_packages_set
                .difference(&lockfile_packages_set)
                .cloned()
                .collect();
            let removed: BTreeSet<PackageId> = lockfile_packages_set
                .difference(&manifest_packages_set)
                .cloned()
                .collect();
            let unchanged: BTreeSet<PackageId> = manifest_packages_set
                .union(&lockfile_packages_set)
                .cloned()
                .collect();
            let (removed_packages_map, mut current_packages): (BTreeMap<_,_>, BTreeMap<_,_>) = lockfile_data.package_data.into_iter().partition(|(id, data)| {
                removed.contains(id)
            });

            let mut added_packages: BTreeMap<PackageId, PackageData> = manifest_data.package_data.into_iter().filter(|(id, data)| {
                added.contains(id)
            }).collect();

            current_packages.append(&mut added_packages);

            (added, removed, unchanged, current_packages)
        },
        (Some(manifest_data), None) => {
            let manifest_packages_set = manifest_data.package_data.keys().cloned().collect();
            (manifest_packages_set, BTreeSet::new(), BTreeSet::new(), manifest_data.package_data)
        },
        (None, Some(lockfile_data)) => {
            let lockfile_packages_set = lockfile_data.package_data.keys().cloned().collect();
            (BTreeSet::new(), BTreeSet::new(), lockfile_packages_set, lockfile_data.package_data)
        },
        (None, None) => {
            (BTreeSet::new(), BTreeSet::new(), BTreeSet::new(), BTreeMap::new())
        },
    }
}

enum ManifestResult {
    Manifest(Manifest),
    NoManifest,
    ManifestError(BonjourError),
}

impl ManifestResult {
    pub fn from_source(source: &String) -> ManifestResult {
        match toml::from_str::<Manifest>(source) {
            Ok(m) => ManifestResult::Manifest(m),
            Err(e) => ManifestResult::ManifestError(BonjourError::ManifestTomlParseError(e.to_string())),
        }
    }
}

pub fn update() -> Result<(), BonjourError> {
    let directory = env::current_dir().unwrap(); // TODO: will panic, move this up later
    let manifest_string_source = open_manifest_file(&directory);
    let manifest = manifest_string_source.as_ref()
        .map(ManifestResult::from_source)
        .unwrap_or(ManifestResult::NoManifest);
    let manifest_data: Option<ManifestData> = match manifest {
        ManifestResult::Manifest(ref manifest) => Some(ManifestData::new_from_manifest(manifest)),
        ManifestResult::NoManifest => None,
        ManifestResult::ManifestError(e) => return Err(e)
    }.map_or(Ok(None), |r| r.map(Some))?;
//    let manifest_string = manifest_string_source.as_ref();
    let lockfile_string = open_lockfile(&directory); // None indicates file is unavailable
    // deserialize and create lockfile data
    let lockfile_data = match lockfile_string.as_ref().map(LockfileData::new_from_string) {
        Some(result) => Some(result?),
        None => None,
    };
    let (added, removed, unchanged, new_state) = calculate_differences(manifest_data, lockfile_data);

    // install missing dependencies

    // serialize
    let lockfile_data = LockfileData::new_from_packages(&new_state);
    lockfile_data.save(&directory)?;
    Ok(())
}
