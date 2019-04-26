use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::LockfileModule;
use crate::data::manifest::Manifest;
use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
use crate::dataflow::PackageKey;
use std::collections::hash_map::HashMap;

pub struct LocalPackage<'a> {
    pub key: PackageKey<'a>,
    pub data: LockfilePackage,
}

impl<'a> LocalPackage<'a> {
    pub fn new_from_local_package_in_manifest(manifest: &'a Manifest) -> Self {
        let package_name = manifest.package.name.as_str();
        let package_version = &manifest.package.version;
        let modules = manifest
            .module
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|m| LockfileModule::from_local_module(package_name, package_version, &m))
            .collect();
        let commands = manifest
            .command
            .as_ref()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|c| LockfileCommand::from_command(package_name, package_version.clone(), &c))
            .collect();
        let key = PackageKey::new_registry_package(package_name, package_version.clone());
        let data = LockfilePackage { modules, commands };
        LocalPackage { key, data }
    }
}

impl<'a> Into<LockfilePackages<'a>> for LocalPackage<'a> {
    fn into(self) -> LockfilePackages<'a> {
        let mut packages = HashMap::new();
        packages.insert(self.key, self.data);
        LockfilePackages { packages }
    }
}
