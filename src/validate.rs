use crate::contracts;
use crate::database;
use crate::dataflow::{contracts::ContractFromServer, manifest_packages::ManifestResult};
use std::{fs, io::Read, path::PathBuf};
use wasm_contract::{validate, Contract};

pub fn validate_directory(pkg_path: PathBuf) -> Result<(), failure::Error> {
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

            // hack, short circuit if no contract for now
            if module.contracts.is_none() {
                return validate_wasm_and_report_errors_old(
                    &wasm_buffer[..],
                    source_path_string.clone(),
                );
            }

            let mut conn = database::open_db()?;
            let mut contract: Contract = Default::default();
            for contract_id in module.contracts.unwrap_or_default().into_iter() {
                if !contracts::contract_exists(&mut conn, &contract_id.name, &contract_id.version)?
                {
                    // download contract and store it if we don't have it locally
                    let contract_data_from_server = ContractFromServer::get(
                        contract_id.name.clone(),
                        contract_id.version.clone(),
                    )?;
                    contracts::import_contract(
                        &mut conn,
                        &contract_id.name,
                        &contract_id.version,
                        &contract_data_from_server.content,
                    )?;
                }
                let sub_contract = contracts::load_contract_from_db(
                    &mut conn,
                    &contract_id.name,
                    &contract_id.version,
                )?;
                contract = contract.merge(sub_contract).map_err(|e| {
                    format_err!("Failed to merge contract {}: {}", &contract_id.name, e)
                })?;
            }
            validate::validate_wasm_and_report_errors(&wasm_buffer, &contract).map_err(|e| {
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

#[derive(Debug, Fail)]
pub enum ValidationError {
    #[fail(
        display = "WASM file \"{}\" detected as invalid because {}",
        file, error
    )]
    InvalidWasm { file: String, error: String },
    #[fail(display = "Could not find file {}", file)]
    MissingFile { file: String },
    #[fail(display = "Failed to read file {}; {}", file, error)]
    MiscCannotRead { file: String, error: String },
    #[fail(display = "Failed to unpack archive \"{}\"! {}", file, error)]
    CannotUnpackArchive { file: String, error: String },
}

// legacy function, validates wasm.  TODO: clean up
pub fn validate_wasm_and_report_errors_old(
    wasm: &[u8],
    file_name: String,
) -> Result<(), failure::Error> {
    use wasmparser::WasmDecoder;
    let mut parser = wasmparser::ValidatingParser::new(wasm, None);
    loop {
        let state = parser.read();
        match *state {
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
