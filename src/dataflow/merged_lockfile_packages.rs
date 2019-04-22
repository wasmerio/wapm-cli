use crate::dataflow::lockfile_packages::{LockfilePackages, LockfilePackage};
use std::collections::hash_set::HashSet;
use crate::dataflow::{PackageKey, WapmPackageKey};
use std::collections::hash_map::HashMap;
use std::path::Path;
use crate::dataflow;
use crate::cfg_toml::lock::lockfile::{CommandMap, Lockfile, ModuleMap};
use std::collections::btree_map::BTreeMap;

#[derive(Clone, Debug)]
pub struct MergedLockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage<'a>>,
}

impl<'a> MergedLockfilePackages<'a> {
    pub fn merge(new_packages: LockfilePackages<'a>, mut old_packages: LockfilePackages<'a>) -> Self {
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
        Self {packages: old_packages.packages }
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
//
//#[cfg(test)]
//mod test {
//    #[test]
//    fn test_merge() {
//
//    }
//}