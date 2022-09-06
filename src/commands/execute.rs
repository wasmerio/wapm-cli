//! Module for wax, executes a module immediately

//use crate::constants::RFC3339_FORMAT_STRING_WITH_TIMEZONE;
use crate::config;
use crate::data::wax_index;
use crate::dataflow::find_command_result::FindCommandResult;
use crate::dataflow::installed_packages::{InstalledPackages, RegistryInstaller};
use crate::dataflow::lockfile_packages::{LockfilePackages, LockfileResult};
use crate::dataflow::merged_lockfile_packages::MergedLockfilePackages;
use crate::dataflow::resolved_packages::ResolvedPackages;
use crate::dataflow::retained_lockfile_packages::RetainedLockfilePackages;
use crate::dataflow::WapmPackageKey;
use crate::graphql::{execute_query, DateTime};
//use crate::keys;
use crate::util;
use thiserror::Error;

use graphql_client::*;

use std::convert::From;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::{clap::AppSettings, StructOpt};

#[derive(StructOpt, Debug)]
pub enum ExecuteOpt {
    /// The inner execute command
    #[structopt(external_subcommand)]
    ExecArgs(Vec<String>),
}

impl ExecuteOpt {
    fn args(&self) -> &[String] {
        match self {
            ExecuteOpt::ExecArgs(args) => args.as_slice(),
        }
    }
}

// NOTE: we only derive structopt for the help text, we don't actually
// use it to parse anything!!
#[derive(StructOpt, Debug, Clone, Default)]
#[structopt(settings = &[AppSettings::ColoredHelp])]
struct ExecuteOptInner {
    /// Run unsandboxed emscripten modules too.
    #[structopt(long = "emscripten")]
    enable_emscripten: bool,

    /// Agree to all prompts. Useful for non-interactive uses. (WARNING: this may cause undesired behavior).
    #[structopt(long = "force-yes", short = "y")]
    force_yes: bool,

    /// Package will be verified by its signature. Off by default
    #[structopt(long = "verify", short = "v")]
    verify_signature: bool,

    /// Run `wax` in offline mode.
    #[structopt(long = "offline")]
    offline: bool,

    /// Pre-open a directory for WASI.
    #[structopt(
        long = "dir",
        multiple = true,
        group = "wasi",
        value_name = "DIRECTORY"
    )]
    pre_opened_directories: Vec<String>,

    /// Prevent the current directory from being preopened by default.
    #[structopt(long = "no-default-preopen")]
    no_default_preopen: bool,

    /// The command to run.
    #[structopt(long = "which", value_name = "COMMAND")]
    which: Option<String>,

    /// The command to run.
    #[structopt(conflicts_with_all(&["which"]), index = 1, value_name = "COMMAND")]
    command: Option<String>,

    /// Arguments that the command will get.
    #[structopt(multiple = true, parse(from_os_str), last(true), value_name = "ARGS")]
    args: Vec<OsString>,
}

impl ExecuteOptInner {
    fn print_help_text() {
        // simple hack to force a help message and have StructOpt
        // manage our help text without doing our parsing
        ExecuteOptInner::clap()
            .bin_name("wax")
            .usage("wax [FLAGS] [OPTIONS] <COMMAND> [<ARGS>...]")
            .print_help()
            .expect("could not print help text");
        // pad with extra new line
        println!("");
    }
}

#[derive(Debug, Error)]
enum ExecuteError {
    #[error(
        "Command `{0}` not found in the registry {registry:?} or in the current directory",
        name
    )]
    CommandNotFound { name: String, registry: String },
    #[error(
        "The command `{0}` is using the Emscripten ABI which may be implmented in a way that is partially unsandbooxed. To opt-in to executing Emscripten Wasm modules run the command again with the `--emscripten` flag",
        name
    )]
    EmscriptenDisabled { name: String },
    #[error("Error with the Wax index: {0}")]
    WaxIndexError(wax_index::WaxIndexError),
    #[error("Error parsing data from register: {0}")]
    ErrorInDataFromRegistry(String),
    #[error("An error occured during installation: {0}")]
    InstallationError(String),
    #[error("Please specify a command to run.")]
    NoCommandGiven,
    #[error(
        "The command `{0}` was not found.\nIf you are offline you will need to reconnect to the Internet for this command to succeed.\nOtherwise there may be an error with the wapm registry that you're connected to: run `wapm config get registry.url` to print the URL that we tried to connect to.",
    )]
    CommandNotFoundOfflineMode(String),
    #[error(
        "The command `{0}` was not found.\nPlease try to run this command again without the `--offline` flag.",
    )]
    CommandNotFoundOfflineModeOfflineFlag(String),
}

