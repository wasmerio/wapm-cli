use crate::data::manifest::Manifest;
use crate::dataflow::added_packages::AddedPackages;
use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
use crate::dataflow::installed_packages::{InstalledPackages, RegistryInstaller};
use crate::dataflow::local_package::LocalPackage;
use crate::dataflow::lockfile_packages::{LockfileError, LockfilePackages, LockfileResult};
use crate::dataflow::manifest_packages::{ManifestPackages, ManifestResult};
use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
use crate::dataflow::removed_lockfile_packages::RemovedLockfilePackages;
use crate::dataflow::removed_packages::RemovedPackages;
use crate::dataflow::resolved_packages::{RegistryResolver, ResolvedPackages};
use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
use semver::{Version, VersionReq};
use std::borrow::{Borrow, Cow};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::Path;

pub mod added_packages;
pub mod bin_script;
pub mod bindings;
pub mod changed_manifest_packages;
pub mod find_command_result;
pub mod installed_packages;
pub mod interfaces;
pub mod local_package;
pub mod lockfile_packages;
pub mod manifest_packages;
pub mod merged_lockfile_packages;
pub mod removed_lockfile_packages;
pub mod removed_packages;
pub mod resolved_packages;
pub mod retained_lockfile_packages;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Could not open manifest. {0}")]
    Manifest(manifest_packages::Error),
    #[error("Could not open lockfile. {0}")]
    Lockfile(LockfileError),
    #[error("Could generate lockfile. {0}")]
    GenerateLockfile(merged_lockfile_packages::Error),
    #[error("Could not install package(s). {0}")]
    Install(installed_packages::Error),
    #[error("Could not resolve package(s). {0}")]
    Resolve(resolved_packages::Error),
    #[error("Could not save manifest file because {0}.")]
    Save(String),
    #[error("Could not install new packages. {0}")]
    Add(added_packages::Error),
    #[error("Could not operate on local package data. {0}")]
    LocalPackage(local_package::Error),
    #[error("Could not cleanup old artifacts. {0}")]
    Cleanup(removed_lockfile_packages::Error),
    #[error("Attempting to install multiple versions of package {0} ({1} and {2})")]
    DuplicatePackage(String, String, String),
}

/// A package key for a package in the wapm.io registry.
/// This Is currently defined as name and a version.
#[derive(Clone, Debug, Eq, Hash, PartialOrd, PartialEq)]
pub struct WapmPackageKey<'a> {
    pub name: Cow<'a, str>,
    pub version: Version,
}

/// A range of versions for a package in the wapm.io registry.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WapmPackageRange<'a> {
    pub name: Cow<'a, str>,
    pub version_req: VersionReq,
}

impl<'a> fmt::Display for WapmPackageKey<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{} {}", self.name, self.version)
    }
}

pub fn detect_duplicate_packages(packages: &HashSet<PackageKey>) -> Result<(), Error> {
    let mut seen_pkg_versions = HashMap::new();

    for pkg in packages.iter() {
        if let PackageKey::WapmPackage(WapmPackageKey { name, version }) = pkg {
            if let Some(old_version) = seen_pkg_versions.insert(name, version) {
                // we sort the versions so that output is stable
                let mut versions = [old_version.to_string(), version.to_string()];
                versions.sort();
                return Err(Error::DuplicatePackage(
                    name.to_string(),
                    versions[0].clone(),
                    versions[1].clone(),
                ));
            }
        }
    }

    Ok(())
}

/// A package key can be anything reference to a package, be it a wapm.io registry, a local directory.
/// Currently, only wapm.io keys are supported.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PackageKey<'a> {
    WapmPackage(WapmPackageKey<'a>),
    WapmPackageRange(WapmPackageRange<'a>),
}

impl<'a> PackageKey<'a> {
    /// Convenience constructor for wapm.io registry keys.
    pub fn new_registry_package<S>(name: S, version: Version) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        PackageKey::WapmPackage(WapmPackageKey {
            name: name.into(),
            version,
        })
    }
    pub fn new_registry_package_range<S>(name: S, version_req: VersionReq) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        PackageKey::WapmPackageRange(WapmPackageRange {
            name: name.into(),
            version_req,
        })
    }

    pub fn matches(&self, range: &WapmPackageRange) -> bool {
        match self {
            PackageKey::WapmPackage(key) => {
                key.name == range.name && range.version_req.matches(&key.version)
            }
            _ => false,
        }
    }
}

