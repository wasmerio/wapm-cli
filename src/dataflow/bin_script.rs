#![cfg_attr(target_os = "wasi", allow(dead_code))]
use crate::data::manifest::PACKAGES_DIR_NAME;
use std::fs;
use std::io::Write;
use std::path::Path;
use thiserror::Error;

pub const BIN_DIR_NAME: &str = ".bin";

#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("Could not save script file for command \"{0}\". {1}")]
    SaveError(String, String),
    #[error("Could not create file at \"{0}\". {1}")]
    FileCreationError(String, String),
}

#[cfg(target_os = "wasi")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasConfig {
    pub run: String,
    #[serde(default)]
    pub chroot: bool,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub mappings: Vec<String>,
}

#[cfg(target_os = "wasi")]
pub fn save_bin_script<P: AsRef<Path>>(
    _directory: P,
    command_name: String,
    package_path: String,
    module_path: String,
) -> Result<(), Error> {
    let current_dir = crate::config::Config::get_current_dir()
        .ok()
        .unwrap_or_else(|| std::path::PathBuf::from("/".to_string()));
    let command_path = format!("/bin/{}.alias", command_name);
    let package_path = current_dir
        .clone()
        .join("wapm_packages")
        .join(package_path)
        .to_string_lossy()
        .to_string();
    let module_path = current_dir
        .clone()
        .join("wapm_packages")
        .join(module_path)
        .to_string_lossy()
        .to_string();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(Path::new(command_path.as_str()))
        .map_err(|e| Error::FileCreationError(command_path, e.to_string()))?;

    let mut mappings = Vec::new();
    match crate::dataflow::ManifestResult::find_in_directory(&package_path) {
        crate::dataflow::ManifestResult::Manifest(manifest) => {
            if let Some(ref fs) = manifest.fs {
                for (guest_path, host_path) in fs.iter() {
                    mappings.push(format!(
                        "{}:{}/{}",
                        guest_path,
                        package_path,
                        host_path.to_string_lossy()
                    ));
                }
            }
        }
        _ => (),
    }

    let alias = AliasConfig {
        run: module_path,
        chroot: false,
        base: Some(package_path),
        mappings,
    };

    let data = serde_yaml::to_vec(&alias)
        .map_err(|e| Error::SaveError(command_name.clone(), e.to_string()))?;
    file.write_all(&data[..])
        .map_err(|e| Error::SaveError(command_name.clone(), e.to_string()))?;
    Ok(())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
pub fn save_bin_script<P: AsRef<Path>>(
    directory: P,
    command_name: String,
    _package_path: String,
    _module_path: String,
) -> Result<(), Error> {
    let data = format!("#!/bin/bash\nwapm run {} \"$@\"\n", command_name);
    save(data, directory, command_name)
}

#[cfg(target_os = "windows")]
pub fn save_bin_script<P: AsRef<Path>>(
    directory: P,
    command_name: String,
    _package_path: String,
    _module_path: String,
) -> Result<(), Error> {
    let data = format!("@\"wapm\" run {} %*\n", command_name);
    let file_name = format!("{}.cmd", command_name);
    save(data, directory, file_name)
}

#[cfg(target_os = "wasi")]
pub fn delete_bin_script<P: AsRef<Path>>(_directory: P, command_name: String) -> Result<(), Error> {
    let command_path = format!("/bin/{}", command_name);
    if Path::new(command_path.as_str()).exists() {
        fs::remove_file(command_path)
            .map_err(|e| Error::SaveError(command_name.clone(), e.to_string()))?;
    }
    Ok(())
}

#[cfg(all(not(target_os = "windows"), not(target_os = "wasi")))]
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
    #[cfg(unix)]
    let maybe_unix_mode = {
        use std::os::unix::fs::PermissionsExt;
        script_path.metadata().map(|md| md.permissions().mode())
    };
    let mut script_file = {
        let mut oo = fs::OpenOptions::new();
        oo.create(true).truncate(true).write(true);
        #[cfg(unix)]
        let oo = {
            use std::os::unix::fs::OpenOptionsExt;
            if let Ok(unix_mode) = maybe_unix_mode {
                oo.mode(unix_mode | 0o110)
            } else {
                oo.mode(0o754)
            }
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
