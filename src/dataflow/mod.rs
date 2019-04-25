use crate::data::manifest::Manifest;
use crate::dataflow::added_packages::AddedPackages;
use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
use crate::dataflow::installed_packages::{InstalledPackages, RegistryInstaller};
use crate::dataflow::local_package::LocalPackage;
use crate::dataflow::lockfile_packages::{LockfileError, LockfilePackages, LockfileResult};
use crate::dataflow::manifest_packages::{ManifestPackages, ManifestResult};
use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
use crate::dataflow::resolved_packages::{RegistryResolver, ResolvedPackages};
use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
use std::borrow::Cow;
use std::collections::hash_set::HashSet;
use std::fmt;
use std::path::Path;

pub mod added_packages;
pub mod changed_manifest_packages;
pub mod find_command_result;
pub mod installed_packages;
pub mod local_package;
pub mod lockfile_packages;
pub mod manifest_packages;
pub mod merged_lockfile_packages;
pub mod resolved_packages;
pub mod retained_lockfile_packages;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not open manifest. {}", _0)]
    ManifestError(manifest_packages::Error),
    #[fail(display = "Could not open lockfile. {}", _0)]
    LockfileError(LockfileError),
    #[fail(display = "Could generate lockfile. {}", _0)]
    GenerateLockfileError(merged_lockfile_packages::Error),
    #[fail(display = "Could not install package(s). {}", _0)]
    InstallError(installed_packages::Error),
    #[fail(display = "Could not resolve package(s). {}", _0)]
    ResolveError(resolved_packages::Error),
    #[fail(display = "Could not save manifest file because {}.", _0)]
    SaveError(String),
}

/// A package key for a package in the wapm.io registry.
/// This Is currently defined as name and a version.
#[derive(Clone, Debug, Eq, Hash, PartialOrd, PartialEq)]
pub struct WapmPackageKey<'a> {
    pub name: Cow<'a, str>,
    pub version: Cow<'a, str>,
}

impl<'a> fmt::Display for WapmPackageKey<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{} {}", self.name, self.version)
    }
}

