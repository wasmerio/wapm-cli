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
    pub package: String,
    /// Uninstall the package(s) globally
    #[structopt(short = "g", long = "global")]
    pub global: bool,
}

pub fn uninstall(options: UninstallOpt) -> anyhow::Result<()> {
    let dir = match options.global {
        true => Config::get_globals_directory()?,
        false => Config::get_current_dir()?,
    };
    let uninstalled_package_names = vec![options.package.as_str()];

    // do not allow the "@" symbol to prevent mis-use of this command
    if options.package.contains('@') {
        return Err(Error::NoAtSignAllowed.into());
    }

    // returned bool indicates if there was any to the lockfile. If this pacakge is uninstalled,
    // there will be a diff created, which causes update to return true. Because no other change
    // is made, we can assume any change resulted in successfully uninstalled package.
    let result = dataflow::update(vec![], uninstalled_package_names, dir.clone())?;

    // Uninstall the package from /tmp/wax/...
    let mut wax_uninstalled = false;
    let mut wax_index = wax_index::WaxIndex::open()?;
    if wax_index.search_for_entry(options.package.clone()).is_ok() {
        wax_index.remove_entry(options.package.as_str())?;
        wax_index.save()?;
        wax_uninstalled = true;
    }

    let mut pirita_uninstalled = false;
    let path = dir.join(PACKAGES_DIR_NAME).join(".bin").join(options.package.as_str());
    if path.exists() {
        std::fs::remove_file(&path)?;
        pirita_uninstalled = true;
    }

    if !result && !wax_uninstalled && !pirita_uninstalled {
        info!("Package \"{}\" is not installed.", options.package);
    } else {
        info!("Package \"{}\" uninstalled.", options.package);
    }

    Ok(())
}
