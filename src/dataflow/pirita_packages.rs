use std::path::Path;
use std::io::Error as IoError;

/// A ternary for a manifest: Some, None, Error.
#[derive(Debug)]
pub enum PiritaResult {
    Ok(String),
    Error(IoError)
}

impl PiritaResult {
    pub fn find_in_directory<P: AsRef<Path>>(directory: P, command: &str) -> Self {
        let directory = directory.as_ref();
        match std::fs::read_to_string(directory.join("wapm_packages").join(".bin").join(command)) {
            Ok(o) => Self::Ok(o),
            Err(e) => Self::Error(e),
        }
    }
}