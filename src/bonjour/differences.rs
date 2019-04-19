use crate::bonjour::lockfile::LockfileData;
use crate::bonjour::manifest::ManifestData;
use crate::bonjour::{PackageData, PackageId};
use crate::dependency_resolver::{Dependency, PackageRegistry};
use crate::lock::{LockfileCommand, LockfileModule};
use std::collections::btree_map::BTreeMap;
use std::collections::btree_set::BTreeSet;

#[derive(Debug)]
pub struct PackageDataDifferences<'a> {
    pub added_set: BTreeSet<PackageId<'a>>,
    pub removed_set: BTreeSet<PackageId<'a>>,
    pub retained_set: BTreeSet<PackageId<'a>>,
    pub new_state: BTreeMap<PackageId<'a>, PackageData<'a>>,
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
                        .partition(|(id, _data)| removed_set.contains(id));

                let mut added_packages: BTreeMap<PackageId, PackageData> = manifest_data
                    .package_data
                    .into_iter()
                    .filter(|(id, _data)| added_set.contains(id))
                    .collect();

                new_state.append(&mut added_packages);

                PackageDataDifferences {
                    added_set,
                    removed_set,
                    retained_set,
                    new_state,
                }
            }
            (Some(manifest_data), None) => {
                let manifest_packages_set = manifest_data.package_data.keys().cloned().collect();
                PackageDataDifferences {
                    added_set: manifest_packages_set,
                    removed_set: BTreeSet::new(),
                    retained_set: BTreeSet::new(),
                    new_state: manifest_data.package_data,
                }
            }
            (None, Some(lockfile_data)) => {
                let lockfile_packages_set = lockfile_data.package_data.keys().cloned().collect();
                PackageDataDifferences {
                    added_set: BTreeSet::new(),
                    removed_set: BTreeSet::new(),
                    retained_set: lockfile_packages_set,
                    new_state: lockfile_data.package_data,
                }
            }
            (None, None) => PackageDataDifferences {
                added_set: BTreeSet::new(),
                removed_set: BTreeSet::new(),
                retained_set: BTreeSet::new(),
                new_state: BTreeMap::new(),
            },
        }
    }

    pub fn insert_dependencies_as_lockfile_packages(
        &mut self,
        dependencies: &'a Vec<&'a Dependency>,
    ) {
        for dep in dependencies {
            let modules = LockfileModule::from_dependency(dep).unwrap();
            let commands = LockfileCommand::from_dependency(dep).unwrap();
            let id = PackageId::WapmRegistryPackage {
                name: dep.name.as_str(),
                version: dep.version.as_str(),
            };
            let lockfile_package = PackageData::LockfilePackage { modules, commands };
            self.new_state.insert(id, lockfile_package);
        }
    }
}
