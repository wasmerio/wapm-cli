use crate::config::Config;
use crate::dataflow;
use std::env;
use structopt::StructOpt;

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

    // returned bool indicates if there was any change
    let result = dataflow::update(vec![], uninstalled_package_names, dir)?;

    if !result {
        info!("Package \"{}\" is not installed.", options.package);
    }
    else {
        info!("Package \"{}\" is uninstalled.", options.package);
    }

    Ok(())
}
