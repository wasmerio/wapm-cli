//! Module for wax, executes a module immediately

use crate::data::wax_index;
use crate::dataflow::find_command_result::FindCommandResult;
use crate::dataflow::installed_packages::{InstalledPackages, RegistryInstaller};
use crate::dataflow::lockfile_packages::LockfileResult;
use crate::dataflow::resolved_packages::ResolvedPackages;
use crate::dataflow::WapmPackageKey;
use crate::graphql::{execute_query, DateTime};
use crate::keys;
use crate::util;

use graphql_client::*;

use std::convert::From;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct ExecuteOpt {
    /// Command name
    command: String,
    /// Run unsandboxed emscripten modules too
    #[structopt(long = "emscripten")]
    run_emscripten_too: bool,
    /// Agree to all prompts. Useful for non-interactive uses. (WARNING: this may cause undesired behavior)
    #[structopt(long = "force-yes", short = "y")]
    force_yes: bool,
    /// WASI pre-opened directory
    #[structopt(long = "dir", multiple = true, group = "wasi")]
    pre_opened_directories: Vec<String>,
    /// Print info about Wax instead of executing a command
    #[structopt(long = "info")]
    just_print_info: bool,
    /// Application arguments
    #[structopt(raw(multiple = "true"), parse(from_os_str))]
    args: Vec<OsString>,
}

#[derive(Debug, Fail)]
enum ExecuteError {
    #[fail(
        display = "No package for command `{}` found locally or in the registry",
        name
    )]
    CommandNotFound { name: String },
    #[fail(
        display = "The command `{}` is using the Emscripten ABI which may be implmented in a way that is partially unsandbooxed. To opt-in to executing Emscripten Wasm modules run the command again with the `--emscripten` flag",
        name
    )]
    EmscriptenDisabled { name: String },
    #[fail(display = "Error with the Wax index: {}", _0)]
    WaxIndexError(wax_index::WaxIndexError),
    #[fail(display = "Error parsing data from register: {}", _0)]
    ErrorInDataFromRegistry(String),
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/wax_get_command.graphql",
    response_derives = "Debug"
)]
struct WaxGetCommandQuery;

