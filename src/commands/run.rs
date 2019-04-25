use crate::config::Config;
use crate::dataflow::find_command_result::get_command_from_anywhere;
use crate::dataflow::manifest_packages::ManifestResult;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use structopt::StructOpt;

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
    let (source_path_buf, _command_args, module_name, is_global) =
        get_command_from_anywhere(command_name)?;

    // do not run with wasmer options if running a global command
    // this will change in the future.
    let wasmer_extra_flags: Option<Vec<OsString>> = if !is_global {
        match ManifestResult::find_in_directory(&current_dir) {
            ManifestResult::Manifest(manifest) => {
                manifest
                    .package
                    .wasmer_extra_flags
                    .clone()
                    .map(|extra_flags| {
                        extra_flags
                            .split_whitespace()
                            .map(|str| OsString::from(str))
                            .collect()
                    })
            }
            _ => None,
        }
    }
    else {
        None
    };


    let run_dir = if is_global {
        Config::get_globals_directory().unwrap()
    } else {
        current_dir
    };

    run_dir.join(&source_path_buf).metadata().map_err(|_| {
        RunError::SourceForCommandNotFound(
            command_name.to_string(),
            module_name.to_string(),
            source_path_buf.to_string_lossy().to_string(),
        )
    })?;

    let wasi_preopened_dir_flags = run_options
        .pre_opened_directories
        .iter()
        .map(|entry| OsString::from(format!("--dir={}", entry)))
        .collect();

    let command_vec = create_run_command(
        args,
        wasmer_extra_flags,
        wasi_preopened_dir_flags,
        &run_dir,
        source_path_buf,
        Some(format!("wapm run {}", command_name)),
    )?;
    let mut child = Command::new("wasmer").args(&command_vec).spawn()?;
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
) -> Result<Vec<OsString>, failure::Error> {
    let mut path = PathBuf::new();
    path.push(directory);
    path.push(wasm_file_path);
    let path_string = path.into_os_string();
    let command_vec = vec![OsString::from("run"), path_string];
    let override_command_name_vec = override_command_name
        .map(|cn| vec![OsString::from("--command-name"), OsString::from(cn)])
        .unwrap_or_default();

    Ok([
        &command_vec[..],
        &override_command_name_vec[..],
        &wasi_preopened_dir_flags[..],
        &wasmer_extra_flags.unwrap_or_default()[..],
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
    #[fail(
        display = "The command \"{}\" for module \"{}\" is defined but the source at \"{}\" does not exist.",
        _0, _1, _2
    )]
    SourceForCommandNotFound(String, String, String),
}
