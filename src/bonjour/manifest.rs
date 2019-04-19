use crate::bonjour::{BonjourError, PackageData, PackageId};
use crate::manifest::Manifest;
use std::collections::btree_map::BTreeMap;
use toml::Value;

#[derive(Debug)]
pub enum ManifestResult {
    Manifest(Manifest),
    NoManifest,
    ManifestError(BonjourError),
}

impl ManifestResult {
    pub fn from_optional_source(source: &Option<String>) -> ManifestResult {
        source
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
    pub package_data: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> ManifestData<'a> {
    pub fn new_from_result(result: &'a ManifestResult) -> Result<Option<Self>, BonjourError> {
        match result {
            ManifestResult::Manifest(ref manifest) => match Self::new_from_manifest(manifest) {
                Ok(md) => Ok(Some(md)),
                Err(e) => Err(e),
            },
            ManifestResult::NoManifest => Ok(None),
            ManifestResult::ManifestError(e) => Err(e.clone()),
        }
    }

    fn new_from_manifest(manifest: &'a Manifest) -> Result<Self, BonjourError> {
        let package_data = if let Manifest {
            package,
            dependencies: Some(ref dependencies),
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
        Ok(ManifestData { package_data })
    }
}
