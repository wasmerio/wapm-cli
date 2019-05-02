use crate::config::Config;
use crate::data::lock::lockfile::Lockfile;
use crate::data::manifest::Manifest;
use crate::dataflow::lockfile_packages::LockfileResult;
use crate::dataflow::manifest_packages::ManifestResult;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(
        display = "Command \"{}\" was not found in the local directory or the global install directory.",
        _0
    )]
    CommandNotFound(String),
    #[fail(
        display = "Command \"{}\" was not found in the local directory. There was an error parsing the global lockfile. {}",
        _0, _1
    )]
    CommandNotFoundInLocalDirectoryAndErrorReadingGlobalDirectory(String, String),
    #[fail(
        display = "Could not get command \"{}\" because there was a problem with the local package. {}",
        _0, _1
    )]
    ErrorReadingLocalDirectory(String, String),
    #[fail(
        display = "Command \"{}\" exists in lockfile, but corresponding module \"{}\" not found in lockfile.",
        _0, _1
    )]
    CommandFoundButCorrespondingModuleIsMissing(String, String),
    #[fail(
        display = "Failed to get command \"{}\" because there was an error opening the global installation directory. {}",
        _0, _1
    )]
    CouldNotOpenGlobalsDirectory(String, String),
}

pub enum FindCommandResult {
    CommandNotFound(String),
    CommandFound(PathBuf, Option<String>, String), // source, args, module name
    Error(failure::Error),
}

impl FindCommandResult {
    fn find_command_in_manifest_and_lockfile<S: AsRef<str>>(
        command_name: S,
        manifest: Manifest,
        lockfile: Lockfile,
    ) -> Self {
        match lockfile.get_command(command_name.as_ref()) {
            Err(e) => FindCommandResult::Error(e),
            Ok(lockfile_command) => {
                if lockfile_command.package_name == manifest.package.name {
                    // this is a local module command
                    let found_module = manifest.module.as_ref().and_then(|modules| {
                        modules.iter().find(|m| m.name == lockfile_command.module)
                    });
                    match found_module {
                        Some(module) => FindCommandResult::CommandFound(
                            module.source.clone(),
                            lockfile_command.main_args.clone(),
                            module.name.clone(),
                        ),
                        None => FindCommandResult::Error(
                            Error::CommandFoundButCorrespondingModuleIsMissing(
                                command_name.as_ref().to_string(),
                                lockfile_command.module.clone(),
                            )
                            .into(),
                        ),
                    }
                } else {
                    match lockfile.get_module(
                        &lockfile_command.package_name,
                        &lockfile_command.package_version,
                        &lockfile_command.module,
                    ) {
                        Ok(lockfile_module) => {
                            let path = PathBuf::from(&lockfile_module.entry);
                            FindCommandResult::CommandFound(
                                path,
                                lockfile_command.main_args.clone(),
                                lockfile_module.name.clone(),
                            )
                        }
                        Err(e) => FindCommandResult::Error(e),
                    }
                }
            }
        }
    }

    fn find_command_in_lockfile<S: AsRef<str>>(command_name: S, lockfile: Lockfile) -> Self {
        match lockfile.get_command(command_name.as_ref()) {
            Ok(lockfile_command) => {
                match lockfile.get_module(
                    &lockfile_command.package_name,
                    &lockfile_command.package_version,
                    &lockfile_command.module,
                ) {
                    Ok(lockfile_module) => {
                        let path = PathBuf::from(&lockfile_module.entry);
                        FindCommandResult::CommandFound(
                            path,
                            lockfile_command.main_args.clone(),
                            lockfile_module.name.clone(),
                        )
                    }
                    Err(_e) => {
                        FindCommandResult::CommandNotFound(command_name.as_ref().to_string())
                    }
                }
            }
            Err(_e) => FindCommandResult::CommandNotFound(command_name.as_ref().to_string()),
        }
    }

    pub fn find_command_in_directory<P: AsRef<Path>, S: AsRef<str>>(
        directory: P,
        command_name: S,
    ) -> Self {
        let manifest_result = ManifestResult::find_in_directory(&directory);
        let lockfile_result = LockfileResult::find_in_directory(&directory);
        match (manifest_result, lockfile_result) {
            (ManifestResult::ManifestError(e), _) => return FindCommandResult::Error(e.into()),
            (_, LockfileResult::LockfileError(e)) => return FindCommandResult::Error(e.into()),
            (ManifestResult::NoManifest, LockfileResult::NoLockfile) => {} // continue
            (ManifestResult::NoManifest, LockfileResult::Lockfile(l)) => {
                return Self::find_command_in_lockfile(command_name, l);
            }
            // the edge case of a manifest, but no lockfile would an invalid state. This function
            // should always be run after updating the lockfile with the latest manifest changes.
            // If that function were to fail so horribly that it did not error, and no lockfile was
            // generated, then we will get this panic.
            (ManifestResult::Manifest(_m), LockfileResult::NoLockfile) => {
                panic!("Manifest exists, but lockfile not found!")
            }
            (ManifestResult::Manifest(m), LockfileResult::Lockfile(l)) => {
                return Self::find_command_in_manifest_and_lockfile(command_name, m, l);
            }
        };
        FindCommandResult::CommandNotFound(command_name.as_ref().to_string())
    }
}

pub struct Command {
    // PathBuf, Option<String>, String, bool
    pub source: PathBuf,
    pub args: Option<String>,
    pub module_name: String,
    pub is_global: bool,
}

/// Get a command from anywhere, where anywhere is the set of packages in the local lockfile and the global lockfile.
/// A flag indicating global run is also returned. Commands are found in local lockfile first.
pub fn get_command_from_anywhere<S: AsRef<str>>(command_name: S) -> Result<Command, Error> {
    // look in the local directory, update if necessary
    let current_directory = env::current_dir().unwrap();
    let local_command_result =
        FindCommandResult::find_command_in_directory(&current_directory, &command_name);

    match local_command_result {
        FindCommandResult::CommandNotFound(_cmd) => {} // continue
        FindCommandResult::CommandFound(path, args, module_name) => {
            return Ok(Command {
                source: path,
                args,
                module_name,
                is_global: false,
            });
        }
        FindCommandResult::Error(e) => {
            return Err(Error::ErrorReadingLocalDirectory(
                command_name.as_ref().to_string(),
                e.to_string(),
            ));
        }
    };

    // look in the global directory
    let global_directory = Config::get_globals_directory().map_err(|e| {
        Error::CouldNotOpenGlobalsDirectory(command_name.as_ref().to_string(), e.to_string())
    })?;
    let global_command_result =
        FindCommandResult::find_command_in_directory(&global_directory, &command_name);

    match global_command_result {
        FindCommandResult::CommandNotFound(_) => {} // continue
        FindCommandResult::CommandFound(path, args, module_name) => {
            return Ok(Command {
                source: path,
                args,
                module_name,
                is_global: true,
            });
        }
        FindCommandResult::Error(e) => {
            return Err(
                Error::CommandNotFoundInLocalDirectoryAndErrorReadingGlobalDirectory(
                    command_name.as_ref().to_string(),
                    e.to_string(),
                ),
            );
        }
    };

    return Err(Error::CommandNotFound(command_name.as_ref().to_string()));
}
