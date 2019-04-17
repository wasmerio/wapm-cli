pub mod lockfile;
pub mod lockfile_command;
pub mod lockfile_module;

pub static LOCKFILE_NAME: &str = "wapm.lock";

static LOCKFILE_HEADER: &str = r#"# Lockfile v1
# This file is automatically generated by Wapm.
# It is not intended for manual editing. The schema of this file may change."#;

use crate::dependency_resolver::PackageRegistry;
pub use crate::lock::lockfile::Lockfile;
pub use crate::lock::lockfile_command::LockfileCommand;
pub use crate::lock::lockfile_module::LockfileModule;
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
use std::env;
use std::path::Path;

#[inline]
pub fn get_package_namespace_and_name(package_name: &str) -> Result<(&str, &str), failure::Error> {
    let split: Vec<&str> = package_name.split('/').collect();
    match &split[..] {
        [namespace, name] => Ok((*namespace, *name)),
        [global_package_name] => Ok(("_", *global_package_name)),
        _ => bail!("Package name is invalid"),
    }
}

pub fn is_lockfile_out_of_date<P: AsRef<Path>>(directory: P) -> Result<bool, failure::Error> {
    use std::fs;
    let wapm_lock_metadata = fs::metadata(directory.as_ref().join(LOCKFILE_NAME))?;
    let wapm_toml_metadata = fs::metadata(directory.as_ref().join(MANIFEST_FILE_NAME))?;
    let wapm_lock_last_modified = wapm_lock_metadata.modified()?;
    let wapm_toml_last_modified = wapm_toml_metadata.modified()?;
    Ok(wapm_lock_last_modified < wapm_toml_last_modified)
}

pub fn regenerate_lockfile(
    installed_dependencies: Vec<(&str, &str)>,
) -> Result<(), failure::Error> {
    use crate::lock::lockfile::LockfileError;
    use crate::manifest::ManifestError;
    let current_dir = env::current_dir()?;
    let manifest_result = Manifest::find_in_directory(&current_dir);
    let maybe_manifest: Result<Option<Manifest>, failure::Error> = match manifest_result {
        Err(ManifestError::MissingManifest) => Ok(None),
        Ok(manifest) => Ok(Some(manifest)),
        Err(e) => Err(e.into()),
    };
    let maybe_manifest = maybe_manifest?;
    let mut lockfile_string = String::new();
    let lockfile_result = Lockfile::open(&current_dir, &mut lockfile_string);
    let maybe_lockfile: Result<Option<Lockfile>, failure::Error> = match lockfile_result {
        Err(LockfileError::MissingLockfile) => Ok(None),
        Ok(lockfile) => Ok(Some(lockfile)),
        Err(e) => Err(e.into()),
    };
    let maybe_lockfile = maybe_lockfile?;

    let mut resolver = PackageRegistry::new();
    match (maybe_manifest, maybe_lockfile) {
        (Some(mut manifest), Some(existing_lockfile)) => {
            for (dependency_name, dependency_version) in installed_dependencies {
                manifest.add_dependency(dependency_name, dependency_version);
            }
            // construct lockfile
            let lockfile = Lockfile::new_from_manifest_and_lockfile(
                &manifest,
                existing_lockfile,
                &mut resolver,
            )?;
            // write the manifest
            manifest.save()?;
            // write the lockfile
            lockfile.save(&manifest.base_directory_path)?;
        }
        (Some(mut manifest), None) => {
            for (dependency_name, dependency_version) in installed_dependencies {
                manifest.add_dependency(dependency_name, dependency_version);
            }
            // construct lockfile
            let lockfile = Lockfile::new_from_manifest(&manifest, &mut resolver)?;
            // write the manifest
            manifest.save()?;
            // write the lockfile
            lockfile.save(&manifest.base_directory_path)?;
        }
        (None, Some(existing_lockfile)) => {
            let lockfile = Lockfile::new_from_lockfile_and_installed_dependencies(
                installed_dependencies,
                existing_lockfile,
                &mut resolver,
            )?;
            let cwd = env::current_dir()?;
            lockfile.save(&cwd)?;
        }
        (None, None) => {
            let lockfile =
                Lockfile::new_from_installed_dependencies(installed_dependencies, &mut resolver)?;
            let cwd = env::current_dir()?;
            lockfile.save(&cwd)?;
        }
    }

    Ok(())
}
