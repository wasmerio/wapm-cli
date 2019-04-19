use crate::bonjour::lockfile::LockfileData;
use crate::bonjour::manifest::ManifestData;
use crate::bonjour::{BonjourError, PackageData, PackageId};
use crate::dependency_resolver::Dependency;
use std::collections::btree_map::BTreeMap;
use std::collections::btree_set::BTreeSet;
use std::path::Path;
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
use crate::cfg_toml::lock::lockfile::Lockfile;

#[derive(Debug)]
pub struct PackageDataDifferences<'a> {
    pub added_set: BTreeSet<PackageId<'a>>,
    pub removed_set: BTreeSet<PackageId<'a>>,
    pub retained_set: BTreeSet<PackageId<'a>>,
    pub new_state: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> PackageDataDifferences<'a> {
    pub fn calculate_differences(
        manifest_data: ManifestData<'a>,
        lockfile_data: LockfileData<'a>,
    ) -> Self {
        let manifest_package_data = manifest_data.package_data.unwrap_or_default();
        let lockfile_package_data = lockfile_data.package_data.unwrap_or_default();

        let manifest_packages_set: BTreeSet<PackageId> =
            manifest_package_data.keys().cloned().collect();
        let lockfile_packages_set: BTreeSet<PackageId> =
            lockfile_package_data.keys().cloned().collect();
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
        let (_removed_packages_map, mut new_state): (BTreeMap<_, _>, BTreeMap<_, _>) =
            lockfile_package_data
                .into_iter()
                .partition(|(id, _data)| removed_set.contains(id));

        let mut added_packages: BTreeMap<PackageId, PackageData> = manifest_package_data
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

    pub fn generate_lockfile(&self, directory: &'a Path) -> Result<(), BonjourError> {
        let mut lockfile = Lockfile {
            modules: BTreeMap::new(),
            commands: BTreeMap::new(),
        };

        self.new_state
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
}
