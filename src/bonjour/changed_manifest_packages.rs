use crate::bonjour::lockfile::LockfileData;
use crate::bonjour::manifest::ManifestData;
use crate::bonjour::{BonjourError, PackageKey};
use std::collections::hash_set::HashSet;

#[derive(Clone, Debug)]
pub struct ChangedManifestPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> ChangedManifestPackages<'a> {
    pub fn prune_unchanged_dependencies(
        manifest_data: ManifestData<'a>,
        lockfile_data: &LockfileData<'a>,
    ) -> Result<Self, BonjourError> {
        let packages = match manifest_data.package_keys {
            Some(m) => {
                let lockfile_keys: HashSet<PackageKey<'a>> =
                    lockfile_data.packages.keys().cloned().collect();
                let differences: HashSet<PackageKey<'a>> =
                    m.difference(&lockfile_keys).cloned().collect();
                differences
            }
            _ => HashSet::new(),
        };
        Ok(Self { packages })
    }
}
