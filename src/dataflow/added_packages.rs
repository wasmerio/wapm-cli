use crate::dataflow::PackageKey;
use std::collections::hash_set::HashSet;

/// Holds packages that are added via the command line
#[derive(Default)]
pub struct AddedPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> AddedPackages<'a> {
    pub fn new_from_str_pairs(added_packages: Vec<(&'a str, &'a str)>) -> Self {
        let packages = added_packages
            .into_iter()
            .map(|(n, v)| PackageKey::new_registry_package(n, v))
            .collect();
        Self { packages }
    }

    pub fn prune_already_installed_packages(
        self,
        lockfile_packages_keys: HashSet<PackageKey<'a>>,
    ) -> Self {
        let added_packages = self.packages;
        let packages = added_packages
            .difference(&lockfile_packages_keys)
            .cloned()
            .collect();
        Self { packages }
    }

    pub fn add_missing_packages(self, missing_package_keys: HashSet<PackageKey<'a>>) -> Self {
        let added_packages = self.packages;
        let packages = added_packages
            .union(&missing_package_keys)
            .cloned()
            .collect();
        Self { packages }
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::added_packages::AddedPackages;
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::PackageKey;
    use std::collections::hash_set::HashSet;
    use std::collections::HashMap;

    #[test]
    fn prune_test() {
        let mut packages = HashSet::new();
        packages.insert(PackageKey::new_registry_package("_/foo", "1.1.0"));
        packages.insert(PackageKey::new_registry_package("_/bar", "2.0.0"));
        let added_packages = AddedPackages { packages };

        let mut packages = HashMap::new();
        packages.insert(
            PackageKey::new_registry_package("_/foo", "1.0.0"),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/bar", "2.0.0"),
            LockfilePackage::default(),
        );
        let existing_lockfile_packages = LockfilePackages { packages };

        let existing_lockfile_keys = existing_lockfile_packages.package_keys();

        let pruned_packages =
            added_packages.prune_already_installed_packages(existing_lockfile_keys);
        assert!(pruned_packages
            .packages
            .contains(&PackageKey::new_registry_package("_/foo", "1.1.0")));
        assert_eq!(1, pruned_packages.packages.len())
    }
}
