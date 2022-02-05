use crate::init;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct InitOpt {
    /// Agree to all prompts. Useful for non-interactive uses
    #[structopt(long = "force-yes", short = "y")]
    force_yes: bool,
}

pub fn init(opt: InitOpt) -> anyhow::Result<()> {
    let current_directory = crate::config::Config::get_current_dir()?;
    init::init(current_directory, opt.force_yes)
}

#[cfg(feature = "integration_tests")]
impl InitOpt {
    pub fn new(force_yes: bool) -> Self {
        InitOpt { force_yes }
    }
}
