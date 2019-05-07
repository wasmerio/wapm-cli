use crate::dataflow::lockfile_packages::LockfilePackages;
use crate::dataflow::manifest_packages::ManifestPackages;
use crate::dataflow::PackageKey;
use std::collections::hash_set::HashSet;

/// Contains the package IDs for dependencies that have changed between a manifest and an existing lockfile.
#[derive(Clone, Debug)]
pub struct ChangedManifestPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> ChangedManifestPackages<'a> {
    /// Construct with packages that have been added to the manifest data and did not previously exist in lockfile.
    pub fn get_changed_packages_from_manifest_and_lockfile(
        manifest_data: &ManifestPackages<'a>,
        lockfile_data: &LockfilePackages<'a>,
    ) -> Self {
        let lockfile_keys = lockfile_data.package_keys();
        let packages = manifest_data
            .keys()
            .into_iter()
            .filter(|package_key| {
                match package_key {
                    // if the package version is exact, then do a contains check,
                    // the lockfile will always contain exact package keys
                    PackageKey::WapmPackage(_) => !lockfile_keys.contains(package_key),
                    // if the package version is a range, then linear search the lockfile keys
                    // and do a semver match
                    PackageKey::WapmPackageRange(range) => {
                        // if the range does not match any of the lockfile keys, then we have a changed package
                        lockfile_keys.iter().find(|k| k.matches(range)).is_none()
                    }
                }
            })
            .collect::<HashSet<PackageKey<'a>>>();
        Self { packages }
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::manifest_packages::ManifestPackages;
    use crate::dataflow::PackageKey;
    use std::collections::hash_map::HashMap;
    use std::collections::hash_set::HashSet;

    #[test]
    fn no_shared_dependencies() {
        let manifest_data = ManifestPackages::default();
        let lockfile_data = LockfilePackages::default();
        let changed_packages =
            ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
                &manifest_data,
                &lockfile_data,
            );
        assert_eq!(0, changed_packages.packages.len());
    }

    #[test]
    fn two_added_range_packages() {
        let mut manifest_package_keys = HashSet::new();
        let package_key_foo = PackageKey::new_registry_package_range(
            "_/foo",
            semver::VersionReq::parse("^1").unwrap(),
        );
        let package_key_bar = PackageKey::new_registry_package_range(
            "_/bar",
            semver::VersionReq::parse("*").unwrap(),
        );
        manifest_package_keys.insert(package_key_foo);
        manifest_package_keys.insert(package_key_bar);
        let manifest_data = ManifestPackages {
            packages: manifest_package_keys,
        };
        let lockfile_data = LockfilePackages {
            packages: HashMap::new(),
        };
        let changed_packages =
            ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
                &manifest_data,
                &lockfile_data,
            );
        assert_eq!(2, changed_packages.packages.len());
    }

    #[test]
    fn one_added_package() {
        let mut manifest_package_keys = HashSet::new();
        let package_key = PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0));
        manifest_package_keys.insert(package_key);
        let manifest_data = ManifestPackages {
            packages: manifest_package_keys,
        };
        let lockfile_data = LockfilePackages {
            packages: HashMap::new(),
        };
        let changed_packages =
            ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
                &manifest_data,
                &lockfile_data,
            );
        assert_eq!(1, changed_packages.packages.len());
    }

    #[test]
    fn both_share_same_package() {
        let mut manifest_package_keys = HashSet::new();
        let package_key = PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0));
        manifest_package_keys.insert(package_key.clone());
        let manifest_data = ManifestPackages {
            packages: manifest_package_keys,
        };
        let mut lockfile_packages = HashMap::new();
        let lockfile_package = LockfilePackage {
            modules: vec![],
            commands: vec![],
        };
        lockfile_packages.insert(package_key, lockfile_package);
        let lockfile_data = LockfilePackages {
            packages: lockfile_packages,
        };
        let changed_packages =
            ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
                &manifest_data,
                &lockfile_data,
            );
        assert_eq!(0, changed_packages.packages.len());
    }

    #[test]
    fn one_shared_and_one_added_range() {
        let mut manifest_package_keys = HashSet::new();
        let package_key_1 = PackageKey::new_registry_package_range(
            "_/foo",
            semver::VersionReq::parse("^1").unwrap(),
        );
        let package_key_2 =
            PackageKey::new_registry_package("_/bar", semver::Version::new(2, 0, 0));
        manifest_package_keys.insert(package_key_1.clone());
        manifest_package_keys.insert(package_key_2.clone());
        // manifest has package_key_1 and package_key_2
        let manifest_data = ManifestPackages {
            packages: manifest_package_keys,
        };
        let mut lockfile_packages = HashMap::new();
        // lockfile has package_key_2
        let lockfile_package = LockfilePackage {
            modules: vec![],
            commands: vec![],
        };
        lockfile_packages.insert(package_key_2, lockfile_package);
        let lockfile_data = LockfilePackages {
            packages: lockfile_packages,
        };
        let changed_packages =
            ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
                &manifest_data,
                &lockfile_data,
            );
        assert_eq!(1, changed_packages.packages.len());
    }

    #[test]
    fn one_shared_and_one_added() {
        let mut manifest_package_keys = HashSet::new();
        let package_key_1 =
            PackageKey::new_registry_package("_/foo", semver::Version::new(1, 0, 0));
        let package_key_2 =
            PackageKey::new_registry_package("_/bar", semver::Version::new(2, 0, 0));
        manifest_package_keys.insert(package_key_1.clone());
        manifest_package_keys.insert(package_key_2.clone());
        // manifest has package_key_1 and package_key_2
        let manifest_data = ManifestPackages {
            packages: manifest_package_keys,
        };
        let mut lockfile_packages = HashMap::new();
        // lockfile has package_key_1
        let lockfile_package = LockfilePackage {
            modules: vec![],
            commands: vec![],
        };
        lockfile_packages.insert(package_key_1, lockfile_package);
        let lockfile_data = LockfilePackages {
            packages: lockfile_packages,
        };
        let changed_packages =
            ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
                &manifest_data,
                &lockfile_data,
            );
        assert_eq!(1, changed_packages.packages.len());
    }
}