#[derive(Debug, Error)]
enum ExecuteArgParsingError {
    #[error("Argument `{arg_name}` expects a value `{expected}` but none was found.")]
    MissingValue { arg_name: String, expected: String },
    #[error("Unrecognized argument `{arg_name}`")]
    UnrecognizedArgument { arg_name: String },
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/wax_get_command.graphql",
    response_derives = "Debug"
)]
struct WaxGetCommandQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/wax_get_package_command.graphql",
    response_derives = "Debug"
)]
struct WaxGetPackageCommandQuery;

#[derive(Debug)]
struct QueriedPackage {
    package: String,
    command_to_exec: String,
    version: semver::Version,
    download_url: String,
}

impl QueriedPackage {
    pub fn query(opt: &ExecuteOptInner, package_name: &str) -> Option<Self> {

        if opt.offline {
            return None;
        }

        let split = package_name.split("@").collect::<Vec<_>>();
        let (package_name, version) = match split.as_slice() {
            [n, v] => { (n.to_string(), Some(v.to_string())) },
            [n] => { (n.to_string(), None) },
            _ => { return None; },
        };

        let split2 = package_name.split(":").collect::<Vec<_>>();
        let (package_name, command_name) = match split2.as_slice() {
            [n, v] => { (n.to_string(), Some(v.to_string())) },
            [n] => { (n.to_string(), None) },
            _ => { return None; },
        };

        let q2 = WaxGetPackageCommandQuery::build_query(wax_get_package_command_query::Variables {
            name: package_name.to_string(),
        });

        let mut queried_package: Option<QueriedPackage> = None;

        match version {
            None => {
                let response: wax_get_package_command_query::ResponseData = execute_query(&q2).ok()?;
                let download_url = response.package.as_ref()?.last_version.as_ref()?.distribution.download_url.clone();
                let version = semver::Version::from_str(&response.package.as_ref()?.last_version.as_ref()?.version)
                .map_err(|e| ExecuteError::ErrorInDataFromRegistry(e.to_string())).ok()?;
                let manifest = wapm_toml::Manifest::from_str(&response.package.as_ref()?.last_version.as_ref()?.manifest).ok()?;
                let command_to_exec = match command_name {
                    Some(s) => s,
                    None => {
                        manifest.command
                        .unwrap_or_default()
                        .iter().map(|v| v.get_name())
                        .next()?
                    }
                };
                let name = response.package.as_ref()?.name.clone();
                queried_package = Some(QueriedPackage {
                    package: name,
                    command_to_exec,
                    version,
                    download_url,
                });
            },
            Some(v) => {

            }
        }

        queried_package
    }
}

