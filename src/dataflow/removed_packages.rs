use crate::dataflow::normalize_global_namespace_package_name;
use std::borrow::Cow;
use std::collections::hash_set::HashSet;

/// Holds packages that are removed on the cli
#[derive(Debug, Default)]
pub struct RemovedPackages<'a> {
    pub packages: HashSet<Cow<'a, str>>,
}

impl<'a> RemovedPackages<'a> {
    pub fn new_from_package_names(removed_packages: Vec<&'a str>) -> Self {
        let packages = removed_packages
            .into_iter()
            .map(|package_name| normalize_global_namespace_package_name(package_name.into()))
            .collect();
        Self { packages }
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::removed_packages::RemovedPackages;
    use crate::dataflow::PackageKey;
    use std::collections::HashMap;

    #[test]
    fn remove_from_lockfile_packages() {
        let mut packages = HashMap::default();
        packages.insert(
            PackageKey::new_registry_package("_/foo", semver::Version::parse("1.0.0").unwrap()),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/bar", semver::Version::parse("2.0.0").unwrap()),
            LockfilePackage::default(),
        );
        let mut lockfile_packages = LockfilePackages { packages };

        let removed_packages = RemovedPackages::new_from_package_names(vec!["_/foo"]);

        lockfile_packages.remove_packages(removed_packages);

        assert_eq!(1, lockfile_packages.packages.len());
        lockfile_packages
            .packages
            .get(&PackageKey::new_registry_package(
                "_/bar",
                semver::Version::parse("2.0.0").unwrap(),
            ))
            .unwrap();
    }

    #[test]
    fn remove_from_lockfile_packages_using_global_namespace_shorthand() {
        let mut packages = HashMap::default();
        packages.insert(
            PackageKey::new_registry_package("_/foo", semver::Version::parse("1.0.0").unwrap()),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/bar", semver::Version::parse("2.0.0").unwrap()),
            LockfilePackage::default(),
        );
        let mut lockfile_packages = LockfilePackages { packages };

        let removed_packages = RemovedPackages::new_from_package_names(vec!["foo"]);

        lockfile_packages.remove_packages(removed_packages);

        assert_eq!(1, lockfile_packages.packages.len());
        lockfile_packages
            .packages
            .get(&PackageKey::new_registry_package(
                "_/bar",
                semver::Version::parse("2.0.0").unwrap(),
            ))
            .unwrap();
    }
}
