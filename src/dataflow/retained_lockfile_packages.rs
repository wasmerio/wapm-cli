use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
use crate::dataflow::manifest_packages::ManifestPackages;
use crate::dataflow::PackageKey;
use std::collections::hash_set::HashSet;
use std::collections::HashMap;

pub struct RetainedLockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage>,
}

impl<'a> RetainedLockfilePackages<'a> {
    pub fn from_manifest_and_lockfile(
        manifest_packages: &'a ManifestPackages<'a>,
        lockfile_packages: LockfilePackages<'a>,
    ) -> Self {
        let manifest_keys = manifest_packages.keys();
        let lockfile_keys: HashSet<_> = lockfile_packages.packages.keys().cloned().collect();
        let keys: HashSet<_> = manifest_keys
            .intersection(&lockfile_keys)
            .cloned()
            .collect();

        let packages: HashMap<_, _> = lockfile_packages
            .packages
            .into_iter()
            .filter(|(k, _)| keys.contains(k))
            .collect();

        RetainedLockfilePackages { packages }
    }

    pub fn from_lockfile_packages(lockfile_packages: LockfilePackages<'a>) -> Self {
        Self {
            packages: lockfile_packages.packages,
        }
    }
}

#[cfg(test)]
mod retained_lockfile_packages_tests {
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::manifest_packages::ManifestPackages;
    use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
    use crate::dataflow::PackageKey;
    use std::collections::hash_set::HashSet;
    use std::collections::HashMap;

    #[test]
    fn retain_lockfile_packages() {
        let mut manifest_package_keys = HashSet::new();

        // one upgrade package, one new package
        manifest_package_keys.insert(PackageKey::new_registry_package(
            "_/foo",
            semver::Version::new(1, 1, 0),
        ));
        manifest_package_keys.insert(PackageKey::new_registry_package(
            "_/bar",
            semver::Version::new(2, 2, 0),
        ));
        manifest_package_keys.insert(PackageKey::new_registry_package(
            "_/baz",
            semver::Version::new(11, 11, 11),
        ));

        let manifest_packages = ManifestPackages {
            packages: manifest_package_keys,
        };

        let mut lockfile_package_map = HashMap::new();
        // one existing package that is upgraded, and one old package, and one unchanged
        lockfile_package_map.insert(
            PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0)),
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );
        lockfile_package_map.insert(
            PackageKey::new_registry_package("_/baz", semver::Version::new(11, 11, 11)),
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );
        lockfile_package_map.insert(
            PackageKey::new_registry_package("_/qux", semver::Version::new(3, 0, 0)),
            LockfilePackage {
                modules: vec![],
                commands: vec![],
            },
        );

        let lockfile_packages = LockfilePackages {
            packages: lockfile_package_map,
        };

        let retained_lockfile_packages = RetainedLockfilePackages::from_manifest_and_lockfile(
            &manifest_packages,
            lockfile_packages,
        );

        assert_eq!(1, retained_lockfile_packages.packages.len());
        // should only contain this one:
        assert!(retained_lockfile_packages.packages.contains_key(
            &PackageKey::new_registry_package("_/baz", semver::Version::new(11, 11, 11))
        ));
        // should not contain these:
        assert!(!retained_lockfile_packages.packages.contains_key(
            &PackageKey::new_registry_package("_/qux", semver::Version::new(3, 0, 0))
        ));
        assert!(!retained_lockfile_packages.packages.contains_key(
            &PackageKey::new_registry_package("_/foo", semver::Version::new(1, 1, 0))
        ));
        assert!(!retained_lockfile_packages.packages.contains_key(
            &PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0))
        ));
        assert!(!retained_lockfile_packages.packages.contains_key(
            &PackageKey::new_registry_package("_/bar", semver::Version::new(2, 2, 0))
        ));
    }
}