/// Do the real argument parsing into [`ExecuteOptInner`].
fn transform_args(arg_stream: &[String]) -> Result<ExecuteOptInner, ExecuteArgParsingError> {
    let mut idx = 0;
    let mut out = ExecuteOptInner::default();
    let parse_which = |which_arg: Option<String>| -> Result<String, ExecuteArgParsingError> {
        let val: String = which_arg.ok_or_else(|| ExecuteArgParsingError::MissingValue {
            arg_name: "--which".to_string(),
            expected: "<COMMAND NAME>".to_string(),
        })?;
        Ok(val)
    };
    let parse_dir = |dir_arg: Option<String>| -> Result<String, ExecuteArgParsingError> {
        let val: String = dir_arg.ok_or_else(|| ExecuteArgParsingError::MissingValue {
            arg_name: "--dir".to_string(),
            expected: "<DIRECTORY>".to_string(),
        })?;
        Ok(val)
    };
    while idx < arg_stream.len() {
        match arg_stream[idx].as_ref() {
            "--emscripten" => out.enable_emscripten = true,
            "--force_yes" | "-y" => out.force_yes = true,
            "--verify" | "-v" => out.verify_signature = true,
            "--no-default-preopen" => out.no_default_preopen = true,
            "--offline" => out.offline = true,
            "--which" => {
                out.which = Some(parse_which(arg_stream.get(idx + 1).cloned())?);
                idx += 1;
            }
            "--dir" => {
                out.pre_opened_directories
                    .push(parse_dir(arg_stream.get(idx + 1).cloned())?);
                idx += 1;
            }
            "help" | "--help" | "-h" => {
                ExecuteOptInner::print_help_text();
                std::process::exit(0);
            }
            misc => {
                if misc.contains('=') {
                    // if it has a `=`, then it's an argument
                    let mut splitter = misc.split('=');
                    let arg = splitter.next().unwrap();
                    let val = splitter.next().map(|x| x.to_string());
                    match arg {
                        "--which" => out.which = Some(parse_which(val)?),
                        "--dir" => {
                            out.pre_opened_directories.push(parse_dir(val)?);
                        }
                        otherwise => {
                            return Err(ExecuteArgParsingError::UnrecognizedArgument {
                                arg_name: otherwise.to_string(),
                            })
                        }
                    }
                } else {
                    //otherwise, this is a command
                    out.command = Some(misc.to_string());
                    if let Some(after_command) = arg_stream.get(idx + 1) {
                        // eat `--` after command if it exists
                        if after_command == "--" {
                            idx += 1;
                        }
                    }
                    out.args = arg_stream[idx + 1..].iter().map(Into::into).collect();
                    break;
                }
            }
        }
        idx += 1;
    }

    Ok(out)
}

