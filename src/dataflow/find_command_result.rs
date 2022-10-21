use crate::config::Config;
use crate::data::lock::lockfile::{Lockfile, LockfileError};
use crate::data::manifest::Manifest;
use crate::dataflow::lockfile_packages::LockfileResult;
use crate::dataflow::manifest_packages::ManifestResult;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::graphql::execute_query;
use graphql_client::*;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package_by_command.graphql",
    response_derives = "Debug"
)]
struct GetPackageByCommandQuery;

#[derive(Debug)]
pub struct PackageInfoFromCommand {
    pub command: String,
    pub version: String,
    pub namespaced_package_name: String,
}

impl PackageInfoFromCommand {
    fn get_response(
        command_name: String,
    ) -> anyhow::Result<get_package_by_command_query::ResponseData> {
        let q = GetPackageByCommandQuery::build_query(get_package_by_command_query::Variables {
            command_name,
        });
        execute_query(&q)
    }

    pub fn get(command_name: String) -> anyhow::Result<Self> {
        let response = Self::get_response(command_name)?;
        let response_val = response
            .get_command
            .ok_or_else(|| anyhow!("Error getting packages for given command from server"))?;
        Ok(Self {
            command: response_val.command,
            version: response_val.package_version.version,
            namespaced_package_name: response_val.package_version.package.display_name,
        })
    }
}

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error(
        "Command \"{command}\" was not found in the local directory or the global install directory."
    )]
    CommandNotFound {
        command: String,
        error: String,
        local_log: Vec<String>,
        global_log: Vec<String>,
    },
    #[error(
        "Command \"{command}\" was neither found in the local nor in the global directory. {error}"
    )]
    CommandNotFoundInLocalDirectoryAndErrorReadingGlobalDirectory {
        command: String,
        error: String,
        local_log: Vec<String>,
        global_log: Vec<String>,
    },
    #[error(
        "Could not get command \"{command}\" because there was a problem with the local package. {error}"
    )]
    ReadingLocalDirectory {
        command: String,
        error: String,
        local_log: Vec<String>,
        global_log: Vec<String>,
    },
    #[error(
        "Command \"{0}\" exists in lockfile, but corresponding module \"{1}\" not found in lockfile.",
    )]
    CommandFoundButCorrespondingModuleIsMissing(String, String),
    #[error(
        "Failed to get command \"{0}\" because there was an error opening the global installation directory. {1}",
    )]
    CouldNotOpenGlobalsDirectory(String, String),
}

#[derive(Debug)]
pub enum FindCommandResult {
    CommandNotFound {
        error: String,
        // Extended description of the error
        extended: Vec<String>,
    },
    CommandFound {
        source: PathBuf,
        manifest_dir: PathBuf,
        args: Option<String>,
        module_name: String,
        prehashed_cache_key: Option<String>,
    },
    Error(anyhow::Error),
}

impl From<LockfileError> for FindCommandResult {
    fn from(error: LockfileError) -> Self {
        match error {
            LockfileError::CommandNotFound(c) => FindCommandResult::CommandNotFound {
                error: c,
                extended: Vec::new(),
            },
            _ => FindCommandResult::Error(error.into()),
        }
    }
}

