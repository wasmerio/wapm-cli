use crate::init;
use crate::util;
use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InitOpt {
    /// The name of the wapm package
    package_name: String,
}

pub fn init(opt: InitOpt) -> Result<(), failure::Error> {
    let current_directory = env::current_dir()?;
    util::validate_package_name(&opt.package_name)?;
    init::init(current_directory, opt.package_name)
}
