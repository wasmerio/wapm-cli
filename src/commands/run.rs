use crate::lock::{is_lockfile_out_of_date, regenerate_lockfile, Lockfile};
use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
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
    let manifest_path = current_dir.join(MANIFEST_FILE_NAME);
    let manifest = Manifest::open(&manifest_path);
    let mut lockfile_string = String::new();
    let lockfile = Lockfile::open(&current_dir, &mut lockfile_string);

    // regenerate the lockfile if it is out of date
    match is_lockfile_out_of_date(&current_dir) {
        Ok(false) => {}
        _ => regenerate_lockfile(manifest, lockfile, vec![])
            .map_err(|err| RunError::CannotRegenLockFile(format!("{}", err)))?,
    }
    let mut lockfile_string = String::new();
    let lockfile = Lockfile::open(&current_dir, &mut lockfile_string)
        .map_err(|err| RunError::MissingLockFile(format!("{}", err)))?;
    let lockfile_command = lockfile.get_command(command_name)?;

    // hack to get around running commands for local modules
    let source_path: PathBuf = if let Ok(manifest) = Manifest::open(manifest_path) {
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

    let command_vec = create_run_command(args, &current_dir, &source_path)?;
    let mut child = Command::new("wasmer").args(&command_vec).spawn()?;
    child.wait()?;
    Ok(())
}

fn create_run_command<P: AsRef<Path>, P2: AsRef<Path>>(
    args: &Vec<OsString>,
    directory: P,
    wasm_file_path: P2,
) -> Result<Vec<OsString>, failure::Error> {
    let mut path = PathBuf::new();
    path.push(directory);
    path.push(wasm_file_path);
    let path_string = path.into_os_string();
    let command_vec = vec![OsString::from("run"), path_string, OsString::from("--")];
    Ok([&command_vec[..], &args[..]].concat())
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
            abi = "None"
            entry = "foo_entry.wasm"
            [modules."_/bar"."3.0.0"."bar_mod"]
            package_name = "_/bar"
            package_version = "3.0.0"
            name = "bar_mod"
            source = "registry+bar"
            resolved = ""
            integrity = ""
            hash = ""
            abi = "None"
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
        let lock_toml_string = lock_toml.to_string();
        let lockfile: Lockfile = toml::from_str(&lock_toml_string).unwrap();
        let lockfile_module = lockfile.get_module("_/foo", "1.0.2", "foo_mod").unwrap();
        let lockfile_command = lockfile.get_command("do_more_foo_stuff").unwrap();
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
        let expected_dir: PathBuf = dunce::canonicalize(&wapm_module_dir).unwrap();
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
        let actual_command = create_run_command(&args, &dir, wasm_relative_path).unwrap();
        assert_eq!(expected_command, actual_command);
    }
}

#[derive(Debug, Fail)]
enum RunError {
    #[fail(display = "Failed to regenerate lock file: {}", _0)]
    CannotRegenLockFile(String),

    #[fail(display = "Could not find lock file: {}", _0)]
    MissingLockFile(String),
}
