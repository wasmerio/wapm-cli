use crate::abi::Abi;
use std::env;
use std::fs;
use std::path::PathBuf;
use toml::value::Table;

/// The name of the manifest file. This is hard-coded for now.
static MANIFEST_FILE_NAME: &str = "wapm.toml";

/// The manifest represents the file used to describe a Wasm bundle. This file contains fields needed
/// to generated a wasm bundle. The important fields are `Target` and `Source` which are Paths to wasm
/// files. The target will be generated or overwritten by the bundler.
///
/// The `fs` section represents assets that will be embedded into the Wasm module as custom sections.
/// These are pairs of paths.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: Option<String>,
    pub readme: Option<PathBuf>,
    source: PathBuf,
    target: PathBuf,
    pub fs: Option<Table>,
    #[serde(default = "Abi::default")]
    pub abi: Abi,
    /// The path of the manifest file
    #[serde(skip)]
    path: PathBuf,
}

pub type Target = PathBuf;
pub type Source = PathBuf;

impl Manifest {
    /// get the target absolute path
    pub fn target_absolute_path(&self) -> Result<Target, failure::Error> {
        if self.target.is_relative() {
            let abs_path = self.path.join(&self.target);
            Ok(abs_path)
        } else {
            Ok(self.target.clone())
        }
    }

    /// get the absolute path given a relative path
    pub fn get_absolute_path(&self, path: &PathBuf) -> PathBuf {
        if let Some(base_path) = self.path.parent() {
            let abs_path = base_path.join(path.as_path());
            abs_path
        }
        else {
            path.to_path_buf()
        }
    }

    /// get the source absolute path
    pub fn source_absolute_path(&self) -> Result<Source, failure::Error> {
        let path = self.get_absolute_path(&self.source);
        dunce::canonicalize(path).map_err(|e| e.into())
    }

    // init from file path
    pub fn new_from_path(cli_manifest_path: Option<PathBuf>) -> Result<Self, failure::Error> {
        let manifest_path_buf = get_absolute_manifest_path(cli_manifest_path)?;
        let _base_manifest_path = manifest_path_buf.parent().unwrap();
        let contents = fs::read_to_string(&manifest_path_buf)?;
        let mut manifest: Self = toml::from_str(contents.as_str())?;
        manifest.path = manifest_path_buf;
        Ok(manifest)
    }
}

/// Helper for getting the absolute path to a wasmer.toml from an optional PathBuf. The path could
/// be absolute or relative. If it is None, we create a PathBuf based in the current directory.
pub fn get_absolute_manifest_path(
    cli_manifest_path: Option<PathBuf>,
) -> Result<PathBuf, failure::Error> {
    let absolute_manifest_path = match cli_manifest_path {
        // path supplied on command-line
        Some(cli_manifest_path) => {
            // get the absolute path
            dunce::canonicalize(&cli_manifest_path).map_err(|_e| ManifestError::MissingManifest)?
        }
        // no path supplied, look in current directory
        None => {
            let cwd = env::current_dir()?;
            let absolute_manifest_path = cwd.join(MANIFEST_FILE_NAME);
            absolute_manifest_path
                .metadata()
                .map_err(|_e| ManifestError::MissingManifestInCwd)?;
            absolute_manifest_path
        }
    };

    Ok(absolute_manifest_path)
}

#[derive(Debug, Fail)]
pub enum ManifestError {
    #[fail(display = "Manifest file not found.")]
    MissingManifest,
    #[fail(display = "Manifest file not found in current directory.")]
    MissingManifestInCwd,
    #[fail(
        display = "Manifest target doesn't  ({:?}). Did you forgot to run `wapm bundle`?",
        path
    )]
    #[allow(dead_code)]
    MissingTarget { path: PathBuf },
}

#[cfg(test)]
mod test {
    use crate::manifest::{get_absolute_manifest_path, MANIFEST_FILE_NAME};
    use std::fs::File;

    #[test]
    fn manifest_in_local_directory() {
        let tmp_dir = tempdir::TempDir::new("manifest_in_local_directory").unwrap();
        let manifest_absolute_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let _ = File::create(manifest_absolute_path.clone()).unwrap();
        let manifest_path = Some(manifest_absolute_path.clone());
        let result = get_absolute_manifest_path(manifest_path);
        assert!(result.is_ok(), "Failed to get manifest path");
        let actual_manifest_path = result.unwrap();
        let expected_manifest_path = manifest_absolute_path;
        assert_eq!(
            expected_manifest_path, actual_manifest_path,
            "Manifest paths do not match."
        );
    }
}
