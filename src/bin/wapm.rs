use structopt::{clap::AppSettings, StructOpt};
use wapm_cli::commands;

#[derive(StructOpt, Debug)]
enum Command {
    #[structopt(name = "whoami")]
    /// Prints the current user (if authed) in the stdout
    WhoAmI,

    #[structopt(name = "login")]
    /// Logins into wapm, saving the token locally for future commands
    Login,

    #[structopt(name = "logout")]
    /// Remove the token for the registry
    Logout,

    #[structopt(name = "config")]
    /// Config related subcommands
    Config(commands::ConfigOpt),

    #[structopt(name = "install")]
    /// Install a package
    Install(commands::InstallOpt),

    #[structopt(name = "publish")]
    /// Publish a package
    Publish,

    #[structopt(
        name = "run",
        raw(settings = "&[AppSettings::TrailingVarArg, AppSettings::AllowLeadingHyphen]")
    )]
    /// Run a command from the package or one of the dependencies
    Run(commands::RunOpt),

    #[structopt(name = "search")]
    /// Search packages
    Search(commands::SearchOpt),

    #[cfg(feature = "package")]
    #[structopt(name = "package", raw(aliases = r#"&["p", "pkg"]"#))]
    /// Create a wasm package with bundled assets
    Package(commands::PackageOpt),

    #[structopt(name = "validate")]
    Validate(commands::ValidateOpt),

    /// Generate autocompletion scripts for your shell
    #[structopt(name = "completions")]
    Completions(commands::CompletionOpts),
}

fn main() {
    // dotenv::dotenv().ok();
    // env_logger::init();
    // let config: Env = envy::from_env()?;

    let args = Command::from_args();
    let result = match args {
        Command::WhoAmI => commands::whoami(),
        Command::Login => commands::login(),
        Command::Logout => commands::logout(),
        Command::Config(config_options) => commands::config(config_options),
        Command::Install(install_options) => commands::install(install_options),
        Command::Publish => commands::publish(),
        Command::Run(run_options) => commands::run(run_options),
        Command::Search(search_options) => commands::search(search_options),
        #[cfg(feature = "package")]
        Command::Package(package_options) => commands::package(package_options),
        Command::Validate(validate_options) => commands::validate(validate_options),
        Command::Completions(completion_options) => {
            Command::clap().gen_completions_to(
                "wapm",
                completion_options.shell,
                &mut ::std::io::stdout(),
            );
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("\nError: {}\n", e);
        std::process::exit(-1);
    }
}