impl FindCommandResult {
    fn find_command_in_manifest_and_lockfile<S: AsRef<str>>(
        command_name: S,
        manifest: Manifest,
        lockfile: Lockfile,
        directory: &Path,
    ) -> Self {
        match lockfile.get_command(command_name.as_ref()) {
            Err(e) => e.into(),
            Ok(lockfile_command) => {
                debug!("Command found in lockfile: {:?}", &lockfile_command);
                if lockfile_command.package_name == manifest.package.name {
                    // this is a local module command
                    let found_module = manifest.module.as_ref().and_then(|modules| {
                        modules.iter().find(|m| m.name == lockfile_command.module)
                    });
                    match found_module {
                        Some(module) => FindCommandResult::CommandFound {
                            source: module.source.clone(),
                            manifest_dir: manifest.base_directory_path,
                            args: lockfile_command.main_args.clone(),
                            module_name: module.name.clone(),
                            // don't use prehashed cache key for local modules
                            prehashed_cache_key: None,
                        },
                        None => FindCommandResult::Error(
                            Error::CommandFoundButCorrespondingModuleIsMissing(
                                command_name.as_ref().to_string(),
                                lockfile_command.module.clone(),
                            )
                            .into(),
                        ),
                    }
                } else {
                    // this is a module being run as a dependency in a local context
                    debug!(
                        "Command's package name({}) and manifest's package name({}) are different",
                        lockfile_command.package_name, manifest.package.name
                    );
                    match lockfile.get_module(
                        &lockfile_command.package_name,
                        &lockfile_command.package_version,
                        &lockfile_command.module,
                    ) {
                        Ok(lockfile_module) => {
                            let path = lockfile_module
                                .get_canonical_source_path_from_lockfile_dir(directory.into());
                            let manifest_dir = lockfile_module
                                .get_canonical_manifest_path_from_lockfile_dir(
                                    directory.into(),
                                    true,
                                );
                            FindCommandResult::CommandFound {
                                source: path,
                                manifest_dir,
                                args: lockfile_command.main_args.clone(),
                                module_name: lockfile_module.name.clone(),
                                prehashed_cache_key: lockfile
                                    .get_prehashed_cache_key_from_command(lockfile_command),
                            }
                        }
                        Err(e) => FindCommandResult::Error(e),
                    }
                }
            }
        }
    }

    fn find_command_in_lockfile<S: AsRef<str>>(
        command_name: S,
        lockfile: Lockfile,
        directory: &Path,
    ) -> Self {
        let command_name = command_name.as_ref();
        let mut error_lines = Vec::new();

        // Look into the lockfile.commands to find the command by name first
        if let Ok(lockfile_command) = lockfile.get_command(command_name) {
            // If this fails, the package is corrupt
            match lockfile.get_module(
                &lockfile_command.package_name,
                &lockfile_command.package_version,
                &lockfile_command.module,
            ) {
                Ok(lockfile_module) => {
                    let path = lockfile_module
                        .get_canonical_source_path_from_lockfile_dir(directory.into());
                    let manifest_dir = lockfile_module
                        .get_canonical_manifest_path_from_lockfile_dir(directory.into(), true);
                    return FindCommandResult::CommandFound {
                        source: path,
                        manifest_dir,
                        args: lockfile_command.main_args.clone(),
                        module_name: lockfile_module.name.clone(),
                        prehashed_cache_key: lockfile
                            .get_prehashed_cache_key_from_command(lockfile_command),
                    };
                }
                Err(e) => {
                    return FindCommandResult::CommandNotFound {
                        error: command_name.to_string(),
                        extended: vec![format!("{e}")],
                    };
                }
            }
        }

        if let Some(s) = lockfile
            .modules
            .keys()
            .find(|k| k.as_str().contains(command_name))
        {
            error_lines.push(String::new());
            error_lines.push("Note:".to_string());
            error_lines.push(format!("    A package {s:?} seems to be installed locally"));
            error_lines.push(format!(
                "    but the package {s:?} has no commands to execute"
            ));

            let all_commands = lockfile.commands.keys().cloned().collect::<Vec<_>>();
            let nearest = all_commands
                .iter()
                .filter_map(|c| sublime_fuzzy::best_match(c, command_name).map(|_| c.clone()))
                .take(3)
                .collect::<Vec<_>>();

            if !nearest.is_empty() {
                error_lines.push(String::new());
                error_lines.push("Did you mean:".to_string());
                for n in &nearest {
                    error_lines.push(format!("    {n}"));
                }
                error_lines.push(String::new());
            }
        }

        FindCommandResult::CommandNotFound {
            error: command_name.to_string(),
            extended: error_lines,
        }
    }