impl<'a> fmt::Display for PackageKey<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            PackageKey::WapmPackage(wapm_package_key) => write!(f, "{}", wapm_package_key),
            _ => panic!("Non wapm registry keys are unsupported."),
        }
    }
}

pub fn normalize_global_namespace_package_name(package_name: Cow<str>) -> Cow<str> {
    if !package_name.contains('/') {
        Cow::Owned(format!("_/{}", package_name))
    } else {
        package_name
    }
}

/// Normalize global namespace package names that are using the shorthand e.g. convert pkg to "_/pkg"
pub fn normalize_global_namespace(key: PackageKey) -> PackageKey {
    let new_key = match key {
        PackageKey::WapmPackage(WapmPackageKey {
            ref name,
            ref version,
        }) if !name.contains('/') => {
            let name = format!("_/{}", name);
            PackageKey::new_registry_package(name, version.clone())
        }
        PackageKey::WapmPackageRange(WapmPackageRange {
            ref name,
            ref version_req,
        }) if !name.contains('/') => {
            let name = format!("_/{}", name);
            PackageKey::new_registry_package_range(name, version_req.clone())
        }
        key => key,
    };
    new_key
}

/// If there is no mainfest, then this is a non-manifest project. All installations are retained
/// in the lockfile, and installs are additive.
/// This function returns a bool on success indicating if any changes were applied
pub fn update_with_no_manifest<P: AsRef<Path>>(
    directory: P,
    added_packages: AddedPackages,
    removed_packages: RemovedPackages,
) -> Result<bool, Error> {
    let directory = directory.as_ref();
    // get lockfile data
    let lockfile_result = LockfileResult::find_in_directory(directory);
    let mut lockfile_packages =
        LockfilePackages::new_from_result(lockfile_result).map_err(Error::Lockfile)?;
    detect_duplicate_packages(&added_packages.packages)?;

    // capture the initial lockfile keys before any modifications
    let initial_package_keys: HashSet<_> = lockfile_packages.package_keys();

    let removed_lockfile_packages = RemovedLockfilePackages::from_removed_packages_and_lockfile(
        &removed_packages,
        &lockfile_packages,
    );

    // cleanup any old artifacts
    removed_lockfile_packages
        .cleanup_old_packages(directory)
        .map_err(Error::Cleanup)?;

    // remove/uninstall packages
    lockfile_packages.remove_packages(removed_packages);

    // check that the added packages are not already installed
    let lockfile_package_keys = lockfile_packages.package_keys();
    let added_packages = added_packages.prune_already_installed_packages(lockfile_package_keys);
    // check for missing packages e.g. deleting stuff from wapm_packages
    // install any missing or newly added packages
    let missing_packages = lockfile_packages.find_missing_packages(directory);
    let added_packages = added_packages.add_missing_packages(missing_packages);

    let resolved_packages =
        ResolvedPackages::new_from_added_packages::<RegistryResolver>(added_packages)
            .map_err(Error::Resolve)?;
    let installed_packages =
        InstalledPackages::install::<RegistryInstaller>(directory, resolved_packages, false)
            .map_err(Error::Install)?;
    let added_lockfile_data =
        LockfilePackages::from_installed_packages(&installed_packages).map_err(Error::Lockfile)?;

    let retained_lockfile_packages =
        RetainedLockfilePackages::from_lockfile_packages(lockfile_packages);

    // merge the lockfile data, and generate the new lockfile
    let final_lockfile_data =
        MergedLockfilePackages::merge(added_lockfile_data, retained_lockfile_packages);
    let final_package_keys: HashSet<_> = final_lockfile_data.packages.keys().cloned().collect();
    if final_package_keys != initial_package_keys {
        final_lockfile_data
            .generate_lockfile(directory)
            .map_err(Error::GenerateLockfile)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// If there is a manifest, then we construct lockfile data from manifest dependencies, and merge
/// with existing lockfile data.
/// This function returns a bool on success indicating if any changes were applied
pub fn update_with_manifest<P: AsRef<Path>>(
    directory: P,
    manifest: Manifest,
    added_packages: AddedPackages,
    removed_packages: RemovedPackages,
) -> Result<bool, Error> {
    let directory = directory.as_ref();

    let mut manifest_packages =
        ManifestPackages::new_from_manifest_and_added_packages(&manifest, &added_packages)
            .map_err(Error::Manifest)?;

    detect_duplicate_packages(&manifest_packages.packages)?;

    // remove/uninstall packages
    manifest_packages.remove_packages(&removed_packages);

    // get lockfile data
    let lockfile_result = LockfileResult::find_in_directory(directory);
    let lockfile_packages =
        LockfilePackages::new_from_result(lockfile_result).map_err(Error::Lockfile)?;
    // store lockfile package keys before updating it
    let initial_package_keys = lockfile_packages.package_keys();

    // get the local package modules and commands from the manifest
    let local_package =
        LocalPackage::new_from_local_package_in_manifest(&manifest).map_err(Error::LocalPackage)?;

    let changed_manifest_data =
        ChangedManifestPackages::get_changed_packages_from_manifest_and_lockfile(
            &manifest_packages,
            &lockfile_packages,
        );

    let packages_to_install = AddedPackages {
        packages: changed_manifest_data.packages,
    };

    let missing_lockfile_packages = lockfile_packages.find_missing_packages(directory);
    let new_added_packages = packages_to_install.add_missing_packages(missing_lockfile_packages);

    let removed_lockfile_packages =
        RemovedLockfilePackages::from_manifest_and_lockfile(&manifest_packages, &lockfile_packages);

    // cleanup any old artifacts
    removed_lockfile_packages
        .cleanup_old_packages(directory)
        .map_err(Error::Cleanup)?;

    let retained_lockfile_packages =
        RetainedLockfilePackages::from_manifest_and_lockfile(&manifest_packages, lockfile_packages);

    let resolved_manifest_packages =
        ResolvedPackages::new_from_added_packages::<RegistryResolver>(new_added_packages)
            .map_err(Error::Resolve)?;
    let installed_manifest_packages = InstalledPackages::install::<RegistryInstaller>(
        directory,
        resolved_manifest_packages,
        false,
    )
    .map_err(Error::Install)?;
    let mut manifest_lockfile_data =
        LockfilePackages::from_installed_packages(&installed_manifest_packages)
            .map_err(Error::Lockfile)?;

    manifest_lockfile_data.extend(local_package.into());

    // merge the lockfile data, and generate the new lockfile
    let final_lockfile_data =
        MergedLockfilePackages::merge(manifest_lockfile_data, retained_lockfile_packages);
    let final_package_keys: HashSet<_> = final_lockfile_data.packages.keys().cloned().collect();

    final_lockfile_data
        .generate_lockfile(directory)
        .map_err(Error::GenerateLockfile)?;

    // update the manifest, if applicable
    if final_package_keys != initial_package_keys {
        update_manifest(manifest.clone(), &added_packages, &removed_packages)?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// The function that starts lockfile dataflow. This function finds a manifest and a lockfile,
/// calculates differences, installs missing dependencies, and finally generates a new lockfile.
pub fn update<P: AsRef<Path>>(
    added_packages: Vec<(&str, &str)>,
    removed_packages: Vec<&str>,
    directory: P,
) -> Result<bool, Error> {
    let directory = directory.as_ref();
    let added_packages = AddedPackages::new_from_str_pairs(added_packages).map_err(Error::Add)?;
    let removed_packages = RemovedPackages::new_from_package_names(removed_packages);
    let manifest_result = ManifestResult::find_in_directory(directory);
    match manifest_result {
        ManifestResult::NoManifest => {
            update_with_no_manifest(directory, added_packages, removed_packages)
        }
        ManifestResult::Manifest(manifest) => {
            update_with_manifest(directory, manifest, added_packages, removed_packages)
        }
        ManifestResult::ManifestError(e) => Err(Error::Manifest(e)),
    }
}

/// Updates the manifest and saves it
pub fn update_manifest(
    manifest: Manifest,
    added_packages: &AddedPackages,
    removed_packages: &RemovedPackages,
) -> Result<(), Error> {
    if added_packages.packages.is_empty() && removed_packages.packages.is_empty() {
        return Ok(());
    }

    let mut manifest = manifest;
    for key in added_packages.packages.iter().cloned() {
        match key {
            PackageKey::WapmPackageRange(WapmPackageRange { name, version_req }) => {
                manifest.add_dependency(name.to_string(), version_req.to_string());
            }
            PackageKey::WapmPackage(WapmPackageKey { name, version }) => {
                manifest.add_dependency(name.to_string(), version.to_string());
            }
        }
    }

    for package_name in &removed_packages.packages {
        manifest.remove_dependency(package_name.borrow());
    }

    manifest.save().map_err(|e| Error::Save(e.to_string()))?;

    Ok(())
}
