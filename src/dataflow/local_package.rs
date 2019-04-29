use crate::data::lock::lockfile_command;
use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::LockfileModule;
use crate::data::manifest::Manifest;
use crate::dataflow::lockfile_packages::{LockfilePackage, LockfilePackages};
use crate::dataflow::PackageKey;
use std::collections::hash_map::HashMap;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not extract commands from manifest. {}", _0)]
    CouldNotExtractCommandsFromManifest(lockfile_command::Error),
}

pub struct LocalPackage<'a> {
    pub key: PackageKey<'a>,
    pub data: LockfilePackage,
}

impl<'a> LocalPackage<'a> {
    pub fn new_from_local_package_in_manifest(manifest: &'a Manifest) -> Result<Self, Error> {
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
            .collect::<Result<Vec<LockfileCommand>, lockfile_command::Error>>()
            .map_err(|e| Error::CouldNotExtractCommandsFromManifest(e))?;
        let key = PackageKey::new_registry_package(package_name, package_version.clone());
        let data = LockfilePackage { modules, commands };
        Ok(LocalPackage { key, data })
    }
}

impl<'a> Into<LockfilePackages<'a>> for LocalPackage<'a> {
    fn into(self) -> LockfilePackages<'a> {
        let mut packages = HashMap::new();
        packages.insert(self.key, self.data);
        LockfilePackages { packages }
    }
}
