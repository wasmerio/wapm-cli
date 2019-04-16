use structopt::{clap::AppSettings, clap::Shell, StructOpt};

#[derive(StructOpt, Debug)]
#[structopt(raw(setting = "AppSettings::Hidden"))]
pub struct CompletionOpt {
    /// The shell to generate the completions script for
    #[structopt(name = "SHELL", hidden = true, parse(try_from_str))]
    pub shell: Shell,
}
