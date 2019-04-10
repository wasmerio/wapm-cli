use crate::manifest::Manifest;

use flate2::read::GzDecoder;
use std::{fs, io::Read, path::PathBuf};
use structopt::StructOpt;
use tar::Archive;

#[derive(StructOpt, Debug)]
pub struct ValidateOpt {
    /// Directory or tar file to validate
    package: String,
}

pub fn validate(validate_opts: ValidateOpt) -> Result<(), failure::Error> {
    let pkg_path = PathBuf::from(&validate_opts.package);
    validate_manifest_and_modules(pkg_path)
}

pub fn validate_manifest_and_modules(pkg_path: PathBuf) -> Result<(), failure::Error> {
    if pkg_path.is_dir() {
        validate_directory(pkg_path)
    } else {
        //unzip then validate as dir
        let mut compressed_archive_data = Vec::new();
        let mut compressed_archive =
            fs::File::open(&pkg_path).map_err(|_| ValidationError::MissingFile {
                file: pkg_path.to_string_lossy().to_string(),
            })?;
        compressed_archive
            .read_to_end(&mut compressed_archive_data)
            .map_err(|err| ValidationError::MiscCannotRead {
                file: pkg_path.to_string_lossy().to_string(),
                error: format!("{}", err),
            })?;

        let mut gz = GzDecoder::new(&compressed_archive_data[..]);
        let mut archive_data = Vec::new();
        gz.read_to_end(&mut archive_data).unwrap();

        let temp_out_dir = tempdir::TempDir::new("temp").unwrap();
        let out_dir = temp_out_dir.path();
        let mut archive = Archive::new(archive_data.as_slice());
        // TODO: consider doing this entirely in memory with multiple passes
        archive
            .unpack(&out_dir)
            .map_err(|err| ValidationError::CannotUnpackArchive {
                file: pkg_path.to_string_lossy().to_string(),
                error: format!("{}", err),
            })?;

        let archive_path = {
            let mut ar_path = out_dir.clone().to_path_buf();
            let archive_folder_name = pkg_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(".tar.gz", "");
            ar_path.push(archive_folder_name);
            ar_path
        };

        validate_directory(archive_path)
    }
}

pub fn validate_directory(pkg_path: PathBuf) -> Result<(), failure::Error> {
    // validate as dir
    let manifest = Manifest::find_in_directory(pkg_path.clone())?;
    if let Some(module) = manifest.module {
        let path_str = module.module.to_string_lossy().to_string();
        let mut wasm_file =
            fs::File::open(module.module).map_err(|_| ValidationError::MissingFile {
                file: path_str.clone(),
            })?;
        let mut wasm_buffer = Vec::new();
        wasm_file
            .read_to_end(&mut wasm_buffer)
            .map_err(|err| ValidationError::MiscCannotRead {
                file: path_str.clone(),
                error: format!("{}", err),
            })?;

        validate_wasm_and_report_errors(&wasm_buffer, path_str)?;
    }

    Ok(())
}

pub fn validate_wasm_and_report_errors(
    wasm: &[u8],
    file_name: String,
) -> Result<(), failure::Error> {
    use wasmparser::WasmDecoder;
    let mut parser = wasmparser::ValidatingParser::new(wasm, None);
    loop {
        let state = parser.read();
        match *state {
            wasmparser::ParserState::EndWasm => break Ok(()),
            wasmparser::ParserState::Error(e) => {
                break Err(ValidationError::InvalidWasm {
                    file: file_name,
                    error: format!("{}", e),
                }
                .into())
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
    #[fail(display = "Failed to unpack archive \"{}\"! {}", file, error)]
    CannotUnpackArchive { file: String, error: String },
}
