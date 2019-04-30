use crate::config::Config;
use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::dataflow::bin_script::BIN_DIR_NAME;
use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct BinOpt {
    /// Get the global .bin dir
    #[structopt(short = "g", long = "global")]
    pub global: bool,
}

pub fn bin(options: BinOpt) -> Result<(), failure::Error> {
    let mut root_dir = match options.global {
        true => Config::get_globals_directory()?,
        false => env::current_dir()?,
    };
    root_dir.push(PACKAGES_DIR_NAME);
    root_dir.push(BIN_DIR_NAME);
    let bin_dir = root_dir;
    println!("{}", bin_dir.display());
    Ok(())
}
