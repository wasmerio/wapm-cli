use crate::bonjour::{BonjourError, PackageData, PackageKey, WapmPackageKey};
use crate::dependency_resolver::{PackageRegistry, PackageRegistryLike};
use std::collections::btree_map::BTreeMap;
use std::collections::btree_set::BTreeSet;

pub struct RemotePackageData {
    pub source: Vec<(String, String, String)>,
    //    pub data: BTreeMap<PackageKey<'a>, PackageData<'a>>,
}

impl<'a> RemotePackageData {
    //    pub fn new_from_package_keys(added_set: BTreeSet<PackageKey<'a>>) -> Result<Self, BonjourError> {
    //        let added_set = added_set
    //            .into_iter()
    //            .filter_map(|d| match d {
    //                PackageKey::WapmPackage(k) => Some(k),
    //                _ => None,
    //            })
    //            .collect::<Vec<_>>();
    //        PackageRegistry::better_sync_packages(added_set)
    //            .map(|source| Self { source })
    //            .map_err(|e| BonjourError::InstallError(e.to_string()))
    //    }
}
