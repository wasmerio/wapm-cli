use std::path::Path;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;

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
            Ok(o) =>{
                if o.starts_with("wasmer run") {
                    Self::Ok(o)
                } else {
                    Self::Error(IoError::new(IoErrorKind::Other, format!("Command {command:?} does not start with \"wasmer run\"")))
                }
            },
            Err(e) => Self::Error(e),
        }
    }
}