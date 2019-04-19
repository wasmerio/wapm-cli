use crate::abi::Abi;
use std::{fs, io::Read, path::PathBuf};
use crate::cfg_toml::manifest::Manifest;

pub fn validate_directory(pkg_path: PathBuf) -> Result<(), failure::Error> {
    // validate as dir
    let manifest = Manifest::find_in_directory(pkg_path.clone())?;
    if let Some(modules) = manifest.module {
        for module in modules.iter() {
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
            let detected_abi = validate_wasm_and_report_errors(&wasm_buffer, source_path_string)?;

            if module.abi != Abi::None && module.abi != detected_abi {
                return Err(ValidationError::MismatchedABI {
                    module_name: module.name.clone(),
                    found_abi: detected_abi,
                    expected_abi: module.abi,
                }
                .into());
            }
        }
    }

    Ok(())
}

pub fn validate_wasm_and_report_errors(
    wasm: &[u8],
    file_name: String,
) -> Result<Abi, failure::Error> {
    use wasmparser::WasmDecoder;
    let mut parser = wasmparser::ValidatingParser::new(wasm, None);
    let mut abi = Abi::None;
    loop {
        let state = parser.read();
        match *state {
            wasmparser::ParserState::EndWasm => return Ok(abi),
            wasmparser::ParserState::Error(e) => {
                return Err(ValidationError::InvalidWasm {
                    file: file_name,
                    error: format!("{}", e),
                }
                .into());
            }
            wasmparser::ParserState::ImportSectionEntry {
                module: "wasi_unstable",
                ..
            } => {
                if abi == Abi::None || abi == Abi::Wasi {
                    abi = Abi::Wasi;
                } else {
                    return Err(ValidationError::MultipleABIs {
                        file: file_name,
                        first_abi: abi,
                        second_abi: Abi::Wasi,
                    }
                    .into());
                }
            }
            wasmparser::ParserState::ImportSectionEntry {
                module: "env",
                field: "_emscripten_memcpy_big",
                ..
            } => {
                if abi == Abi::None || abi == Abi::Emscripten {
                    abi = Abi::Emscripten;
                } else {
                    return Err(ValidationError::MultipleABIs {
                        file: file_name,
                        first_abi: abi,
                        second_abi: Abi::Emscripten,
                    }
                    .into());
                }
            }
            _ => {}
        }
    }
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
    #[fail(
        display = "Multiple ABIs detected in file {}; previously detected {} but found {}",
        file, first_abi, second_abi
    )]
    MultipleABIs {
        file: String,
        first_abi: Abi,
        second_abi: Abi,
    },
    #[fail(
        display = "Detected ABI ({}) does not match ABI specified in wapm.toml ({}) for module \"{}\"",
        found_abi, expected_abi, module_name
    )]
    MismatchedABI {
        module_name: String,
        found_abi: Abi,
        expected_abi: Abi,
    },
    #[fail(display = "Failed to unpack archive \"{}\"! {}", file, error)]
    CannotUnpackArchive { file: String, error: String },
}
