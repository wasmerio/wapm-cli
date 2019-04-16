use structopt::{clap::Shell, StructOpt};

#[derive(StructOpt, Debug)]
pub struct CompletionOpts {
    /// The shell to generate the completions script for
    #[structopt(name = "SHELL", hidden = true, parse(try_from_str))]
    pub shell: Shell,
}
