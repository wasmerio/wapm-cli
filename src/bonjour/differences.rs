//use crate::bonjour::lockfile::LockfileData;
//use crate::bonjour::manifest::ManifestData;
//use crate::bonjour::{BonjourError, PackageData, PackageKey, WapmPackageKey};
//use crate::cfg_toml::lock::lockfile::Lockfile;
//use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
//use crate::cfg_toml::lock::lockfile_module::LockfileModule;
//use crate::dependency_resolver::Dependency;
//use std::collections::btree_map::BTreeMap;
//use std::collections::btree_set::BTreeSet;
//use std::path::Path;
//use std::collections::hash_set::HashSet;
//
//pub struct AddedPackages<'a> {
//    pub added_set: BTreeSet<PackageKey<'a>>,
//}
//
//impl<'a> AddedPackages<'a> {
//    pub fn from_diffs(diffs: &PackageDataDifferences<'a>) -> Self {
//        let added_set: BTreeSet<PackageKey> = diffs.manifest_packages_set
//            .difference(&diffs.lockfile_packages_set)
//            .cloned()
//            .collect();
//        Self { added_set }
//    }
//}
//
//pub struct RetainedPackages<'a> {
//    pub retained_set: BTreeMap<PackageKey<'a>, PackageData<'a>>,
//}
//
//impl<'a> RetainedPackages<'a> {
//    pub fn from_diffs(diffs: &mut PackageDataDifferences<'a>) -> Self {
//        let retained_set: BTreeSet<PackageKey> = diffs.manifest_packages_set
//            .union(&diffs.lockfile_packages_set)
//            .cloned().collect();
//        retained_set.iter().for_each(|k| {
//            diffs.lockfile_package_data.remove(k);
//        });
//        unimplemented!()
////        diffs.lockfile_package_data.remove()
////        Self { retained_set }
//    }
//}
//
//#[derive(Debug)]
//pub struct PackageDataDifferences<'a> {
//    manifest_packages_set: BTreeSet<PackageKey<'a>>,
//    lockfile_package_data: BTreeMap<PackageKey<'a>, PackageData<'a>>,
//    lockfile_packages_set: BTreeSet<PackageKey<'a>>,
////    pub added_set: BTreeSet<PackageKey<'a>>,
////    pub removed_set: BTreeSet<PackageKey<'a>>,
////    pub retained_set: BTreeSet<PackageKey<'a>>,
////    pub retained_data: BTreeMap<PackageKey<'a>, PackageData<'a>>,
//}
//
//impl<'a> PackageDataDifferences<'a> {
////    pub fn get_added_set(&self) -> BTreeSet<PackageKey> {
////        let added_set: BTreeSet<PackageKey> = self.manifest_packages_set
////            .difference(&self.lockfile_packages_set)
////            .cloned()
////            .collect();
////        added_set
////    }
//
////    pub fn get_retained_lockfile_data(&self) -> BTreeMap<PackageKey<'a>, PackageData<'a>> {
////        let removed_set: BTreeSet<PackageKey> = self.lockfile_packages_set
////            .difference(&self.manifest_packages_set)
////            .cloned()
////            .collect();
////        let retained_set: BTreeSet<PackageKey> = self.manifest_packages_set
////            .union(&self.lockfile_packages_set)
////            .cloned()
////            .collect();
////        let (_removed_packages_map, mut new_state): (BTreeMap<_, _>, BTreeMap<_, _>) =
////            self.lockfile_package_data
////                .into_iter()
////                .partition(|(id, _data)| removed_set.contains(id));
////        new_state
////    }
//
//    pub fn calculate_differences(
//        manifest_data: ManifestData<'a>,
//        lockfile_data: LockfileData<'a>,
//    ) -> Self {
//        let manifest_packages_set = manifest_data.package_keys.unwrap_or_default();
//        let lockfile_package_data = lockfile_data.package_data.unwrap_or_default();
//        let lockfile_packages_set: BTreeSet<PackageKey> =
//            lockfile_package_data.keys().cloned().collect();
//
////        let added_set: BTreeSet<PackageKey> = manifest_packages_set
////            .difference(&lockfile_packages_set)
////            .cloned()
////            .collect();
////        let removed_set: BTreeSet<PackageKey> = lockfile_packages_set
////            .difference(&manifest_packages_set)
////            .cloned()
////            .collect();
////        let retained_set: BTreeSet<PackageKey> = manifest_packages_set
////            .union(&lockfile_packages_set)
////            .cloned()
////            .collect();
////        let (_removed_packages_map, mut new_state): (BTreeMap<_, _>, BTreeMap<_, _>) =
////            lockfile_package_data
////                .into_iter()
////                .partition(|(id, _data)| removed_set.contains(id));
//
//        //        let added_packages: BTreeSet<PackageKey> = manifest_packages_set
//        //            .into_iter()
//        //            .filter(|key| added_set.contains(key))
//        //            .collect();
//        //
//        //        new_state.append(&mut added_packages);
//
//        PackageDataDifferences {
//            manifest_packages_set,
//            lockfile_package_data,
//            lockfile_packages_set,
//
////            added_set,
////            removed_set,
////            retained_set,
////            retained_data: new_state,
//        }
//    }
//
//    pub fn insert_dependencies_as_lockfile_packages(
//        &mut self,
//        dependencies: &'a Vec<&'a Dependency>,
//    ) {
////        for dep in dependencies {
////            let modules = LockfileModule::from_dependency(dep).unwrap();
////            let commands = LockfileCommand::from_dependency(dep).unwrap();
////            let id = PackageKey::WapmPackage(WapmPackageKey {
////                name: dep.name.as_str(),
////                version: dep.version.as_str(),
////            });
////            let lockfile_package = PackageData::LockfilePackage { modules, commands };
////            self.retained_data.insert(id, lockfile_package);
//////            self.retained_data.insert(id, lockfile_package);
////        }
//    }
//
//    pub fn generate_lockfile(&self, directory: &'a Path) -> Result<(), BonjourError> {
//        Ok(())
////        let mut lockfile = Lockfile {
////            modules: BTreeMap::new(),
////            commands: BTreeMap::new(),
////        };
////
////        self.retained_data
////            .iter()
////            .map(|(id, data)| match (id, data) {
////                (
////                    PackageKey::WapmPackage(WapmPackageKey { name, version }),
////                    PackageData::LockfilePackage { modules, commands },
////                ) => {
////                    for module in modules {
////                        let versions: &mut BTreeMap<&str, BTreeMap<&str, LockfileModule>> =
////                            lockfile.modules.entry(name).or_default();
////                        let modules: &mut BTreeMap<&str, LockfileModule> =
////                            versions.entry(version).or_default();
////                        modules.insert(module.name.clone(), module.clone());
////                    }
////                    for command in commands {
////                        lockfile
////                            .commands
////                            .insert(command.name.clone(), command.clone());
////                    }
////                }
////                _ => {}
////            })
////            .for_each(drop);
////
////        lockfile
////            .save(&directory)
////            .map_err(|e| BonjourError::LockfileSaveError(e.to_string()))
//    }
//}
//
//pub struct MergedPackageData<'a> {
//    packages: BTreeMap<PackageKey<'a>, PackageData<'a>>
//}
//
//impl<'a> MergedPackageData <'a> {
//    pub fn merge_from_manifest_and_lockfile(manifest_data: ManifestData<'a>, lockfile_data: LockfileData<'a>) -> Self {
//        let manifest_packages_set = manifest_data.package_keys.unwrap_or_default();
//        let lockfile_package_data = lockfile_data.package_data.unwrap_or_default();
//        let lockfile_packages_set: BTreeSet<PackageKey> =
//            lockfile_package_data.keys().cloned().collect();
//
//        let added_set: BTreeSet<PackageKey> = manifest_packages_set
//            .difference(&lockfile_packages_set)
//            .cloned()
//            .collect();
//        let removed_set: BTreeSet<PackageKey> = lockfile_packages_set
//            .difference(&manifest_packages_set)
//            .cloned()
//            .collect();
//
//        let added_packages = added_set.into_iter().map(|k| (k, PackageData::ManifestPackage));
//        let existing_lockfile_packages = lockfile_package_data.into_iter().filter(|(k, v)| !removed_set.contains(k));
//        let packages: BTreeMap<_,_> = added_packages.chain(existing_lockfile_packages).collect();
//        Self { packages }
//    }
//}