/// A package key can be anything reference to a package, be it a wapm.io registry, a local directory, or a git url.
/// Currently, only wapm.io keys are supported.
#[allow(dead_code)]
#[derive(Clone, Debug, Eq, Hash, PartialOrd, PartialEq)]
pub enum PackageKey<'a> {
    GitUrl { url: &'a str },
    WapmPackage(WapmPackageKey<'a>),
}

impl<'a> PackageKey<'a> {
    /// Convenience constructor for wapm.io registry keys.
    fn new_registry_package<S>(name: S, version: S) -> Self
    where
        S: Into<Cow<'a, str>>,
    {
        PackageKey::WapmPackage(WapmPackageKey {
            name: name.into(),
            version: version.into(),
        })
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

/// If there is no mainfest, then this is a non-manifest project. All installations are retained
/// in the lockfile, and installs are additive.
pub fn update_with_no_manifest<P: AsRef<Path>>(
    directory: P,
    added_packages: AddedPackages,
) -> Result<(), Error> {
    let directory = directory.as_ref();
    // get lockfile data
    let lockfile_result = LockfileResult::find_in_directory(&directory);
    let lockfile_packages =
        LockfilePackages::new_from_result(lockfile_result).map_err(|e| Error::LockfileError(e))?;

    // check that the added packages are not already installed
    let initial_package_keys: HashSet<_> = lockfile_packages.package_keys();
    let lockfile_package_keys = lockfile_packages.package_keys();
    let added_packages = added_packages.prune_already_installed_packages(lockfile_package_keys);
    // check for missing packages e.g. deleting stuff from wapm_packages
    // install any missing or newly added packages
    let missing_packages = lockfile_packages.find_missing_packages(&directory);
    let added_packages = added_packages.add_missing_packages(missing_packages);

    let resolved_packages =
        ResolvedPackages::new_from_added_packages::<RegistryResolver>(added_packages)
            .map_err(|e| Error::ResolveError(e))?;
    let installed_packages =
        InstalledPackages::install::<RegistryInstaller, _>(&directory, resolved_packages)
            .map_err(|e| Error::InstallError(e))?;
    let added_lockfile_data = LockfilePackages::from_installed_packages(&installed_packages);

    let retained_lockfile_packages =
        RetainedLockfilePackages::from_lockfile_packages(lockfile_packages);

    // merge the lockfile data, and generate the new lockfile
    let final_lockfile_data =
        MergedLockfilePackages::merge(added_lockfile_data, retained_lockfile_packages);
    let final_package_keys: HashSet<_> = final_lockfile_data.packages.keys().cloned().collect();
    if final_package_keys != initial_package_keys {
        final_lockfile_data
            .generate_lockfile(&directory)
            .map_err(|e| Error::GenerateLockfileError(e))?;
    }

    Ok(())
}

/// If there is a manifest, then we construct lockfile data from manifest dependencies, and merge
/// with existing lockfile data.
pub fn update_with_manifest<P: AsRef<Path>>(
    directory: P,
    manifest: Manifest,
    added_packages: AddedPackages,
) -> Result<(), Error> {
    let directory = directory.as_ref();
    let manifest_data =
        ManifestPackages::new_from_manifest_and_added_packages(&manifest, added_packages)
            .map_err(|e| Error::ManifestError(e))?;

    // get lockfile data
    let lockfile_result = LockfileResult::find_in_directory(&directory);
    let lockfile_data =
        LockfilePackages::new_from_result(lockfile_result).map_err(|e| Error::LockfileError(e))?;

    // get the local package modules and commands from the manifest
    let local_package = LocalPackage::new_from_local_package_in_manifest(&manifest);

    let changed_manifest_data =
        ChangedManifestPackages::prune_unchanged_dependencies(&manifest_data, &lockfile_data);

    let added_packages = AddedPackages {
        packages: changed_manifest_data.packages,
    };

    let missing_lockfile_packages = lockfile_data.find_missing_packages(&directory);
    let added_packages = added_packages.add_missing_packages(missing_lockfile_packages);

    let retained_lockfile_packages =
        RetainedLockfilePackages::from_manifest_and_lockfile(&manifest_data, lockfile_data);

    let resolved_manifest_packages =
        ResolvedPackages::new_from_added_packages::<RegistryResolver>(added_packages)
            .map_err(|e| Error::ResolveError(e))?;
    let installed_manifest_packages =
        InstalledPackages::install::<RegistryInstaller, _>(&directory, resolved_manifest_packages)
            .map_err(|e| Error::InstallError(e))?;
    let mut manifest_lockfile_data =
        LockfilePackages::from_installed_packages(&installed_manifest_packages);

    manifest_lockfile_data.extend(local_package.into());

    // merge the lockfile data, and generate the new lockfile
    let final_lockfile_data =
        MergedLockfilePackages::merge(manifest_lockfile_data, retained_lockfile_packages);
    final_lockfile_data
        .generate_lockfile(&directory)
        .map_err(|e| Error::GenerateLockfileError(e))?;

    // update the manifest, if applicable
    update_manifest(manifest.clone(), &installed_manifest_packages)?;
    Ok(())
}

/// The function that starts lockfile dataflow. This function finds a manifest and a lockfile,
/// calculates differences, installs missing dependencies, and finally generates a new lockfile.
pub fn update<P: AsRef<Path>>(
    added_packages: Vec<(&str, &str)>,
    directory: P,
) -> Result<(), Error> {
    let directory = directory.as_ref();
    let added_packages = AddedPackages::new_from_str_pairs(added_packages);
    let manifest_result = ManifestResult::find_in_directory(&directory);
    match manifest_result {
        ManifestResult::NoManifest => update_with_no_manifest(directory, added_packages),
        ManifestResult::Manifest(manifest) => {
            update_with_manifest(directory, manifest, added_packages)
        }
        ManifestResult::ManifestError(e) => return Err(Error::ManifestError(e)),
    }
}

pub fn update_manifest(
    manifest: Manifest,
    installed_packages: &InstalledPackages,
) -> Result<(), Error> {
    let mut manifest = manifest;
    for (key, _, _) in installed_packages.packages.iter() {
        manifest.add_dependency(key.name.as_ref(), key.version.as_ref());
    }
    manifest.save().map_err(|e| Error::SaveError(e.to_string()))
}
