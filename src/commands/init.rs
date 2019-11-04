use crate::init;
use std::env;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InitOpt {
    /// Agree to all prompts. Useful for non-interactive uses
    #[structopt(long = "force-yes", short = "y")]
    force_yes: bool,
}

pub fn init(opt: InitOpt) -> Result<(), failure::Error> {
    let current_directory = env::current_dir()?;
    init::init(current_directory, opt.force_yes)
}
