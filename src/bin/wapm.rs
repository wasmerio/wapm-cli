#[cfg(feature = "update-notifications")]
use log::debug;
use structopt::{clap::AppSettings, StructOpt};
#[cfg(feature = "update-notifications")]
use wapm_cli::update_notifier;
use wapm_cli::{commands, logging};

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
    Publish(commands::PublishOpt),

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

    #[structopt(name = "list")]
    /// List the currently installed packages and their commands
    List(commands::ListOpt),

    #[cfg(feature = "packagesigning")]
    #[structopt(name = "keys")]
    /// Manage minisign keys for verifying packages
    Keys(commands::KeyOpt),

    #[structopt(name = "uninstall")]
    /// Uninstall a package
    Uninstall(commands::UninstallOpt),

    #[structopt(name = "bin")]
    /// Get the .bin dir path
    Bin(commands::BinOpt),
}

fn main() {
    if let Err(e) = logging::set_up_logging() {
        eprintln!("Error: {}", e);
    }

    #[cfg(feature = "update-notifications")]
    let maybe_receiver = update_notifier::run_async_check();

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
        Command::Publish(publish_options) => commands::publish(publish_options),
        Command::Run(run_options) => commands::run(run_options),
        Command::Search(search_options) => commands::search(search_options),
        #[cfg(feature = "package")]
        Command::Package(package_options) => commands::package(package_options),
        Command::Validate(validate_options) => commands::validate(validate_options),
        Command::Init(init_options) => commands::init(init_options),
        Command::List(list_options) => commands::list(list_options),
        #[cfg(feature = "packagesigning")]
        Command::Keys(key_options) => commands::keys(key_options),
        Command::Completions(completion_options) => {
            Command::clap().gen_completions_to(
                "wapm",
                completion_options.shell,
                &mut ::std::io::stdout(),
            );
            Ok(())
        }
        Command::Uninstall(uninstall_options) => commands::uninstall(uninstall_options),
        Command::Bin(bin_options) => commands::bin(bin_options),
    };

    // ensure async check finishes before exiting
    #[cfg(feature = "update-notifications")]
    {
        if let Some((recv, thread_handle)) = maybe_receiver {
            match recv.try_recv() {
                Ok(message) => {
                    update_notifier::print_message(&message);
                    if let Err(e) = thread_handle.join() {
                        debug!("Failed to join the async update message thread: {:?}", e);
                    }
                }
                Err(e) => debug!(
                    "Could not receive message from update notification thread: {}",
                    e.to_string()
                ),
            }
        }
    }

    if let Err(e) = result {
        #[cfg(feature = "telemetry")]
        {
            drop(_guard);
        };
        eprintln!("Error: {}", e);
        std::process::exit(-1);
    }
}
