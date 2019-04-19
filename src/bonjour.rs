use crate::dependency_resolver::{Dependency, PackageRegistry, PackageRegistryLike};
use crate::lock::lockfile::LockfileError;
use crate::lock::{Lockfile, LockfileCommand, LockfileModule, LOCKFILE_NAME};
use crate::manifest::{Manifest, MANIFEST_FILE_NAME, Package};
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

pub enum PackageData<'a> {
    LockfilePackage {
        modules: Vec<LockfileModule<'a>>,
        commands: Vec<LockfileCommand<'a>>,
    },
    ManifestDependencyPackage,
    ResolvedManifestDependencyPackage(Dependency),
    ManifestPackage,
}

struct ManifestData<'a> {
    pub package_data: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> ManifestData<'a> {
    pub fn new_from_result(result: &'a ManifestResult) -> Result<Option<Self>, BonjourError> {
         match result {
             ManifestResult::Manifest(ref manifest) => {
                 match Self::new_from_manifest(manifest) {
                     Ok(md) => Ok(Some(md)),
                     Err(e) => Err(e),
                 }
             },
             ManifestResult::NoManifest => Ok(None),
             ManifestResult::ManifestError(e) => Err(e.clone()),
         }
    }

    fn new_from_manifest(manifest: &'a Manifest) -> Result<Self, BonjourError> {
        let package_data = if let Manifest {
            package,
            dependencies: Some(ref dependencies),
            ..
        } = manifest
        {
            dependencies
                .iter()
                .map(|(name, value)| match value {
                    Value::String(ref version) => Ok((
                        PackageId::WapmRegistryPackage { name, version },
                        PackageData::ManifestDependencyPackage,
                    )),
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
    pub fn new_from_result(result: LockfileResult<'a>) -> Result<Option<Self>, BonjourError> {
        match result {
            LockfileResult::Lockfile(l) => Ok(Some(Self::new_from_lockfile(l))),
            LockfileResult::NoLockfile => Ok(None),
            LockfileResult::LockfileError(e) => return Err(e),
        }
    }

    fn new_from_lockfile(lockfile: Lockfile<'a>) -> LockfileData {
        let (raw_lockfile_modules, raw_lockfile_commands) = (lockfile.modules, lockfile.commands);

        let mut lockfile_commands_map: BTreeMap<PackageId, Vec<LockfileCommand>> = BTreeMap::new();
        for (name, command) in raw_lockfile_commands {
            let command: LockfileCommand<'a> = command;
            let id = PackageId::new_registry_package(command.package_name, command.package_version);
            let command_vec = lockfile_commands_map.entry(id).or_default();
            command_vec.push(command);
        }

        let package_data: BTreeMap<PackageId, PackageData> = raw_lockfile_modules
            .into_iter()
            .map(|(pkg_name, pkg_versions)| {
                pkg_versions
                    .into_iter()
                    .map(|(pkg_version, modules)| {
                        let id =
                            PackageId::new_registry_package(pkg_name.clone(), pkg_version.clone());
                        let lockfile_modules = modules
                            .into_iter()
                            .map(|(module_name, module)| module)
                            .collect::<Vec<_>>();
                        let lockfile_commands = lockfile_commands_map.remove(&id).unwrap();
                        let package_data = PackageData::LockfilePackage {
                            modules: lockfile_modules,
                            commands: lockfile_commands,
                        };
                        (id, package_data)
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<BTreeMap<_, _>>();

        Self { package_data }
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

enum ManifestResult {
    Manifest(Manifest),
    NoManifest,
    ManifestError(BonjourError),
}

impl ManifestResult {
    pub fn from_optional_source(source: &Option<String>) -> ManifestResult {
        source.as_ref().map(|source| {
            match toml::from_str::<Manifest>(source) {
                Ok(m) => ManifestResult::Manifest(m),
                Err(e) => {
                    ManifestResult::ManifestError(BonjourError::ManifestTomlParseError(e.to_string()))
                }
            }
        }).unwrap_or(ManifestResult::NoManifest)
    }
}

enum LockfileResult<'a> {
    Lockfile(Lockfile<'a>),
    NoLockfile,
    LockfileError(BonjourError),
}

impl<'a> LockfileResult<'a> {
    pub fn from_optional_source(source: &'a Option<String>) -> LockfileResult {
        source.as_ref().map(|source| {
            match toml::from_str::<Lockfile>(source) {
                Ok(l) => LockfileResult::Lockfile(l),
                Err(e) => {
                    LockfileResult::LockfileError(BonjourError::LockfileTomlParseError(e.to_string()))
                }
            }
        }).unwrap_or(LockfileResult::NoLockfile)
    }
}

impl<'a> Default for LockfileResult<'a> {
    fn default() -> Self {
        LockfileResult::NoLockfile
    }
}

struct PackageDataDifferences<'a> {
    pub added_set: BTreeSet<PackageId<'a>>,
    pub removed_set: BTreeSet<PackageId<'a>>,
    pub retained_set: BTreeSet<PackageId<'a>>,
    pub new_state: BTreeMap<PackageId<'a>, PackageData<'a>>,
    registry: PackageRegistry,
}

impl<'a> PackageDataDifferences<'a> {
    pub fn calculate_differences(
        manifest_data: Option<ManifestData<'a>>,
        lockfile_data: Option<LockfileData<'a>>,
    ) -> Self {
        match (manifest_data, lockfile_data) {
            (Some(manifest_data), Some(lockfile_data)) => {
                let manifest_packages_set: BTreeSet<PackageId> =
                    manifest_data.package_data.keys().cloned().collect();
                let lockfile_packages_set: BTreeSet<PackageId> =
                    lockfile_data.package_data.keys().cloned().collect();
                let added_set: BTreeSet<PackageId> = manifest_packages_set
                    .difference(&lockfile_packages_set)
                    .cloned()
                    .collect();
                let removed_set: BTreeSet<PackageId> = lockfile_packages_set
                    .difference(&manifest_packages_set)
                    .cloned()
                    .collect();
                let retained_set: BTreeSet<PackageId> = manifest_packages_set
                    .union(&lockfile_packages_set)
                    .cloned()
                    .collect();
                let (removed_packages_map, mut new_state): (BTreeMap<_, _>, BTreeMap<_, _>) =
                    lockfile_data
                        .package_data
                        .into_iter()
                        .partition(|(id, data)| removed_set.contains(id));

                let mut added_packages: BTreeMap<PackageId, PackageData> = manifest_data
                    .package_data
                    .into_iter()
                    .filter(|(id, data)| added_set.contains(id))
                    .collect();

                new_state.append(&mut added_packages);

                PackageDataDifferences {
                    added_set,
                    removed_set,
                    retained_set,
                    new_state,
                    registry: PackageRegistry::new(),
                }
            }
            (Some(manifest_data), None) => {
                let manifest_packages_set = manifest_data.package_data.keys().cloned().collect();
                PackageDataDifferences {
                    added_set: manifest_packages_set,
                    removed_set: BTreeSet::new(),
                    retained_set: BTreeSet::new(),
                    new_state: manifest_data.package_data,
                    registry: PackageRegistry::new(),
                }
            }
            (None, Some(lockfile_data)) => {
                let lockfile_packages_set = lockfile_data.package_data.keys().cloned().collect();
                PackageDataDifferences {
                    added_set: BTreeSet::new(),
                    removed_set: BTreeSet::new(),
                    retained_set: lockfile_packages_set,
                    new_state: lockfile_data.package_data,
                    registry: PackageRegistry::new(),
                }
            }
            (None, None) => PackageDataDifferences {
                added_set: BTreeSet::new(),
                removed_set: BTreeSet::new(),
                retained_set: BTreeSet::new(),
                new_state: BTreeMap::new(),
                registry: PackageRegistry::new(),
            },
        }
    }

    pub fn insert_dependencies_as_lockfile_packages(&mut self, dependencies: &'a Vec<&'a Dependency>) {
        for dep in dependencies {
            let modules = LockfileModule::from_dependency(dep).unwrap();
            let commands = LockfileCommand::from_dependency(dep).unwrap();
            let id = PackageId::WapmRegistryPackage { name: dep.name.as_str(), version: dep.version.as_str() };
            let lockfile_package = PackageData::LockfilePackage { modules, commands };
            self.new_state.insert(id, lockfile_package);
        }
    }
}

fn install_added_dependencies<'a>(added_set: BTreeSet<PackageId<'a>>, registry: &'a mut PackageRegistry) -> Result<Vec<&'a Dependency>, BonjourError> {
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

fn generate_lockfile<'a>(differences: &PackageDataDifferences<'a>) -> Lockfile<'a> {
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
        }).for_each(drop);

    lockfile
}

pub fn update() -> Result<(), BonjourError> {
    let directory = env::current_dir().unwrap(); // TODO: will panic, move this up later
    let manifest_string_source = open_manifest_file(&directory);
    let manifest_result = ManifestResult::from_optional_source(&manifest_string_source);
    let manifest_data = ManifestData::new_from_result(&manifest_result)?;
    let lockfile_string = open_lockfile(&directory); // None indicates file is unavailable
    let lockfile_result = LockfileResult::from_optional_source(&lockfile_string);
    let lockfile_data = LockfileData::new_from_result(lockfile_result)?;
    let mut registry = PackageRegistry::new();

    // create a differences object. It has added, removed, and unchanged package ids/
    let mut differences =
        PackageDataDifferences::calculate_differences(manifest_data, lockfile_data);

    // install added dependencies
    let added_set = differences.added_set.clone();
    let dependencies = install_added_dependencies(added_set, &mut registry)?;
    differences.insert_dependencies_as_lockfile_packages(&dependencies);

    // generate and save a lockfile
    let lockfile = generate_lockfile(&differences);
    lockfile
        .save(&directory)
        .map_err(|e| BonjourError::LockfileSaveError(e.to_string()))?;
    Ok(())
}