pub fn execute(opt: ExecuteOpt) -> anyhow::Result<()> {
    let mut opt = transform_args(opt.args())?;
    if !opt.no_default_preopen {
        opt.pre_opened_directories.push(".".into());
    }
    let opt = opt;
    trace!("Execute {:?}", &opt);
    let current_dir = crate::config::Config::get_current_dir()?;
    let config = crate::config::Config::from_file()?;
    if let Some(which) = opt.which {
        let mut wax_index = wax_index::WaxIndex::open()?;
        let dir = if let Ok((package_name, version, _)) = wax_index.search_for_entry(which.clone())
        {
            wax_index
                .base_path()
                .join(format!("{}@{}", package_name, version))
        } else {
            if let FindCommandResult::CommandFound { manifest_dir, .. } =
                FindCommandResult::find_command_in_directory(&current_dir, &which)
            {
                manifest_dir
            } else {
                return Err(ExecuteError::CommandNotFound {
                    name: which.clone(),
                    registry: config.registry.get_current_registry(),
                }
                .into());
            }
        };

        println!("{}", dir.to_string_lossy());
        return Ok(());
    }
    let command = if let Some(command) = &opt.command {
        command.clone()
    } else {
        ExecuteOptInner::print_help_text();
        return Err(ExecuteError::NoCommandGiven.into());
    };
    let command_name = command.as_str();
    let _value = util::set_wapm_should_accept_all_prompts(opt.force_yes);
    debug_assert!(
        _value.is_some(),
        "this function should only be called once!"
    );

    // first search for locally installed command
    match FindCommandResult::find_command_in_directory(&current_dir, &command_name) {
        FindCommandResult::CommandNotFound(_) => {
            // go to normal wax flow
            debug!(
                "Wax: Command \"{}\" not found locally in directory {}",
                &command_name,
                current_dir.to_string_lossy()
            );
        }
        FindCommandResult::CommandFound {
            source,
            manifest_dir,
            args: _,
            module_name,
            prehashed_cache_key,
        } => {
            debug!(
                "Wax command found locally in {}",
                current_dir.to_string_lossy()
            );
            // run it and return
            crate::commands::run::do_run(
                current_dir,
                source,
                manifest_dir,
                command_name,
                &module_name,
                &opt.pre_opened_directories,
                &opt.args,
                prehashed_cache_key,
            )?;
            return Ok(());
        }
        FindCommandResult::Error(e) => {
            warn!(
                "Error in Wax when looking for command locally: {}... Continuing execution",
                e
            );
        }
    }

    // get wax info from the index and find out if the entry is stale
    let mut wax_index = wax_index::WaxIndex::open()?;
    let (wax_info, exists_and_recently_updated): (Option<(String, semver::Version)>, bool) =
        if let Ok((package_name, version, last_seen)) =
            wax_index.search_for_entry(command_name.to_string())
        {
            let wax_cooldown = get_wax_cooldown();
            trace!("Using wax cooldown: {}", wax_cooldown);
            let now = time::now_utc().to_timespec();
            let cooldown_duration = time::Duration::seconds(wax_cooldown as i64);
            let time_to_update = last_seen + cooldown_duration;

            // if we haven't yet hit the time to update, then we've recently updated
            let recently_updated = now < time_to_update;
            debug!("The package was recently updated? {}", recently_updated);

            (Some((package_name, version)), recently_updated)
        } else {
            (None, false)
        };

    if exists_and_recently_updated {
        let (package_name, version) = wax_info
            .clone()
            .expect("critical internal logic error in `wapm execute`");
        let package_version_str = format!("{}@{}", &package_name, &version);
        let location = wax_index.base_path().join(&package_version_str);
        if !location
            .join("wapm_packages")
            .join(&package_version_str)
            .join("wapm.toml")
            .exists()
        {
            debug!("Package found in index, but cache has been cleared! continuing to logic that should reinstall");
        } else {
            // do execute unless it fails then continue
            wax_index.save()?;

            match run(
                command_name,
                location,
                &opt.pre_opened_directories,
                &opt.args,
            ) {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // REVIEW: does this even make sense?
                    // maybe if we force a reinstall but eh
                    debug!("Failed to run when cached due to `{}`. continuing...", e);
                }
            }
        }
    }

    // if not found, query the server and check if we already have it installed
    let q = WaxGetCommandQuery::build_query(wax_get_command_query::Variables {
        command: command_name.to_string(),
    });
    let response: wax_get_command_query::ResponseData = if !opt.offline {
        debug!("Querying server for package info");
        let response: Result<wax_get_command_query::ResponseData, _> = execute_query(&q);
        if response.is_err() {
            info!("Failed to connect to the wapm registry. Continuning in offline mode.");
            return do_offline_run(command_name, &opt);
        }
        response?
    } else {
        return do_offline_run(command_name, &opt);
    };

    trace!("Wax get command query: {:?}", response);
    if let Some(command) = response.command {
        // command found, check if it's installed
        if let Some(abi) = command.module.abi.as_ref() {
            if abi == "emscripten" && !opt.enable_emscripten {
                return Err(ExecuteError::EmscriptenDisabled {
                    name: command_name.to_string(),
                }
                .into());
            }
        }
        let registry_version = semver::Version::from_str(&command.package_version.version)
            .map_err(|e| ExecuteError::ErrorInDataFromRegistry(e.to_string()))?;

        if let Some((package_name, version)) = wax_info {
            let package_version_str = format!("{}@{}", &package_name, &version);
            let location = wax_index.base_path().join(&package_version_str);
            if registry_version > version
                || !location
                    .join("wapm_packages")
                    .join(&package_version_str)
                    .join("wapm.toml")
                    .exists()
            {
                debug!(
                    "Found version {} locally in Wax but version {} from the registry: upgrading",
                    version, registry_version
                );
            } else {
                debug!(
                    "Command found in Wax index, executing version {} directly",
                    version
                );
                wax_index.insert_entry(
                    command_name.to_string(),
                    registry_version.clone(),
                    command.package_version.package.name.clone(),
                );
                wax_index.save()?;

                run(
                    command_name,
                    location,
                    &opt.pre_opened_directories,
                    &opt.args,
                )?;
                return Ok(());
            }
        } else {
            debug!("Entry not found in wax index");
        }

        // ===================
        // Perform the install
        // if we made it this far, it means we haven't executed the command yet,
        // so we install the package and run it
        trace!("Installing Wax package from registry");
        let install_loc = wax_index.base_path().join(format!(
            "{}@{}",
            &command.package_version.package.name, &registry_version
        ));
        let resolved_packages = ResolvedPackages {
            packages: vec![(
                WapmPackageKey {
                    name: command.package_version.package.name.clone().into(),
                    version: registry_version.clone(),
                },
                (
                    command.package_version.distribution.download_url.clone(),
                    None, /*
                              // package signing disabled for `wapm execute` for now
                              command
                              .package_version
                              .signature
                              .map(|sig| keys::WapmPackageSignature {
                              public_key_id: sig.public_key.key_id.clone(),
                              public_key: sig.public_key.key.clone(),
                              signature_data: sig.data.clone(),
                              date_created: time::strptime(
                              &sig.public_key.uploaded_at,
                              RFC3339_FORMAT_STRING_WITH_TIMEZONE,
                          )
                              .unwrap_or_else(|err| {
                              panic!("Failed to parse time string: {}", err)
                          })
                              .to_timespec(),
                              revoked: sig.public_key.revoked,
                              owner: sig.public_key.owner.username.clone(),
                          })*/
                ),
            )],
        };

        // perform the install and generate the lockfile (like a simpler version of dataflow::update updating without a manifest)
        let lockfile_result = LockfileResult::find_in_directory(&install_loc);
        let lockfile_packages = LockfilePackages::new_from_result(lockfile_result)
            .map_err(|e| ExecuteError::InstallationError(e.to_string()))?;
        let installed_packages = InstalledPackages::install::<RegistryInstaller>(
            &install_loc,
            resolved_packages,
            !opt.verify_signature,
        )?;
        let added_lockfile_data = LockfilePackages::from_installed_packages(&installed_packages)
            .map_err(|e| ExecuteError::InstallationError(e.to_string()))?;

        let retained_lockfile_packages =
            RetainedLockfilePackages::from_lockfile_packages(lockfile_packages);
        let final_lockfile_data =
            MergedLockfilePackages::merge(added_lockfile_data, retained_lockfile_packages);
        final_lockfile_data
            .generate_lockfile(&install_loc)
            .map_err(|e| ExecuteError::InstallationError(e.to_string()))?;

        debug!("Wax package installed to {}", install_loc.to_string_lossy());

        wax_index.insert_entry(
            command_name.to_string(),
            registry_version,
            command.package_version.package.name.clone(),
        );
        wax_index.save()?;
        run(
            command_name,
            install_loc,
            &opt.pre_opened_directories,
            &opt.args,
        )?;
        return Ok(());
    } else if let Some(package) = QueriedPackage::query(&opt, &command_name) .as_ref() {
        println!("did not find command, but found package at url {:?}", package);
        Ok(())
    } else {
        return Err(ExecuteError::CommandNotFound {
            name: command_name.to_string(),
            registry: config.registry.get_current_registry(),
        }
        .into());
    }
}

