use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
use crate::dataflow::installed_manifest_packages::{InstalledManifestPackages, RegistryInstaller};
use crate::dataflow::lockfile_packages::{LockfileError, LockfilePackages, LockfileResult};
use crate::dataflow::manifest_packages::{ManifestPackages, ManifestResult};
use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
use crate::dataflow::resolved_manifest_packages::{RegistryResolver, ResolvedManifestPackages};
use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
use std::borrow::Cow;
use std::fmt;
use std::path::Path;

pub mod changed_manifest_packages;
pub mod installed_manifest_packages;
pub mod lockfile_packages;
pub mod manifest_packages;
pub mod merged_lockfile_packages;
pub mod resolved_manifest_packages;
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
    InstallError(installed_manifest_packages::Error),
    #[fail(display = "Could not resolve package(s). {}", _0)]
    ResolveError(resolved_manifest_packages::Error),
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
    LocalPackage { directory: &'a Path },
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

/// The function that starts lockfile dataflow. This function finds a manifest and a lockfile,
/// calculates differences, installs missing dependencies, and finally generates a new lockfile.
pub fn update<P: AsRef<Path>>(
    added_packages: Vec<(&str, &str)>,
    directory: P,
) -> Result<(), Error> {
    let directory = directory.as_ref();
    // find manifest in directory
    // return if manifest is invalid
    // ---
    // create a projection of the manifest
    // get manifest data
    let manifest_result = ManifestResult::find_in_directory(&directory);
    let mut manifest_data =
        ManifestPackages::new_from_result(&manifest_result).map_err(|e| Error::ManifestError(e))?;
    // add the extra packages
    manifest_data.add_additional_packages(added_packages.clone());
    let manifest_data = manifest_data;

    // get lockfile data
    let lockfile_result = LockfileResult::find_in_directory(&directory);
    let lockfile_data =
        LockfilePackages::new_from_result(lockfile_result).map_err(|e| Error::LockfileError(e))?;

    let changed_manifest_data =
        ChangedManifestPackages::prune_unchanged_dependencies(&manifest_data, &lockfile_data);

    let retained_lockfile_packages =
        RetainedLockfilePackages::from_manifest_and_lockfile(&manifest_data, lockfile_data);

    let resolved_manifest_packages =
        ResolvedManifestPackages::new::<RegistryResolver>(changed_manifest_data)
            .map_err(|e| Error::ResolveError(e))?;
    let installed_manifest_packages = InstalledManifestPackages::install::<RegistryInstaller, _>(
        &directory,
        resolved_manifest_packages,
    )
    .map_err(|e| Error::InstallError(e))?;
    let manifest_lockfile_data =
        LockfilePackages::from_installed_packages(&installed_manifest_packages);

    // merge the lockfile data, and generate the new lockfile
    let final_lockfile_data =
        MergedLockfilePackages::merge(manifest_lockfile_data, retained_lockfile_packages);
    final_lockfile_data
        .generate_lockfile(&directory)
        .map_err(|e| Error::GenerateLockfileError(e))?;

    // update the manifest, if applicable
    manifest_result
        .update_manifest(&installed_manifest_packages)
        .map_err(|e| Error::ManifestError(e))?;
    Ok(())
}
