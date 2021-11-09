use crate::validate::*;
use crate::util::create_temp_dir;
use flate2::read::GzDecoder;
use std::{fs, io::Read, path::PathBuf};
use structopt::StructOpt;
use tar::Archive;

#[derive(StructOpt, Debug)]
pub struct ValidateOpt {
    /// Directory or tar file to validate
    package: String,
}

pub fn validate(validate_opts: ValidateOpt) -> anyhow::Result<()> {
    let pkg_path = PathBuf::from(&validate_opts.package);
    validate_manifest_and_modules(pkg_path)
}

pub fn validate_manifest_and_modules(pkg_path: PathBuf) -> anyhow::Result<()> {
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
        gz.read_to_end(&mut archive_data)
            .map_err(|e| anyhow!("Failed to read archive data: {}", e.to_string()))?;

        let temp_out_dir = create_temp_dir()
            .map_err(|e| anyhow!("Could not create temporary directory: {}", e.to_string()))?;
        let out_dir = temp_out_dir.clone();
        let mut archive = Archive::new(archive_data.as_slice());
        // TODO: consider doing this entirely in memory with multiple passes
        archive
            .unpack(&out_dir)
            .map_err(|err| ValidationError::CannotUnpackArchive {
                file: pkg_path.to_string_lossy().to_string(),
                error: format!("{}", err),
            })?;

        let archive_path = {
            let mut ar_path = out_dir.to_path_buf();
            let archive_folder_name = pkg_path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .ok_or_else(|| anyhow!("Failed to get archive folder name"))?
                .replace(".tar.gz", "");
            ar_path.push(archive_folder_name);
            ar_path
        };

        validate_directory(archive_path)
    }
}