    pub fn find_command_in_directory<S: AsRef<str>>(directory: &Path, command_name: S) -> Self {
        let manifest_result = ManifestResult::find_in_directory(directory);
        let lockfile_result = LockfileResult::find_in_directory(directory);
        match (manifest_result, lockfile_result) {
            (ManifestResult::ManifestError(e), _) => return FindCommandResult::Error(e.into()),
            (_, LockfileResult::LockfileError(e)) => return FindCommandResult::Error(e.into()),
            (ManifestResult::NoManifest, LockfileResult::NoLockfile) => {} // continue
            (ManifestResult::NoManifest, LockfileResult::Lockfile(l)) => {
                debug!("Looking for local command in the lockfile");
                return Self::find_command_in_lockfile(command_name, l, directory);
            }
            // the edge case of a manifest, but no lockfile would an invalid state. This function
            // should always be run after updating the lockfile with the latest manifest changes.
            // If that function were to fail so horribly that it did not error, and no lockfile was
            // generated, then we will get this panic.
            (ManifestResult::Manifest(_m), LockfileResult::NoLockfile) => {
                panic!("Manifest exists, but lockfile not found!")
            }
            (ManifestResult::Manifest(m), LockfileResult::Lockfile(l)) => {
                debug!("Looking for local command in the manifest and lockfile");
                return Self::find_command_in_manifest_and_lockfile(command_name, m, l, directory);
            }
        };
        FindCommandResult::CommandNotFound {
            error: command_name.as_ref().to_string(),
            extended: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct Command {
    // PathBuf, Option<String>, String, bool
    pub source: PathBuf,
    pub manifest_dir: PathBuf,
    pub args: Option<String>,
    pub module_name: String,
    /// whether the command was found in the global context
    pub is_global: bool,
    /// the prehashed module key
    pub prehashed_cache_key: Option<String>,
}

/// Get a command from anywhere, where anywhere is the set of packages in the local lockfile and the global lockfile.
/// A flag indicating global run is also returned. Commands are found in local lockfile first.
pub fn get_command_from_anywhere<S: AsRef<str>>(command_name: S) -> Result<Command, Error> {
    // look in the local directory, update if necessary
    let current_directory = crate::config::Config::get_current_dir().unwrap();
    let local_command_result =
        FindCommandResult::find_command_in_directory(&current_directory, &command_name);

    let mut log = Vec::new();

    match local_command_result {
        FindCommandResult::CommandNotFound { error: _, extended } => {
            log = extended;
            // not found, continue searching...
        }
        FindCommandResult::CommandFound {
            source,
            manifest_dir,
            args,
            module_name,
            prehashed_cache_key,
        } => {
            return Ok(Command {
                source,
                manifest_dir,
                args,
                module_name,
                is_global: false,
                prehashed_cache_key,
            });
        }
        FindCommandResult::Error(e) => {
            return Err(Error::ReadingLocalDirectory {
                command: command_name.as_ref().to_string(),
                error: e.to_string(),
                local_log: log,
                global_log: Vec::new(),
            });
        }
    };
    trace!("Local command not found");

    // look in the global directory
    let global_directory = Config::get_globals_directory().map_err(|e| {
        Error::CouldNotOpenGlobalsDirectory(command_name.as_ref().to_string(), e.to_string())
    })?;
    let global_command_result =
        FindCommandResult::find_command_in_directory(&global_directory, &command_name);

    let mut global_log = Vec::new();
    match global_command_result {
        FindCommandResult::CommandNotFound { error: _, extended } => {
            global_log = extended;
            // continue searching...
        }
        FindCommandResult::CommandFound {
            source,
            manifest_dir,
            args,
            module_name,
            prehashed_cache_key,
        } => {
            return Ok(Command {
                source,
                manifest_dir,
                args,
                module_name,
                is_global: true,
                prehashed_cache_key,
            });
        }
        FindCommandResult::Error(e) => {
            return Err(
                Error::CommandNotFoundInLocalDirectoryAndErrorReadingGlobalDirectory {
                    command: command_name.as_ref().to_string(),
                    error: e.to_string(),
                    local_log: log,
                    global_log,
                },
            );
        }
    };
    trace!("Global command not found");

    return Err(Error::CommandNotFound {
        command: command_name.as_ref().to_string(),
        error: "Command not found in global or local directory".to_string(),
        local_log: log,
        global_log,
    });
}
