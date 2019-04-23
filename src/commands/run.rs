use crate::data::lock::is_lockfile_out_of_date;
use crate::data::manifest::Manifest;
use crate::dataflow;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use structopt::StructOpt;
use crate::dataflow::lockfile_packages::LockfileResult;

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
    // regenerate the lockfile if it is out of date
    match is_lockfile_out_of_date(&current_dir) {
        Ok(false) => {}
        _ => dataflow::update(vec![], &current_dir)
            .map_err(|e| RunError::CannotRegenLockfile(command_name.to_string(), e))?,
    }
    let lockfile_result = LockfileResult::find_in_directory(&current_dir);
    let lockfile = match lockfile_result {
        LockfileResult::NoLockfile => return Err(RunError::MissingLockFile("Lockfile was not generated.".to_string()).into()),
        LockfileResult::LockfileError(e) => return Err(RunError::MissingLockFile(format!("There was an issue opening the lockfile. {}", e.to_string())).into()),
        LockfileResult::Lockfile(lockfile) => lockfile,
    };

    let mut wasmer_extra_flags: Option<Vec<OsString>> = None;
    let manifest_result = Manifest::find_in_directory(&current_dir);
    // hack to get around running commands for local modules
    let (module_name, source_path): (String, String) = if let Ok(ref manifest) = manifest_result {
        let lockfile_command = lockfile
            .get_command(command_name)
            .map_err(|_| RunError::CommandNotFound(command_name.to_string()))?;

        wasmer_extra_flags = manifest
            .package
            .wasmer_extra_flags
            .clone()
            .map(|extra_flags| {
                extra_flags
                    .split_whitespace()
                    .map(|str| OsString::from(str))
                    .collect()
            });

        if lockfile_command.package_name == manifest.package.name {
            // this is a local module command
            let module = manifest.module.as_ref().map(|modules| {
                let module = modules.iter().find(|m| m.name == lockfile_command.module);
                module
            });
            module
                .unwrap_or(None)
                .map(|module| {
                    (
                        module.name.clone(),
                        module.source.clone().to_string_lossy().to_string(),
                    )
                })
                .ok_or(RunError::FoundCommandInLockfileButMissingModule(
                    command_name.to_string(),
                    lockfile_command.module.to_string(),
                    lockfile_command.package_name.to_string(),
                ))?
        } else {
            let lockfile_module = lockfile.get_module(
                &lockfile_command.package_name,
                &lockfile_command.package_version,
                &lockfile_command.module,
            )?;
            (lockfile_module.name.clone(), lockfile_module.entry.clone())
        }
    } else {
        let lockfile_command = lockfile
            .get_command(command_name)
            .map_err(|_| RunError::CommandNotFoundInDependencies(command_name.to_string()))?;

        let lockfile_module = lockfile.get_module(
            &lockfile_command.package_name,
            &lockfile_command.package_version,
            &lockfile_command.module,
        )?;
        (lockfile_module.name.clone(), lockfile_module.entry.clone())
    };

    // check that the source exists
    let source_path_buf = PathBuf::from(&source_path);
    source_path_buf.metadata().map_err(|_| {
        RunError::SourceForCommandNotFound(
            command_name.to_string(),
            module_name.to_string(),
            source_path.to_string(),
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
        &current_dir,
        source_path,
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
    use crate::data::manifest::PACKAGES_DIR_NAME;
    use crate::commands::run::create_run_command;
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
    #[fail(display = "Could not find lock file: {}", _0)]
    MissingLockFile(String),
    #[fail(
        display = "Command \"{}\" not found in the current package manifest or any of the installed dependencies.",
        _0
    )]
    CommandNotFound(String),
    #[fail(
        display = "Command \"{}\" not found in the installed dependencies.",
        _0
    )]
    CommandNotFoundInDependencies(String),
    #[fail(
        display = "The command \"{}\" for module \"{}\" is defined but the source at \"{}\" does not exist.",
        _0, _1, _2
    )]
    SourceForCommandNotFound(String, String, String),
    #[fail(
        display = "Command \"{}\" was found in the lockfile but the module \"{}\" from package \"{}\" was not found in the lockfile. Did you modify the lockfile?",
        _0, _1, _2
    )]
    FoundCommandInLockfileButMissingModule(String, String, String),
}
