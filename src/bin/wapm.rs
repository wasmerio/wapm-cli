use std::{env, path};
use structopt::{clap::AppSettings, StructOpt};
#[cfg(feature = "update-notifications")]
use wapm_cli::update_notifier;
use wapm_cli::{commands, logging};

#[derive(StructOpt, Debug)]
#[structopt(global_settings = &[AppSettings::VersionlessSubcommands, AppSettings::ColorAuto, AppSettings::ColoredHelp])]
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
        settings = &[AppSettings::TrailingVarArg, AppSettings::AllowLeadingHyphen],
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

    #[cfg(feature = "update-notifications")]
    #[structopt(name = "run-background-update-check")]
    /// Run the background updater explicitly
    BackgroundUpdateCheck,

    #[structopt(name = "add")]
    /// Add packages to the manifest without installing
    Add(commands::AddOpt),

    #[structopt(name = "remove")]
    /// Remove packages from the manifest
    Remove(commands::RemoveOpt),

    /// Execute a command, installing it temporarily if necessary
    Execute(commands::ExecuteOpt),
}

fn main() {
    let is_atty = atty::is(atty::Stream::Stdout);
    if let Err(e) = logging::set_up_logging(is_atty) {
        eprintln!("Error: {}", e);
    }

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

    let prog_name = path::PathBuf::from(
        env::args()
            .next()
            .expect("Fatal error could not find any arguments!"),
    );
    let maybe_subcommand_name = env::args().skip(1).next();
    let prog_name = prog_name
        .file_name()
        .expect("Could not parse argv[0] as a path")
        .to_string_lossy();

    let args = if prog_name == "wax" {
        Command::Execute(commands::ExecuteOpt::ExecArgs(
            env::args().skip(1).collect(),
        ))
    } else if maybe_subcommand_name == Some("execute".to_string()) {
        Command::Execute(commands::ExecuteOpt::ExecArgs(
            env::args().skip(2).collect(),
        ))
    } else {
        Command::from_args()
    };

    #[cfg(feature = "update-notifications")]
    // Only show the async check on certain commands
    let maybe_show_update_notification = match args {
        Command::Install(_)
        | Command::Add(_)
        | Command::Run(_)
        | Command::Execute(_)
        | Command::Publish(_)
        | Command::Search(_)
        | Command::List(_)
        | Command::Uninstall(_) => {
            update_notifier::run_async_check_base();
            true
        }
        _ => false,
    };

    let result = match args {
        Command::WhoAmI => commands::whoami(),
        Command::Login => commands::login(),
        Command::Logout => commands::logout(),
        Command::Config(config_options) => commands::config(config_options),
        Command::Install(install_options) => commands::install(install_options),
        Command::Add(add_options) => commands::add(add_options),
        Command::Remove(remove_options) => commands::remove(remove_options),
        Command::Publish(publish_options) => commands::publish(publish_options),
        Command::Run(run_options) => commands::run(run_options),
        Command::Execute(execute_options) => commands::execute(execute_options),
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
        #[cfg(feature = "update-notifications")]
        Command::BackgroundUpdateCheck => {
            update_notifier::run_subprocess_check();
            Ok(())
        }
    };

    // Exit the program, flushing stdout, stderr
    // and show pending notifications (if any)
    {
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        std::io::stderr().flush().unwrap();
    }

    if let Err(e) = &result {
        eprintln!("Error: {}", e);
    }

    #[cfg(feature = "update-notifications")]
    {
        if maybe_show_update_notification {
            update_notifier::check_sync();
        }
    }

    if result.is_err() {
        #[cfg(feature = "telemetry")]
        {
            drop(_guard);
        };
        std::process::exit(-1);
    }
}
