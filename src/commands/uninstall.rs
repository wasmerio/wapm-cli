use crate::config::Config;
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
    let result = dataflow::update(vec![], uninstalled_package_names, dir)?;

    match result {
        Ok(()) => { info!("Package \"{}\" uninstalled.", options.package); },
        Err(e) => { info!("Failed to uninstall package \"{}\": {e}", options.package); },
    }

    Ok(())
}
