use crate::data::manifest::{Manifest, MANIFEST_FILE_NAME};
use crate::dataflow::added_packages::AddedPackages;
use crate::dataflow::{PackageKey, WapmPackageKey};
use std::borrow::Cow;
use std::collections::hash_set::HashSet;
use std::fs;
use std::path::Path;
use toml::Value;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not parse manifest because {}.", _0)]
    ManifestTomlParseError(String),
    #[fail(display = "Could not parse manifest because {}.", _0)]
    IoError(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
}

/// A ternary for a manifest: Some, None, Error.
#[derive(Debug)]
pub enum ManifestResult {
    Manifest(Manifest),
    NoManifest,
    ManifestError(Error),
}

impl ManifestResult {
    pub fn find_in_directory<P: AsRef<Path>>(directory: P) -> Self {
        let directory = directory.as_ref();
        if !directory.is_dir() {
            ManifestResult::ManifestError(Error::IoError(
                "Manifest must be a file named `wapm.toml`.".to_string(),
            ));
        }
        let manifest_path_buf = directory.join(MANIFEST_FILE_NAME);
        if !manifest_path_buf.is_file() {
            ManifestResult::ManifestError(Error::IoError(
                "Manifest must be a file named `wapm.toml`.".to_string(),
            ));
        }
        let source = match fs::read_to_string(&manifest_path_buf) {
            Ok(s) => s,
            Err(_) => return ManifestResult::NoManifest,
        };
        match toml::from_str::<Manifest>(&source) {
            Ok(m) => ManifestResult::Manifest(m),
            Err(e) => ManifestResult::ManifestError(Error::ManifestTomlParseError(e.to_string())),
        }
    }
}

/// A convenient structure containing all modules and commands for a package stored in manifest.
#[derive(Clone, Debug, Default)]
pub struct ManifestPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> ManifestPackages<'a> {
    pub fn new_from_manifest_and_added_packages(
        manifest: &'a Manifest,
        added_packages: AddedPackages<'a>,
    ) -> Result<Self, Error> {
        let mut packages = if let Manifest {
            dependencies: Some(ref dependencies),
            ..
        } = manifest
        {
            dependencies
                .iter()
                .map(|(name, value)| match value {
                    Value::String(ref version) => Ok(PackageKey::WapmPackage(WapmPackageKey {
                        name: Cow::Borrowed(name),
                        version: Cow::Borrowed(version),
                    })),
                    _ => Err(Error::DependencyVersionMustBeString(name.to_string())),
                })
                .collect::<Result<HashSet<PackageKey>, Error>>()
        } else {
            Ok(HashSet::new())
        }?;

        for added_package_key in added_packages.packages {
            packages.insert(added_package_key);
        }

        Ok(Self { packages })
    }

    pub fn keys(&self) -> HashSet<PackageKey<'a>> {
        self.packages.iter().cloned().collect()
    }
}
