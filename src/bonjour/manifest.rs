use crate::bonjour::{BonjourError, PackageData, PackageId};
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
use std::collections::btree_map::BTreeMap;
use toml::Value;
use std::path::Path;
use std::fs;

pub struct ManifestSource {
    source: Option<String>,
}

impl ManifestSource {
    pub fn new<P: AsRef<Path>>(directory: P) -> Self {
        let directory = directory.as_ref();
        if !directory.is_dir() {
            return Self { source: None };
        }
        let manifest_path_buf = directory.join(MANIFEST_FILE_NAME);
        let source = fs::read_to_string(&manifest_path_buf).ok();
        Self { source }
    }
}


#[derive(Debug)]
pub enum ManifestResult {
    Manifest(Manifest),
    NoManifest,
    ManifestError(BonjourError),
}

impl ManifestResult {
    pub fn from_source(source: &ManifestSource) -> ManifestResult {
        source.source
            .as_ref()
            .map(|s| match toml::from_str::<Manifest>(s) {
                Ok(m) => ManifestResult::Manifest(m),
                Err(e) => ManifestResult::ManifestError(BonjourError::ManifestTomlParseError(
                    e.to_string(),
                )),
            })
            .unwrap_or(ManifestResult::NoManifest)
    }
}

pub struct ManifestData<'a> {
    pub package_data: Option<BTreeMap<PackageId<'a>, PackageData<'a>>>,
}

impl<'a> ManifestData<'a> {
    pub fn new_from_result(result: &'a ManifestResult) -> Result<Self, BonjourError> {
        match result {
            ManifestResult::Manifest(ref manifest) => Self::new_from_manifest(manifest),
            ManifestResult::NoManifest => Ok(Self {package_data: None}),
            ManifestResult::ManifestError(e) => Err(e.clone()),
        }
    }

    fn new_from_manifest(manifest: &'a Manifest) -> Result<Self, BonjourError> {
        let package_data = if let Manifest {
            dependencies: Some(ref dependencies),
//            package,
            ..
        } = manifest
        {
            dependencies
                .iter()
                .map(|(name, value)| match value {
                    Value::String(ref version) => Ok((
                        PackageId::WapmRegistryPackage { name, version },
                        PackageData::ManifestDependencyPackage,
                    )),
                    _ => Err(BonjourError::DependencyVersionMustBeString(
                        name.to_string(),
                    )),
                })
                .collect::<Result<BTreeMap<PackageId, PackageData>, BonjourError>>()?
        } else {
            BTreeMap::new()
        };
        Ok(ManifestData { package_data: Some(package_data) })
    }

    pub fn add_additional_packages(&mut self, added_packages: &Vec<(&'a str, &'a str)>) {
        if let Some(ref mut package_data) = self.package_data {
            for (name, version) in added_packages {
                let id = PackageId::new_registry_package(name, version);
                package_data.insert(id, PackageData::ManifestDependencyPackage);
            }
        }
        else {
            let mut package_data = BTreeMap::new();
            for (name, version) in added_packages {
                let id = PackageId::new_registry_package(name, version);
                package_data.insert(id, PackageData::ManifestDependencyPackage);
            }
            self.package_data = Some(package_data);
        }
    }
}
