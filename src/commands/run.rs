use crate::lock::{is_lockfile_out_of_date, regenerate_lockfile, Lockfile};
use crate::manifest::Manifest;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct RunOpt {
    /// Command name
    command: String,
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
        _ => regenerate_lockfile(vec![])
            .map_err(|e| RunError::CannotRegenLockfile(command_name.to_string(), e))?,
    }
    let mut lockfile_string = String::new();
    let lockfile = Lockfile::open(&current_dir, &mut lockfile_string)
        .map_err(|err| RunError::MissingLockFile(format!("{}", err)))?;
    let lockfile_command = lockfile.get_command(command_name)
        .map_err(|_| RunError::CommandNotFound(command_name.to_string()))?;

    let mut wasmer_extra_flags: Option<Vec<OsString>> = None;
    // hack to get around running commands for local modules
    let source_path: PathBuf = if let Ok(manifest) = Manifest::find_in_directory(&current_dir) {
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
            let modules = manifest.module.unwrap();
            let source = modules
                .iter()
                .find(|m| m.name == lockfile_command.module)
                .map(|m| m.source.as_path())
                .unwrap();
            source.to_path_buf()
        } else {
            let lockfile_module = lockfile.get_module(
                lockfile_command.package_name,
                lockfile_command.package_version,
                lockfile_command.module,
            )?;
            PathBuf::from(&lockfile_module.entry)
        }
    } else {
        let lockfile_module = lockfile.get_module(
            lockfile_command.package_name,
            lockfile_command.package_version,
            lockfile_command.module,
        )?;
        PathBuf::from(&lockfile_module.entry)
    };

    let command_vec = create_run_command(
        args,
        wasmer_extra_flags,
        &current_dir,
        &source_path,
        Some(format!("wapm run {}", command_name)),
    )?;
    let mut child = Command::new("wasmer").args(&command_vec).spawn()?;
    child.wait()?;
    Ok(())
}

fn create_run_command<P: AsRef<Path>, P2: AsRef<Path>>(
    args: &Vec<OsString>,
    wasmer_extra_flags: Option<Vec<OsString>>,
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
        &wasmer_extra_flags.unwrap_or_default()[..],
        &[OsString::from("--")],
        &args[..],
    ]
    .concat())
}

#[cfg(test)]
mod test {
    use crate::commands::run::create_run_command;
    use crate::lock::Lockfile;
    use crate::manifest::PACKAGES_DIR_NAME;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn create_run_command_vec() {
        // lockfile
        let lock_toml = toml! {
            [modules."_/foo"."1.0.2"."foo_mod"]
            package_name = "_/foo"
            package_version = "1.0.2"
            name = "foo_mod"
            source = "registry+foo"
            resolved = ""
            integrity = ""
            hash = ""
            abi = "none"
            entry = "foo_entry.wasm"
            [modules."_/bar"."3.0.0"."bar_mod"]
            package_name = "_/bar"
            package_version = "3.0.0"
            name = "bar_mod"
            source = "registry+bar"
            resolved = ""
            integrity = ""
            hash = ""
            abi = "none"
            entry = "bar.wasm"
            [commands.do_more_foo_stuff]
            package_name = "_/foo"
            package_version = "1.0.2"
            name = "do_more_foo_stuff"
            module = "foo_mod"
            is_top_level_dependency = true
            [commands.do_bar_stuff]
            package_name = "_/bar"
            package_version = "3.0.0"
            name = "do_bar_stuff"
            module = "bar_mod"
            is_top_level_dependency = true
        };
        let args: Vec<OsString> = vec![OsString::from("arg1"), OsString::from("arg2")];
        let tmp_dir = tempdir::TempDir::new("create_run_command_vec").unwrap();
        let dir = tmp_dir.path();
        let wapm_module_dir = dir.join(
            [PACKAGES_DIR_NAME, "_/foo@1.0.2"]
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
            create_run_command(&args, None, &dir, wasm_relative_path, None).unwrap();
        assert_eq!(expected_command, actual_command);
    }
}

#[derive(Debug, Fail)]
enum RunError {
    #[fail(display = "Failed to run command \"{}\". {}", _0, _1)]
    CannotRegenLockfile(String, failure::Error),
    #[fail(display = "Could not find lock file: {}", _0)]
    MissingLockFile(String),
    #[fail(display = "Command \"{}\" not found in the manifest and not imported from any dependencies.", _0)]
    CommandNotFound(String)
}
