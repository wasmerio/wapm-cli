use crate::config::Config;
use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::dataflow::bin_script::BIN_DIR_NAME;
use std::env;
use structopt::StructOpt;
use thiserror::Error;

#[derive(StructOpt, Debug)]
pub struct BinOpt {
    /// Get the global .bin dir
    #[structopt(short = "g", long = "global")]
    pub global: bool,
}

#[derive(Clone, Debug, Error)]
pub enum BinError {
    #[error("The directory \"{0}\" does not contain wapm packages.")]
    NotWapmProjectDir(String),
}

pub fn bin(options: BinOpt) -> anyhow::Result<()> {
    let mut root_dir = match options.global {
        true => Config::get_globals_directory()?,
        false => env::current_dir()?,
    };
    root_dir.push(PACKAGES_DIR_NAME);

    // for wapm bin -g, display the global path even if it does not exist
    // otherwise error if the bin directory does not exist in the local directory
    if !root_dir.exists() && !options.global {
        return Err(BinError::NotWapmProjectDir(root_dir.to_string_lossy().to_string()).into());
    }

    root_dir.push(BIN_DIR_NAME);
    let bin_dir = root_dir;
    println!("{}", bin_dir.display());
    Ok(())
}
