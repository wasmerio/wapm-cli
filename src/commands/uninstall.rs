use crate::config::Config;
use crate::data::wax_index;
use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::dataflow;
use structopt::StructOpt;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Packages may only be uninstalled by the package name.")]
    NoAtSignAllowed,
}

#[derive(StructOpt, Debug)]
pub struct UninstallOpt {
    pub package: Option<String>,
    /// Uninstall the package(s) globally
    #[structopt(short = "g", long = "global")]
    pub global: bool,
    /// Uninstall all packages (useful for running in CI)
    #[structopt(short = "a", long = "all")]
    pub all: bool, 
}

pub fn uninstall(options: UninstallOpt) -> anyhow::Result<()> {
    let dir = match options.global {
        true => Config::get_globals_directory()?,
        false => Config::get_current_dir()?,
    };

    let package_names = match options.package.as_ref() {
        Some(s) => s.split_whitespace().map(|s| s.to_string()).collect::<Vec<_>>(),
        None => {
            if options.all {
                use crate::dataflow::lockfile_packages::{LockfilePackages, LockfileResult};

                let wax_index = wax_index::WaxIndex::open()?;
                let mut entries = wax_index.get_all_entries();

                // get local packages from lockfile
                let lockfile_result = LockfileResult::find_in_directory(&Config::get_current_dir()?);
                let lockfile_packages = LockfilePackages::new_from_result(lockfile_result)
                    .map(|k| k.package_keys())
                    .unwrap_or_default();
                for key in lockfile_packages.iter() {
                    println!("uninstalling lockfile package {}", key.get_name());
                    entries.push(key.get_name().to_string());
                }

                if options.global {
                    // get local packages from lockfile
                    let lockfile_result = LockfileResult::find_in_directory(&Config::get_globals_directory()?);
                    let lockfile_packages = LockfilePackages::new_from_result(lockfile_result)
                        .map(|k| k.package_keys())
                        .unwrap_or_default();
                    for key in lockfile_packages.iter() {
                        println!("uninstalling lockfile package {}", key.get_name());
                        entries.push(key.get_name().to_string());
                    }
                }

                if entries.is_empty() {
                    return Ok(());
                }
                entries
            } else {
                return Err(anyhow!("No packages specified to uninstall."));
            }
        }
    };

    for package in package_names.iter() {

        let uninstalled_package_names = vec![package.as_str()];

        // do not allow the "@" symbol to prevent mis-use of this command
        if package.contains('@') {
            return Err(Error::NoAtSignAllowed.into());
        }
    
        // returned bool indicates if there was any to the lockfile. If this pacakge is uninstalled,
        // there will be a diff created, which causes update to return true. Because no other change
        // is made, we can assume any change resulted in successfully uninstalled package.
        let result = dataflow::update(vec![], uninstalled_package_names, dir.clone())?;
    
        // Uninstall the package from /tmp/wax/...
        let mut wax_uninstalled = false;
        let mut wax_index = wax_index::WaxIndex::open()?;
        if wax_index.search_for_entry(package.clone()).is_ok() {
            wax_index.remove_entry(package.as_str())?;
            wax_index.save()?;
            wax_uninstalled = true;
        }
    
        let mut pirita_uninstalled = false;
        let path = dir.join(PACKAGES_DIR_NAME).join(".bin").join(package.as_str());
        if path.exists() {
            std::fs::remove_file(&path)?;
            pirita_uninstalled = true;
        }
    
        if !result && !wax_uninstalled && !pirita_uninstalled {
            info!("Package \"{}\" is not installed.", package);
        } else {
            info!("Package \"{}\" uninstalled.", package);
        }
    }

    Ok(())
}
