#![cfg_attr(
    not(feature = "full"),
    allow(dead_code, unused_imports, unused_variables)
)]
#[cfg(feature = "full")]
use crate::database;
use crate::dataflow::{interfaces::InterfaceFromServer, manifest_packages::ManifestResult};
#[cfg(feature = "full")]
use crate::interfaces;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use thiserror::Error;
use wasmer_wasm_interface::{validate, Interface};

#[cfg(feature = "full")]
pub fn validate_directory(pkg_path: PathBuf) -> anyhow::Result<()> {
    // validate as dir
    let manifest = match ManifestResult::find_in_directory(&pkg_path) {
        ManifestResult::NoManifest => return Ok(()),
        ManifestResult::ManifestError(e) => return Err(e.into()),
        ManifestResult::Manifest(manifest) => manifest,
    };
    if let Some(modules) = manifest.module {
        for module in modules.into_iter() {
            let source_path = if module.source.is_relative() {
                manifest.base_directory_path.join(&module.source)
            } else {
                module.source.clone()
            };
            let source_path_string = source_path.to_string_lossy().to_string();
            let mut wasm_file =
                fs::File::open(&source_path).map_err(|_| ValidationError::MissingFile {
                    file: source_path_string.clone(),
                })?;
            let mut wasm_buffer = Vec::new();
            wasm_file.read_to_end(&mut wasm_buffer).map_err(|err| {
                ValidationError::MiscCannotRead {
                    file: source_path_string.clone(),
                    error: format!("{}", err),
                }
            })?;

            if let Some(bindings) = &module.bindings {
                validate_bindings(bindings, &manifest.base_directory_path)?;
            }

            // hack, short circuit if no interface for now
            if module.interfaces.is_none() {
                return validate_wasm_and_report_errors_old(
                    &wasm_buffer[..],
                    source_path_string,
                );
            }

            let mut conn = database::open_db()?;
            let mut interface: Interface = Default::default();
            for (interface_name, interface_version) in
                module.interfaces.unwrap_or_default().into_iter()
            {
                if !interfaces::interface_exists(&mut conn, &interface_name, &interface_version)? {
                    // download interface and store it if we don't have it locally
                    let interface_data_from_server = InterfaceFromServer::get(
                        interface_name.clone(),
                        interface_version.clone(),
                    )?;
                    interfaces::import_interface(
                        &mut conn,
                        &interface_name,
                        &interface_version,
                        &interface_data_from_server.content,
                    )?;
                }
                let sub_interface = interfaces::load_interface_from_db(
                    &mut conn,
                    &interface_name,
                    &interface_version,
                )?;
                interface = interface
                    .merge(sub_interface)
                    .map_err(|e| anyhow!("Failed to merge interface {}: {}", &interface_name, e))?;
            }
            validate::validate_wasm_and_report_errors(&wasm_buffer, &interface).map_err(|e| {
                ValidationError::InvalidWasm {
                    file: source_path_string,
                    error: format!("{:?}", e),
                }
            })?;
        }
    }
    debug!("package at path {:#?} validated", &pkg_path);

    Ok(())
}

fn validate_bindings(
    bindings: &wapm_toml::Bindings,
    base_directory_path: &Path,
) -> Result<(), ValidationError> {
    for file in bindings.referenced_files(base_directory_path) {
        if !file.exists() {
            return Err(ValidationError::MissingFile {
                file: file.display().to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(not(feature = "full"))]
pub fn validate_directory(pkg_path: PathBuf) -> anyhow::Result<()> {
    Ok(())
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("WASM file \"{file}\" detected as invalid because {error}")]
    InvalidWasm { file: String, error: String },
    #[error("Could not find file {file}")]
    MissingFile { file: String },
    #[error("Failed to read file {file}; {error}")]
    MiscCannotRead { file: String, error: String },
    #[error("Failed to unpack archive \"{file}\"! {error}")]
    CannotUnpackArchive { file: String, error: String },
}

// legacy function, validates wasm.  TODO: clean up
pub fn validate_wasm_and_report_errors_old(wasm: &[u8], file_name: String) -> anyhow::Result<()> {
    use wasmparser::WasmDecoder;
    let mut parser = wasmparser::ValidatingParser::new(wasm, None);
    loop {
        let state = parser.read();
        match state {
            wasmparser::ParserState::EndWasm => return Ok(()),
            wasmparser::ParserState::Error(e) => {
                return Err(ValidationError::InvalidWasm {
                    file: file_name,
                    error: format!("{}", e),
                }
                .into());
            }
            _ => {}
        }
    }
}
