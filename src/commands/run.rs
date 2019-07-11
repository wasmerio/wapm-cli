use crate::config::Config;
use crate::data::lock::is_lockfile_out_of_date;
use crate::dataflow;
use crate::dataflow::find_command_result;
use crate::dataflow::find_command_result::get_command_from_anywhere;
use crate::dataflow::manifest_packages::ManifestResult;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use structopt::StructOpt;

const DEFAULT_RUNTIME: &str = "wasmer";

#[derive(StructOpt, Debug)]
pub struct RunOpt {
    /// Command name
    command: String,
    /// WASI pre-opened directory
    #[structopt(long = "dir", multiple = true, group = "wasi")]
    pre_opened_directories: Vec<String>,
    /// Application arguments
    #[structopt(raw(multiple = "true"), parse(from_os_str))]
    args: Vec<OsString>,
}

pub fn run(run_options: RunOpt) -> Result<(), failure::Error> {
    let command_name = run_options.command.as_str();
    let args = &run_options.args;
    let current_dir = env::current_dir()?;

    // always update the local lockfile if the manifest has changed
    match is_lockfile_out_of_date(&current_dir) {
        Ok(false) => {}
        _ => dataflow::update(vec![], vec![], &current_dir)
            .map(|_| ())
            .map_err(|e| RunError::CannotRegenLockfile(command_name.to_string(), e))?,
    }

    let find_command_result::Command {
        source: source_path_buf,
        manifest_dir,
        args: _,
        module_name,
        is_global,
        prehashed_cache_key,
    } = match get_command_from_anywhere(command_name) {
        Err(find_command_result::Error::CommandNotFound(command)) => {
            let package_info = find_command_result::PackageInfoFromCommand::get(command)?;
            return Err(format_err!("Command {} not found, but package {} version {} has this command. You can install it with `wapm install {}@{}`",
                  &package_info.command,
                  &package_info.namespaced_package_name,
                  &package_info.version,
                  &package_info.namespaced_package_name,
                  &package_info.version,
            ));
        }
        otherwise => otherwise?,
    };

    // do not run with wasmer options if running a global command
    // this will change in the future.
    let wasmer_extra_flags: Option<Vec<OsString>> =
        if !is_global {
            match ManifestResult::find_in_directory(&current_dir) {
                ManifestResult::Manifest(manifest) => manifest
                    .package
                    .wasmer_extra_flags
                    .clone()
                    .map(|extra_flags| {
                        extra_flags
                            .split_whitespace()
                            .map(|str| OsString::from(str))
                            .collect()
                    }),
                _ => None,
            }
        } else {
            None
        };

    let run_dir = if is_global {
        Config::get_globals_directory().unwrap()
    } else {
        current_dir.clone()
    };

    let manifest_dir = run_dir.join(manifest_dir);

    debug!(
        "Running module located at {:?}",
        &run_dir.join(&source_path_buf)
    );

    run_dir.join(&source_path_buf).metadata().map_err(|_| {
        RunError::SourceForCommandNotFound(
            command_name.to_string(),
            module_name.to_string(),
            source_path_buf.to_string_lossy().to_string(),
        )
    })?;

    let mut wasi_preopened_dir_flags: Vec<OsString> = run_options
        .pre_opened_directories
        .iter()
        .map(|entry| OsString::from(format!("--dir={}", entry)))
        .collect();

    let mut disable_command_rename = false;

    match ManifestResult::find_in_directory(&manifest_dir) {
        ManifestResult::Manifest(manifest) => {
            disable_command_rename = manifest.package.disable_command_rename;
            if let Some(ref fs) = manifest.fs {
                // todo: normalize (rm `:` and newline, etc) these paths if we haven't yet
                for (guest_path, host_path) in fs.iter() {
                    wasi_preopened_dir_flags.push(OsString::from(format!(
                        "--mapdir={}:{}",
                        guest_path,
                        manifest_dir.join(host_path).to_string_lossy(),
                    )));
                }
            }
        }
        _ => (),
    }

    let command_vec = create_run_command(
        args,
        wasmer_extra_flags,
        wasi_preopened_dir_flags,
        &run_dir,
        source_path_buf,
        if disable_command_rename {
            None
        } else {
            Some(format!("wapm run {}", command_name))
        },
        prehashed_cache_key,
    )?;
    debug!("Running command with args: {:?}", command_vec);
    let mut child = Command::new(DEFAULT_RUNTIME)
        .args(&command_vec)
        .spawn()
        .map_err(|e| -> failure::Error {
            RunError::ProcessFailed {
                runtime: DEFAULT_RUNTIME.to_string(),
                error: format!("{:?}", e),
            }
            .into()
        })?;

    child.wait()?;
    Ok(())
}

