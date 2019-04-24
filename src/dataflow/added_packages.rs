use crate::dataflow::PackageKey;
use std::collections::hash_set::HashSet;

/// Holds packages that are added via the command line
pub struct AddedPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> AddedPackages<'a> {
    pub fn new_from_str_pairs(added_packages: Vec<(&'a str, &'a str)>) -> Self {
        let mut packages = HashSet::new();
        let added_package_keys = added_packages
            .into_iter()
            .map(|(n, v)| PackageKey::new_registry_package(n, v));
        for package_key in added_package_keys {
            packages.insert(package_key);
        }
        Self { packages }
    }
}
