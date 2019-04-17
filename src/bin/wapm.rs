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

    /// Check if a directory or tar.gz is a valid wapm package
    #[structopt(name = "validate")]
    Validate(commands::ValidateOpt),

    #[structopt(name = "completions")]
    /// Generate autocompletion scripts for your shell
    Completions(commands::CompletionOpt),

    #[structopt(name = "init")]
    /// Set up current directory for use with wapm
    Init(commands::InitOpt),
}

fn main() {
    // dotenv::dotenv().ok();
    // env_logger::init();
    // let config: Env = envy::from_env()?;

    #[cfg(feature = "telemetry")]
    let _guard = {
        let telemetry_is_enabled = wapm_cli::util::telemetry_is_enabled();
        if telemetry_is_enabled {
            let _guard = sentry::init("https://aea870c3a5e54439999d8fed773bd8a5@sentry.io/1441509");
            sentry::integrations::panic::register_panic_handler();
            Some(_guard)
        } else {
            None
        }
    };

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
        Command::Init(init_options) => commands::init(init_options),
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
        #[cfg(feature = "telemetry")]
        {
            // check if telemetry is enabled
            if _guard.is_some() {
                sentry::capture_message(&format!("Error: {}", e), sentry::Level::Error);
                // manually flush guard because we exit the process below
                drop(_guard);
            }
        }

        eprintln!("\nError: {}\n", e);
        std::process::exit(-1);
    }
}
