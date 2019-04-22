use crate::dataflow::changed_manifest_packages::ChangedManifestPackages;
use crate::dataflow::installed_manifest_packages::{InstalledManifestPackages, RegistryInstaller};
use crate::dataflow::lockfile_packages::{LockfilePackages, LockfileResult, LockfileSource};
use crate::dataflow::manifest_packages::{ManifestPackages, ManifestResult, ManifestSource};
use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
use crate::dataflow::resolved_manifest_packages::{RegistryResolver, ResolvedManifestPackages};
use std::borrow::Cow;
use std::path::Path;

pub mod changed_manifest_packages;
pub mod installed_manifest_packages;
pub mod lockfile_packages;
pub mod manifest_packages;
pub mod merged_lockfile_packages;
pub mod resolved_manifest_packages;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not parse manifest because {}.", _0)]
    ManifestTomlParseError(String),
    #[fail(display = "Could not parse lockfile because {}.", _0)]
    LockfileTomlParseError(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
    #[fail(display = "Could not install added packages. {}.", _0)]
    InstallError(String),
}

/// A package key for a package in the wapm.io registry.
/// This Is currently defined as name and a version.
#[derive(Clone, Debug, Eq, Hash, PartialOrd, PartialEq)]
pub struct WapmPackageKey<'a> {
    pub name: Cow<'a, str>,
    pub version: Cow<'a, str>,
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

/// The function that starts lockfile dataflow. This function finds a manifest and a lockfile,
/// calculates differences, installs missing dependencies, and finally generates a new lockfile.
pub fn update<P: AsRef<Path>>(
    added_packages: Vec<(&str, &str)>,
    directory: P,
) -> Result<(), Error> {
    let directory = directory.as_ref();
    // get manifest data
    let manifest_source = ManifestSource::new(&directory);
    let manifest_result = ManifestResult::from_source(&manifest_source);
    let mut manifest_data = ManifestPackages::new_from_result(&manifest_result)?;
    // add the extra packages
    manifest_data.add_additional_packages(added_packages.clone());

    // get lockfile data
    let lockfile_string = LockfileSource::new(&directory);
    let lockfile_result = LockfileResult::from_source(&lockfile_string);
    let lockfile_data = LockfilePackages::new_from_result(lockfile_result)?;

    let pruned_manifest_data =
        ChangedManifestPackages::prune_unchanged_dependencies(&manifest_data, &lockfile_data);
    let resolved_manifest_packages =
        ResolvedManifestPackages::new::<RegistryResolver>(pruned_manifest_data)?;
    let installed_manifest_packages = InstalledManifestPackages::install::<RegistryInstaller, _>(
        &directory,
        resolved_manifest_packages,
    )?;
    let manifest_lockfile_data =
        LockfilePackages::from_installed_packages(&installed_manifest_packages);

    // merge the lockfile data, and generate the new lockfile
    let final_lockfile_data = MergedLockfilePackages::merge(manifest_lockfile_data, lockfile_data);
    final_lockfile_data.generate_lockfile(&directory)?;

    // update the manifest, if applicable
    manifest_result.update_manifest(&installed_manifest_packages)?;
    Ok(())
}