fn run(
    command_name: &str,
    location: PathBuf,
    pre_opened_directories: &[String],
    args: &[OsString],
) -> anyhow::Result<()> {
    match FindCommandResult::find_command_in_directory(&location, command_name) {
        FindCommandResult::CommandNotFound(s) => {
            // this should only happen if the package is deleted immediately
            // after being installed to the temp directory or the package is
            // corrupt
            todo!(
                "Implement command not found logic!: {}, {}: {}",
                location.to_string_lossy(),
                &command_name,
                s
            );
        }
        FindCommandResult::CommandFound {
            source,
            manifest_dir,
            args: _,
            module_name,
            prehashed_cache_key,
        } => {
            crate::logging::clear_stdout()?;
            return crate::commands::run::do_run(
                location,
                source,
                manifest_dir,
                command_name,
                &module_name,
                pre_opened_directories,
                args,
                prehashed_cache_key,
            );
        }
        FindCommandResult::Error(e) => return Err(e),
    };
}

fn do_offline_run(command_name: &str, opt: &ExecuteOptInner) -> anyhow::Result<()> {
    let mut wax_index = wax_index::WaxIndex::open()?;
    if let Ok((package_name, version, _)) = wax_index.search_for_entry(command_name.to_string()) {
        let package_version_str = format!("{}@{}", &package_name, &version);
        let location = wax_index.base_path().join(&package_version_str);

        wax_index.save()?;

        crate::logging::clear_stdout()?;
        run(
            command_name,
            location,
            &opt.pre_opened_directories,
            &opt.args,
        )
    } else {
        if opt.offline {
            return Err(ExecuteError::CommandNotFoundOfflineModeOfflineFlag(
                command_name.to_string(),
            )
            .into());
        } else {
            return Err(ExecuteError::CommandNotFoundOfflineMode(command_name.to_string()).into());
        }
    }
}

impl From<wax_index::WaxIndexError> for ExecuteError {
    fn from(other: wax_index::WaxIndexError) -> Self {
        ExecuteError::WaxIndexError(other)
    }
}

fn get_wax_cooldown() -> i32 {
    config::Config::from_file()
        .ok()
        .map(|c| c.wax_cooldown)
        .unwrap_or(config::wax_default_cooldown())
}