fn create_run_command<P: AsRef<Path>, P2: AsRef<Path>>(
    args: &Vec<OsString>,
    wasmer_extra_flags: Option<Vec<OsString>>,
    wasi_preopened_dir_flags: Vec<OsString>,
    directory: P,
    wasm_file_path: P2,
    override_command_name: Option<String>,
    prehashed_cache_key: Option<String>,
) -> Result<Vec<OsString>, failure::Error> {
    let mut path = PathBuf::new();
    path.push(directory);
    path.push(wasm_file_path);
    let path_string = path.into_os_string();
    let command_vec = vec![OsString::from("run"), path_string];
    let override_command_name_vec = override_command_name
        .map(|cn| {
            vec![
                OsString::from("--command-name"),
                OsString::from(format!("\"{}\"", cn)),
            ]
        })
        .unwrap_or_default();
    let prehashed_cache_key_flag = prehashed_cache_key
        .map(|pck| vec![OsString::from(format!("--cache-key=\"{}\"", pck))])
        .unwrap_or_default();

    // NOTE:
    // for optional types, use an empty vec here:
    // an empty OsString may pass empty args to the child program which can cause issues
    Ok([
        &command_vec[..],
        &override_command_name_vec[..],
        &wasi_preopened_dir_flags[..],
        &wasmer_extra_flags.unwrap_or_default()[..],
        &prehashed_cache_key_flag[..],
        &[OsString::from("--")],
        &args[..],
    ]
    .concat())
}

#[cfg(test)]
mod test {
    use crate::commands::run::create_run_command;
    use crate::data::manifest::PACKAGES_DIR_NAME;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn create_run_command_vec() {
        let args: Vec<OsString> = vec![OsString::from("arg1"), OsString::from("arg2")];
        let tmp_dir = tempdir::TempDir::new("create_run_command_vec").unwrap();
        let dir = tmp_dir.path();
        let wapm_module_dir = dir.join(
            [PACKAGES_DIR_NAME, "_", "foo@1.0.2"]
                .iter()
                .collect::<PathBuf>(),
        );
        fs::create_dir_all(&wapm_module_dir).unwrap();
        // calling dunce here to help wih comparing paths on different platforms
        let expected_dir: PathBuf = wapm_module_dir.clone();
        let expected_dir = expected_dir.join("foo_entry.wasm");
        let expected_command = vec![
            OsString::from("run"),
            expected_dir.into_os_string(),
            OsString::from("--"),
            OsString::from("arg1"),
            OsString::from("arg2"),
        ];
        let wasm_relative_path: PathBuf = ["wapm_packages", "_", "foo@1.0.2", "foo_entry.wasm"]
            .iter()
            .collect();
        let actual_command =
            create_run_command(&args, None, vec![], &dir, wasm_relative_path, None).unwrap();
        assert_eq!(expected_command, actual_command);
    }
}

#[derive(Debug, Fail)]
enum RunError {
    #[fail(display = "Failed to run command \"{}\". {}", _0, _1)]
    CannotRegenLockfile(String, dataflow::Error),
    #[fail(
        display = "The command \"{}\" for module \"{}\" is defined but the source at \"{}\" does not exist.",
        _0, _1, _2
    )]
    SourceForCommandNotFound(String, String, String),
    #[fail(display = "Failed to run {}: {}", runtime, error)]
    ProcessFailed { runtime: String, error: String },
}
