use crate::dataflow::bin_script::delete_bin_script;
use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
use crate::dataflow::manifest_packages::ManifestPackages;
use crate::dataflow::removed_packages::RemovedPackages;
use crate::dataflow::{bin_script, PackageKey, WapmPackageKey};
use std::collections::hash_map::HashMap;
use std::collections::hash_set::HashSet;
use std::path::Path;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Could not cleanup uninstalled command \"{0}\". {1}")]
    CommandCleanupError(String, bin_script::Error),
}

#[derive(Clone, Debug)]
pub struct RemovedLockfilePackages<'a> {
    pub packages: HashMap<PackageKey<'a>, LockfilePackage>,
}

impl<'a> RemovedLockfilePackages<'a> {
    pub fn from_manifest_and_lockfile(
        manifest_packages: &'a ManifestPackages<'a>,
        lockfile_packages: &'a LockfilePackages<'a>,
    ) -> Self {
        // collect all removed packages
        let old_package_keys: HashSet<_> = lockfile_packages.packages.keys().cloned().collect();
        let packages = old_package_keys
            .difference(&manifest_packages.packages)
            .map(|key| {
                (
                    key.clone(),
                    lockfile_packages.packages.get(&key).cloned().unwrap(),
                )
            })
            .collect();
        Self { packages }
    }

    pub fn from_removed_packages_and_lockfile(
        removed_packages: &'a RemovedPackages<'a>,
        lockfile_packages: &'a LockfilePackages<'a>,
    ) -> Self {
        let packages = removed_packages
            .packages
            .iter()
            .cloned()
            .filter_map(|removed_package_name| {
                lockfile_packages
                    .packages
                    .iter()
                    .find(|(key, _)| match key {
                        PackageKey::WapmPackage(WapmPackageKey { name, .. }) => {
                            name == &removed_package_name
                        }
                        _ => unreachable!(
                            "Lockfile should only contain exact wapm package versions."
                        ),
                    })
                    .map(|(key, data)| (key.clone(), data.clone()))
            })
            .collect();
        Self { packages }
    }

    /// This will do the required cleanup of old artifacts like bin scripts and wapm packages
    pub fn cleanup_old_packages<P: AsRef<Path>>(self, directory: P) -> Result<(), Error> {
        let directory = directory.as_ref();
        for (_key, data) in self.packages {
            for command in data.commands {
                delete_bin_script(directory, command.name.clone())
                    .map_err(|e| Error::CommandCleanupError(command.name.clone(), e))?;
            }
            // TODO cleanup wapm_packages
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
    use crate::dataflow::manifest_packages::ManifestPackages;
    use crate::dataflow::removed_lockfile_packages::RemovedLockfilePackages;
    use crate::dataflow::removed_packages::RemovedPackages;
    use crate::dataflow::PackageKey;
    use std::collections::hash_set::HashSet;
    use std::collections::HashMap;

    #[test]
    fn get_removed_lockfile_packages_from_manifest_and_lockfile() {
        let mut manifest_packages = ManifestPackages::default();
        let mut packages = HashSet::default();
        packages.insert(PackageKey::new_registry_package(
            "_/foo",
            semver::Version::parse("1.0.0").unwrap(),
        ));
        packages.insert(PackageKey::new_registry_package(
            "_/bar",
            semver::Version::parse("2.0.0").unwrap(),
        ));
        manifest_packages.packages = packages;

        let mut lockfile_packages = LockfilePackages::default();
        let mut packages = HashMap::default();
        packages.insert(
            PackageKey::new_registry_package("_/foo", semver::Version::parse("1.0.0").unwrap()),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/bar", semver::Version::parse("2.0.0").unwrap()),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/baz", semver::Version::parse("3.0.0").unwrap()),
            LockfilePackage::default(),
        );
        lockfile_packages.packages = packages;

        let removed_lockfile_packages = RemovedLockfilePackages::from_manifest_and_lockfile(
            &manifest_packages,
            &lockfile_packages,
        );
        assert_eq!(1, removed_lockfile_packages.packages.len());
        removed_lockfile_packages
            .packages
            .get(&PackageKey::new_registry_package(
                "_/baz",
                semver::Version::parse("3.0.0").unwrap(),
            ))
            .unwrap();
    }

    #[test]
    fn get_removed_lockfile_packages_from_removed_packages_and_lockfile() {
        let mut removed_packages = RemovedPackages::default();
        let mut packages = HashSet::default();
        packages.insert("_/foo".into());
        packages.insert("_/bar".into());
        removed_packages.packages = packages;

        let mut lockfile_packages = LockfilePackages::default();
        let mut packages = HashMap::default();
        packages.insert(
            PackageKey::new_registry_package("_/foo", semver::Version::parse("1.0.0").unwrap()),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/bar", semver::Version::parse("2.0.0").unwrap()),
            LockfilePackage::default(),
        );
        packages.insert(
            PackageKey::new_registry_package("_/baz", semver::Version::parse("3.0.0").unwrap()),
            LockfilePackage::default(),
        );
        lockfile_packages.packages = packages;

        let removed_lockfile_packages = RemovedLockfilePackages::from_removed_packages_and_lockfile(
            &removed_packages,
            &lockfile_packages,
        );
        assert_eq!(2, removed_lockfile_packages.packages.len());
        removed_lockfile_packages
            .packages
            .get(&PackageKey::new_registry_package(
                "_/bar",
                semver::Version::parse("2.0.0").unwrap(),
            ))
            .unwrap();
        removed_lockfile_packages
            .packages
            .get(&PackageKey::new_registry_package(
                "_/foo",
                semver::Version::parse("1.0.0").unwrap(),
            ))
            .unwrap();
    }
}
