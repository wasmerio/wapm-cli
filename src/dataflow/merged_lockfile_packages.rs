use crate::cfg_toml::lock::lockfile::{CommandMap, Lockfile, ModuleMap};
use crate::dataflow;
use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
use crate::dataflow::{PackageKey, WapmPackageKey};
use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct MergedLockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage<'a>>,
}

impl<'a> MergedLockfilePackages<'a> {
    pub fn merge(
        new_packages: LockfilePackages<'a>,
        mut old_packages: LockfilePackages<'a>,
    ) -> Self {
        println!("current: {:?}", new_packages);
        println!("other: {:?}", old_packages);
        let keys: HashSet<_> = new_packages.packages.keys().cloned().collect();
        let other_keys: HashSet<_> = old_packages.packages.keys().cloned().collect();
        let removed_keys: Vec<_> = other_keys.difference(&keys).collect();
        for removed_key in removed_keys {
            old_packages.packages.remove(removed_key);
        }
        for (key, data) in new_packages.packages {
            old_packages.packages.insert(key, data);
        }
        println!("merged: {:?}", old_packages);
        Self {
            packages: old_packages.packages,
        }
    }

    pub fn generate_lockfile(self, directory: &'a Path) -> Result<(), dataflow::Error> {
        let mut modules: ModuleMap<'a> = BTreeMap::new();
        let mut commands: CommandMap<'a> = BTreeMap::new();
        for (key, package) in self.packages {
            match key {
                PackageKey::WapmPackage(WapmPackageKey { name, version }) => {
                    let versions = modules.entry(name).or_default();
                    let modules = versions.entry(version).or_default();
                    for module in package.modules {
                        let name = module.name.clone();
                        modules.insert(name, module);
                    }
                    for command in package.commands {
                        let name = command.name.clone();
                        commands.insert(name, command);
                    }
                }
                PackageKey::LocalPackage { .. } => panic!("Local packages are not supported yet."),
                PackageKey::GitUrl { .. } => panic!("Git url packages are not supported yet."),
            }
        }

        let lockfile = Lockfile { modules, commands };

        lockfile
            .save(directory)
            .map_err(|e| dataflow::Error::InstallError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
    use crate::dataflow::PackageKey;
    use std::collections::HashMap;

    #[test]
    fn test_merge() {
        let mut new_lockfile_packages_map = HashMap::new();
        let pkg_1 = PackageKey::new_registry_package("_/foo", "1.1.0");
        let pkg_2 = PackageKey::new_registry_package("_/bar", "2.0.0");
        new_lockfile_packages_map.insert(
            pkg_1,
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );
        new_lockfile_packages_map.insert(
            pkg_2,
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );
        let new_lockfile_packages = LockfilePackages {
            packages: new_lockfile_packages_map,
        };

        let mut old_lockfile_packages_map = HashMap::new();
        let pkg_1_old = PackageKey::new_registry_package("_/foo", "1.0.0");
        let pkg_2_old = PackageKey::new_registry_package("_/qux", "3.0.0");
        old_lockfile_packages_map.insert(
            pkg_1_old,
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );
        old_lockfile_packages_map.insert(
            pkg_2_old,
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );

        let old_lockfile_packages = LockfilePackages {
            packages: old_lockfile_packages_map,
        };

        let result = MergedLockfilePackages::merge(new_lockfile_packages, old_lockfile_packages);

        assert!(result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/foo", "1.1.0")));
        assert!(!result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/foo", "1.0.0")));
        assert!(result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/bar", "2.0.0")));
        assert!(!result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/qux", "3.0.0")));
        assert_eq!(2, result.packages.len());
    }
}
