//! Module for wax, executes a module immediately

//use crate::constants::RFC3339_FORMAT_STRING_WITH_TIMEZONE;
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

use graphql_client::*;

use std::convert::From;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct ExecuteOpt {
    /// The command to run.
    command: Option<String>,

    /// Run unsandboxed emscripten modules too.
    #[structopt(long = "emscripten")]
    enable_emscripten: bool,

    /// Agree to all prompts. Useful for non-interactive uses. (WARNING: this may cause undesired behavior).
    #[structopt(long = "force-yes", short = "y")]
    force_yes: bool,

    /// Package will be verified by its signature. Off by default
    #[structopt(long = "verify", short = "v")]
    verify_signature: bool,

    /// Pre-open a directory for WASI.
    #[structopt(long = "dir", multiple = true, group = "wasi")]
    pre_opened_directories: Vec<String>,

    /// Prevent the current directory from being preopened by default.
    #[structopt(long = "no-default-preopen")]
    no_default_preopen: bool,

    #[structopt(long = "which")]
    /// The command to run.
    which: Option<String>,

    /// Arguments that the command will get.
    #[structopt(raw(multiple = "true"), parse(from_os_str))]
    args: Vec<OsString>,
}

#[derive(Debug, Fail)]
enum ExecuteError {
    #[fail(
        display = "Command `{}` not found in the registry or in the current directory",
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
    #[fail(display = "An error occured during installation: {}", _0)]
    InstallationError(String),
    #[fail(display = "Please specify a command to run!")]
    NoCommandGiven,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/wax_get_command.graphql",
    response_derives = "Debug"
)]
struct WaxGetCommandQuery;

pub fn execute(mut opt: ExecuteOpt) -> Result<(), failure::Error> {
    if !opt.no_default_preopen {
        opt.pre_opened_directories.push(".".into());
    }
    let opt = opt;
    trace!("Execute {:?}", &opt);
    let current_dir = env::current_dir()?;
    if let Some(which) = opt.which {
        let mut wax_index = wax_index::WaxIndex::open()?;
        let dir = if let Ok((package_name, version)) = wax_index.search_for_entry(which.clone()) {
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
                }
                .into());
            }
        };

        println!("{}", dir.to_string_lossy());
        return Ok(());
    }
    let command = if let Some(command) = opt.command {
        command.clone()
    } else {
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

    // if not found, query the server and check if we already have it installed
    let q = WaxGetCommandQuery::build_query(wax_get_command_query::Variables {
        command: command_name.to_string(),
    });
    let response: wax_get_command_query::ResponseData = execute_query(&q)?;
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
        let mut wax_index = wax_index::WaxIndex::open()?;
        let registry_version = semver::Version::from_str(&command.package_version.version)
            .map_err(|e| ExecuteError::ErrorInDataFromRegistry(e.to_string()))?;

        let install_from_remote;
        if let Ok((package_name, version)) = wax_index.search_for_entry(command_name.to_string()) {
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
                install_from_remote = true;
            } else {
                debug!(
                    "Command found in Wax index, executing version {} directly",
                    version
                );
                install_from_remote = false;
                wax_index.save()?;

                run(
                    command_name,
                    location,
                    &opt.pre_opened_directories,
                    &opt.args,
                )?;
            }
        } else {
            debug!("Entry not found in wax index");
            install_from_remote = true;
        }

        // perform the install
        if install_from_remote {
            trace!("Installing Wax package from registry");
            // do install
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
            let added_lockfile_data =
                LockfilePackages::from_installed_packages(&installed_packages)
                    .map_err(|e| ExecuteError::InstallationError(e.to_string()))?;

            let retained_lockfile_packages =
                RetainedLockfilePackages::from_lockfile_packages(lockfile_packages);
            let final_lockfile_data =
                MergedLockfilePackages::merge(added_lockfile_data, retained_lockfile_packages);
            final_lockfile_data
                .generate_lockfile(&install_loc)
                .map_err(|e| ExecuteError::InstallationError(e.to_string()))?;

            debug!("Wax package installed to {}", install_loc.to_string_lossy());

            // get all the commands from the lockfile and index them
            let mut lockfile_not_found = true;
            match LockfileResult::find_in_directory(&install_loc) {
                LockfileResult::Lockfile(l) => {
                    for (command_name, command_info) in l.commands.iter() {
                        wax_index.insert_entry(
                            command_name.to_string(),
                            command_info.package_version.clone(),
                            command.package_version.package.name.clone(),
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
                        sentry::capture_message(
                            &format!(
                            "Lockfile not found in `{}` just after install! This should not happen",
                            &install_loc.to_string_lossy()
                        ),
                            sentry::Level::Error,
                        );
                    }
                }
                LockfileResult::LockfileError(e) => {
                    error!("Error in lockfile at `{}`! This is likely an internal Wapm error! Error details: {}", &install_loc.to_string_lossy(), e);
                    #[cfg(feature = "telemetry")]
                    {
                        sentry::capture_message(
                            &format!(
                                "Error in lockfile at `{}`. Error details: {}",
                                &install_loc.to_string_lossy(),
                                e
                            ),
                            sentry::Level::Error,
                        );
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
                    command.package_version.package.name.clone(),
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
