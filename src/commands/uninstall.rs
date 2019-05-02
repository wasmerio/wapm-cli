use crate::config::Config;
use crate::dataflow;
use std::env;
use structopt::StructOpt;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Packages may only be uninstalled by the package name.")]
    NoAtSignAllowed,
}

#[derive(StructOpt, Debug)]
pub struct UninstallOpt {
    pub package: String,
    /// Uninstall the package(s) globally
    #[structopt(short = "g", long = "global")]
    pub global: bool,
}

pub fn uninstall(options: UninstallOpt) -> Result<(), failure::Error> {
    let dir = match options.global {
        true => Config::get_globals_directory()?,
        false => env::current_dir()?,
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

    if !result {
        info!("Package \"{}\" is not installed.", options.package);
    } else {
        info!("Package \"{}\" is uninstalled.", options.package);
    }

    Ok(())
}
