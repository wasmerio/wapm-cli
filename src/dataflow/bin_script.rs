use crate::data::manifest::PACKAGES_DIR_NAME;
use std::fs;
use std::io::Write;
use std::path::Path;

pub const BIN_DIR_NAME: &str = ".bin";

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not save script file for command \"{}\". {}", _0, _1)]
    SaveError(String, String),
    #[fail(display = "Could not create file at \"{}\". {}", _0, _1)]
    FileCreationError(String, String),
}

#[cfg(not(target_os = "windows"))]
pub fn save_bin_script<P: AsRef<Path>>(directory: P, command_name: String) -> Result<(), Error> {
    let data = format!("#!/bin/bash\nwapm run {} \"$@\"\n", command_name);
    save(data, directory, command_name)
}

#[cfg(target_os = "windows")]
pub fn save_bin_script<P: AsRef<Path>>(directory: P, command_name: String) -> Result<(), Error> {
    let data = format!("wapm run {} %*\n", command_name);
    let file_name = format!("{}.cmd", command_name);
    save(data, directory, file_name)
}

#[cfg(not(target_os = "windows"))]
pub fn delete_bin_script<P: AsRef<Path>>(directory: P, command_name: String) -> Result<(), Error> {
    delete(directory, command_name)
}

#[cfg(target_os = "windows")]
pub fn delete_bin_script<P: AsRef<Path>>(directory: P, command_name: String) -> Result<(), Error> {
    let file_name = format!("{}.cmd", command_name);
    delete(directory, file_name)
}

/// save the bin script for a command into the .bin directory
fn save<P: AsRef<Path>>(data: String, directory: P, command_name: String) -> Result<(), Error> {
    let mut dir = directory.as_ref().join(PACKAGES_DIR_NAME);
    dir.push(BIN_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| Error::SaveError(command_name.clone(), e.to_string()))?;
    }
    let script_path = dir.join(command_name.clone());
    let mut script_file = {
        let mut oo = fs::OpenOptions::new();
        oo.create(true).truncate(true).write(true);
        #[cfg(unix)]
        let oo = {
            use std::os::unix::fs::OpenOptionsExt;
            oo.mode(0o731)
        };
        oo.open(&script_path).map_err(|e| {
            Error::FileCreationError(script_path.to_string_lossy().to_string(), e.to_string())
        })?
    };
    script_file
        .write(data.as_bytes())
        .map_err(|e| Error::SaveError(command_name.clone(), e.to_string()))?;
    Ok(())
}

/// delete the bin script for a command - for cleanup during uninstall
fn delete<P: AsRef<Path>>(directory: P, command_name: String) -> Result<(), Error> {
    let mut dir = directory.as_ref().join(PACKAGES_DIR_NAME);
    dir.push(BIN_DIR_NAME);
    if !dir.exists() {
        Ok(())
    } else {
        let script_path = dir.join(command_name.clone());
        if script_path.exists() {
            fs::remove_file(script_path)
                .map_err(|e| Error::SaveError(command_name.clone(), e.to_string()))?;
            Ok(())
        } else {
            Ok(())
        }
    }
}
