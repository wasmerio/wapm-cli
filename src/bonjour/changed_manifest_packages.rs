use crate::bonjour::lockfile::LockfileData;
use crate::bonjour::manifest::ManifestData;
use crate::bonjour::{BonjourError, PackageKey};
use std::collections::hash_set::HashSet;

/// Contains the package IDs for dependencies that have changed between a manifest and an existing lockfile.
#[derive(Clone, Debug)]
pub struct ChangedManifestPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> ChangedManifestPackages<'a> {
    /// Construct with packages that have been added to the manifest data and did not previously exist in lockfile.
    pub fn prune_unchanged_dependencies(
        manifest_data: &ManifestData<'a>,
        lockfile_data: &LockfileData<'a>,
    ) -> Result<Self, BonjourError> {
        let lockfile_keys = lockfile_data.package_keys();
        let packages = manifest_data
            .keys()
            .difference(&lockfile_keys)
            .cloned()
            .collect::<HashSet<PackageKey<'a>>>();
        Ok(Self { packages })
    }
}
