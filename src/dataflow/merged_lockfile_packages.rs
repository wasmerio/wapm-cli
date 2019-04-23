use crate::data::lock::lockfile::{CommandMap, Lockfile, ModuleMap};
use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
use crate::dataflow::{PackageKey, WapmPackageKey};
use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::HashMap;
use std::path::Path;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not save generated lockfile because {}.", _0)]
    FailedToSaveLockfile(String),
}

/// Merge two sets, and keep upgraded packages and all other unchanged packages.
/// Remove changed packages e.g. upgraded versions.
#[derive(Clone, Debug)]
pub struct MergedLockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage>,
}

impl<'a> MergedLockfilePackages<'a> {
    pub fn merge(
        new_packages: LockfilePackages<'a>,
        old_packages: RetainedLockfilePackages<'a>,
    ) -> Self {
        let mut unique_packages = HashMap::new();
        for (key, data) in old_packages.packages {
            let name = match key {
                PackageKey::WapmPackage(ref k) => k.name.clone(),
                _ => panic!("Non wapm registry keys are unsupported."),
            };
            unique_packages.insert(name, (key, data));
        }
        for (key, data) in new_packages.packages {
            let name = match key {
                PackageKey::WapmPackage(ref k) => k.name.clone(),
                _ => panic!("Non wapm registry keys are unsupported."),
            };
            unique_packages.insert(name, (key, data));
        }
        let packages: HashMap<_, _> = unique_packages
            .into_iter()
            .map(|(_, (key, data))| (key, data))
            .collect();

        Self { packages }
    }

    pub fn generate_lockfile(self, directory: &'a Path) -> Result<(), Error> {
        let mut modules: ModuleMap = BTreeMap::new();
        let mut commands: CommandMap = BTreeMap::new();
        for (key, package) in self.packages {
            match key {
                PackageKey::WapmPackage(WapmPackageKey { name, version }) => {
                    let versions = modules.entry(name.to_owned().to_string()).or_default();
                    let modules = versions.entry(version.to_owned().to_string()).or_default();
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
            .map_err(|e| Error::FailedToSaveLockfile(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
    use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
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

        let old_lockfile_packages = RetainedLockfilePackages {
            packages: old_lockfile_packages_map,
        };

        let result = MergedLockfilePackages::merge(new_lockfile_packages, old_lockfile_packages);

        // should now contain all old packages, and upgraded new ones
        assert!(result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/foo", "1.1.0")));
        assert!(result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/bar", "2.0.0")));
        assert!(result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/qux", "3.0.0")));

        // should no longer contain the old foo package
        assert!(!result
            .packages
            .contains_key(&PackageKey::new_registry_package("_/foo", "1.0.0")));

        assert_eq!(3, result.packages.len());
    }
}
