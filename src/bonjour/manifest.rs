use crate::bonjour::{BonjourError, PackageKey};
use std::fs;
use std::path::Path;
use toml::Value;
use crate::cfg_toml::manifest::{MANIFEST_FILE_NAME, Manifest};
use std::collections::btree_set::BTreeSet;

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
        source
            .source
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
    pub package_keys: Option<BTreeSet<PackageKey<'a>>>,
}

impl<'a> ManifestData<'a> {
    pub fn new_from_result(result: &'a ManifestResult) -> Result<Self, BonjourError> {
        match result {
            ManifestResult::Manifest(ref manifest) => Self::new_from_manifest(manifest),
            ManifestResult::NoManifest => Ok(Self { package_keys: None }),
            ManifestResult::ManifestError(e) => Err(e.clone()),
        }
    }

    fn new_from_manifest(manifest: &'a Manifest) -> Result<Self, BonjourError> {
        match manifest {
            Manifest { dependencies: Some(ref dependencies), .. } => {
                dependencies
                    .iter()
                    .map(|(name, value)| match value {
                        Value::String(ref version) => Ok(
                            PackageKey::WapmRegistryPackage { name, version },
                        ),
                        _ => Err(BonjourError::DependencyVersionMustBeString(
                            name.to_string(),
                        )),
                    })
                    .collect::<Result<BTreeSet<PackageKey>, BonjourError>>()
                    .map(|package_keys| Self { package_keys: Some(package_keys) })
            },
            _ => {
                Ok(Self { package_keys: None })
            }
        }
    }

    pub fn add_additional_packages(&mut self, added_packages: &Vec<(&'a str, &'a str)>) {
        if let Some(ref mut package_keys) = self.package_keys {
            for (name, version) in added_packages {
                let key = PackageKey::new_registry_package(name, version);
                package_keys.insert(key);
            }
        } else {
            self.package_keys = Some(added_packages.into_iter()
                .map(|(n, v)| PackageKey::new_registry_package(n, v))
                .collect::<BTreeSet<_>>());
        }
    }
}