pub fn execute(opt: ExecuteOpt) -> Result<(), failure::Error> {
    trace!("Execute {:?}", &opt);
    let command_name = opt.command.as_str();
    let current_dir = env::current_dir()?;
    let _value = util::set_wapm_should_accept_all_prompts(opt.force_yes);
    debug_assert!(
        _value.is_some(),
        "this function should only be called once!"
    );

    if opt.just_print_info {
        println!("Wax downloads Wasm modules if they're not available locally, stores them temporarily, and executes them.");
        match wax_index::WaxIndex::open() {
            Ok(wax_index) => {
                let wax_base_path = wax_index.base_path();
                println!(
                    "Wax modules are installed to `{}`",
                    wax_base_path.to_string_lossy()
                );
            }
            Err(e) => {
                println!("Failed to open the Wax Index. Ensure the `WASMER_DIR` env var is set and try again: {}", e);
            }
        }
        return Ok(());
    }

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

    // if not found, query the server and check if we already have it installed
    let q = WaxGetCommandQuery::build_query(wax_get_command_query::Variables {
        command: command_name.to_string(),
    });
    let response: wax_get_command_query::ResponseData = execute_query(&q)?;
    trace!("Wax get command query: {:?}", response);
    if let Some(command) = response.command {
        // command found, check if it's installed
        if let Some(abi) = command.module.abi.as_ref() {
            if abi == "emscripten" && !opt.run_emscripten_too {
                return Err(ExecuteError::EmscriptenDisabled {
                    name: command_name.to_string(),
                }
                .into());
            }
        }
        let mut wax_index = wax_index::WaxIndex::open()?;
        let registry_version = semver::Version::from_str(&command.package_version.version)
            .map_err(|e| ExecuteError::ErrorInDataFromRegistry(e.to_string()))?;

        let install_from_remote;
        if let Ok(wax_index::WaxIndexEntry { version, location }) =
            wax_index.search_for_entry(command_name.to_string())
        {
            if registry_version > version {
                debug!(
                    "Found version {} locally in Wax but version {} from the registry: upgrading",
                    version, registry_version
                );
                install_from_remote = true;
            } else {
                debug!(
                    "Command found in Wax index, executing version {} directly",
                    version
                );
                install_from_remote = false;
                wax_index.save()?;

                // this shouldn't be needed!
                //crate::dataflow::update(vec![], vec![], wax_index.base_path()).unwrap();
                run(
                    command_name,
                    location.to_owned(),
                    &opt.pre_opened_directories,
                    &opt.args,
                )?;
            }
        } else {
            debug!("Entry not found in wax index");
            install_from_remote = true;
        }

        if install_from_remote {
            trace!("Installing Wax package from registry");
            // do install
            let install_loc = wax_index.base_path().join(format!(
                "{}@{}",
                &command.package_version.package.name, &registry_version
            ));
            let resolve_packages = ResolvedPackages {
                packages: vec![(
                    WapmPackageKey {
                        name: command.package_version.package.name.clone().into(),
                        version: registry_version.clone(),
                    },
                    (
                        command.package_version.distribution.download_url.clone(),
                        None, /*
                               TODO: package signing support
                               command.package_version.signature.map(|sig| keys::WapmPackageSignature {
                                  public_key_id: sig.key_id.clone(),
                                  public_key: sig.key.clone(),
                                  signature_data: todo!("HMMM"),
                                  date_created: todo!("TODO PARSE IT")a,
                                  revoked: sig.revoked,
                                  owner: sig.owner.username.clone(),
                              })
                               */
                    ),
                )],
            };
            crate::dataflow::update(
                vec![(
                    &command.package_version.package.name,
                    &command.package_version.version,
                )],
                vec![],
                &install_loc,
            )
            .unwrap();
            debug!("Wax package installed to {}", install_loc.to_string_lossy());

            // get all the commands from the lockfile and index them
            let mut lockfile_not_found = true;
            match LockfileResult::find_in_directory(&install_loc) {
                LockfileResult::Lockfile(l) => {
                    for (command_name, command_info) in l.commands.iter() {
                        wax_index.insert_entry(
                            command_name.to_string(),
                            command_info.package_version.clone(),
                            install_loc.clone(),
                        );
                    }
                    lockfile_not_found = false;
                }
                LockfileResult::NoLockfile => {
                    error!(
                        "Lockfile not found in `{}`! This is likely an internal Wapm error!",
                        &install_loc.to_string_lossy()
                    );
                    #[cfg(feature = "telemetry")]
                    {
                        sentry::integrations::failure::capture_error(format!(
                            "Lockfile not found in `{}` just after install! This should not happen",
                            &install_loc.to_string_lossy()
                        ));
                    }
                }
                LockfileResult::LockfileError(e) => {
                    error!("Error in lockfile at `{}`! This is likely an internal Wapm error! Error details: {}", &install_loc.to_string_lossy(), e);
                    #[cfg(feature = "telemetry")]
                    {
                        sentry::integrations::failure::capture_error(format!(
                            "Error in lockfile at `{}`. Error details: {}",
                            &install_loc.to_string_lossy(),
                            e
                        ));
                    }
                }
            }

            // Just add the current package if we couldn't find the lockfile.
            // This is honestly an error but we'll play it safe and try to keep
            // going even if we run into issues because we've reported the error
            // if telemetry is enabled and printed an error to the user.
            if lockfile_not_found {
                wax_index.insert_entry(
                    command_name.to_string(),
                    registry_version,
                    install_loc.clone(),
                );
            }
            wax_index.save()?;
            run(
                command_name,
                install_loc,
                &opt.pre_opened_directories,
                &opt.args,
            )?;
        }
    } else {
        return Err(ExecuteError::CommandNotFound {
            name: command_name.to_string(),
        }
        .into());
    }

    Ok(())
}

fn run(
    command_name: &str,
    location: PathBuf,
    pre_opened_directories: &[String],
    args: &[OsString],
) -> Result<(), failure::Error> {
    match FindCommandResult::find_command_in_directory(&location, command_name) {
        FindCommandResult::CommandNotFound(s) => {
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

impl From<wax_index::WaxIndexError> for ExecuteError {
    fn from(other: wax_index::WaxIndexError) -> Self {
        ExecuteError::WaxIndexError(other)
    }
}
