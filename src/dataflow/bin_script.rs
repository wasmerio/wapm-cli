use crate::data::manifest::PACKAGES_DIR_NAME;
use std::fs;
use std::path::Path;

const BIN_DIR_NAME: &str = ".bin";

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Could not save script file for command \"{}\". {}", _0, _1)]
    SaveError(String, failure::Error),
}

#[cfg(not(target_os = "windows"))]
pub fn save_in_script<P: AsRef<Path>>() -> Result<(), Error> {
    let data = format!("#!/bin/bash\nwapm run sqlite \"$@\"\n");
    save(data, directory, command_name)
}

#[cfg(target_os = "windows")]
pub fn save_bin_script<P: AsRef<Path>>(directory: P, command_name: String) -> Result<(), Error> {
    let data = format!("wapm run sqlite %*\n");
    let file_name = format!("{}.cmd", command_name);
    save(data, directory, file_name)
}

fn save<P: AsRef<Path>>(data: String, directory: P, command_name: String) -> Result<(), Error> {
    let mut dir = directory.as_ref().join(PACKAGES_DIR_NAME);
    dir.push(BIN_DIR_NAME);
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| Error::SaveError(command_name.clone(), e.into()))?;
    }
    let script_path = dir.join(command_name.clone());
    fs::write(script_path, data).map_err(|e| Error::SaveError(command_name.clone(), e.into()))?;
    Ok(())
}
