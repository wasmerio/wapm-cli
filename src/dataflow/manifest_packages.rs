use crate::data::manifest::{Manifest, MANIFEST_FILE_NAME};
use crate::dataflow::added_packages::AddedPackages;
use crate::dataflow::{normalize_global_namespace, PackageKey};
use semver::{Version, VersionReq};
use std::collections::hash_set::HashSet;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not parse manifest because {}.", _0)]
    ManifestTomlParseError(String),
    #[fail(display = "Could not parse manifest because {}.", _0)]
    IoError(String),
    #[fail(
        display = "Version {} for package {} must be a semantic version or a semantic version requirement.",
        _0, _1
    )]
    SemVerError(String, String),
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
    /// Construct package keys from the manifest and any other additional packages.
    /// Short-hand package names are transformed.
    pub fn new_from_manifest_and_added_packages(
        manifest: &'a Manifest,
        added_packages: &AddedPackages<'a>,
    ) -> Result<Self, Error> {
        let packages = Self::extract_package_keys(&manifest)?;
        let mut packages: HashSet<PackageKey> = packages
            .into_iter()
            .map(normalize_global_namespace)
            .collect();

        for added_package_key in added_packages.packages.iter().cloned() {
            packages.insert(added_package_key);
        }
        Ok(Self { packages })
    }

    pub fn keys(&self) -> HashSet<PackageKey<'a>> {
        self.packages.iter().cloned().collect()
    }

    /// Extract package keys from the manifest
    fn extract_package_keys(manifest: &'a Manifest) -> Result<Vec<PackageKey<'a>>, Error> {
        match manifest.dependencies {
            Some(ref dependencies) => {
                let result = dependencies
                    .iter()
                    .map(|(name, value)| (name.as_str(), value.as_str()))
                    .map(Self::parse_wapm_package_key)
                    .collect::<Result<Vec<_>, Error>>()?;
                Ok(result)
            }
            None => Ok(vec![]),
        }
    }

    /// Parse a raw pair of strings as an exact wapm package or a range. May fail with a semver
    /// error.
    fn parse_wapm_package_key(
        (name, version): (&'a str, &'a str),
    ) -> Result<PackageKey<'a>, Error> {
        if let Ok(version) = Version::parse(version) {
            Ok(PackageKey::new_registry_package(name, version))
        } else if let Ok(version_req) = VersionReq::parse(version) {
            Ok(PackageKey::new_registry_package_range(name, version_req))
        } else {
            Err(Error::SemVerError(name.to_string(), version.to_string()))
        }
    }
}
