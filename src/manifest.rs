use crate::abi::Abi;
use std::env;
use std::fs;
use std::path::{PathBuf, Path};
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
            let target_path = self.get_absolute_path(&self.target);
            Ok(target_path)
        } else {
            Ok(self.target.clone())
        }
    }

    /// get the absolute path given a relative path
    pub fn get_absolute_path(&self, path: &Path) -> PathBuf {
        let mut base_path = self.path.parent().expect("Can't use the root dir / as your manifest file").to_path_buf();
        let abs_path = base_path.join(path);
        abs_path
    }

    /// get the source absolute path
    pub fn source_absolute_path(&self) -> Result<Source, failure::Error> {
        let path = self.get_absolute_path(&self.source);
        dunce::canonicalize(&path).map_err(|e| e.into())
    }

    // init from file path
    pub fn new_from_path(cli_manifest_path: Option<PathBuf>) -> Result<Self, failure::Error> {
        let manifest_path_buf = get_absolute_manifest_path(cli_manifest_path)?;
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
    use crate::manifest::{get_absolute_manifest_path, Manifest, MANIFEST_FILE_NAME};
    use std::fs;
    use std::fs::File;
    use std::io::Write;

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

    #[test]
    fn target_and_source_paths() {
        let tmp_dir = tempdir::TempDir::new("target_and_source_paths").unwrap();
        let manifest_absolute_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_absolute_path).unwrap();
        file.write_all(
            r#"
name = "test"
version = "1.0.0"
target = "target.wasm"
source = "source.wasm"
description = "description"
        "#
            .as_bytes(),
        );

        let source_wasm_path = tmp_dir.path().join("source.wasm");
        let _ = File::create(&source_wasm_path).unwrap();

        let manifest = Manifest::new_from_path(Some(manifest_absolute_path)).unwrap();
        let expected_source_path = source_wasm_path;

        let actual_source_path = manifest.source_absolute_path().unwrap();
        assert_eq!(actual_source_path, expected_source_path);

        let expected_target_path = tmp_dir.path().join("target.wasm");
        let actual_target_path = manifest.target_absolute_path().unwrap();
        assert_eq!(actual_target_path, expected_target_path);
    }

    #[test]
    fn nested_target_and_source_paths() {
        let tmp_dir = tempdir::TempDir::new("nested_target_and_source_paths").unwrap();
        let manifest_absolute_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_absolute_path).unwrap();
        file.write_all(
            r#"
name = "test"
version = "1.0.0"
target = "my/awesome/target.wasm"
source = "my/old/boring/source.wasm"
description = "description"
        "#
            .as_bytes(),
        );

        let target_dir = tmp_dir.path().join("my/awesome");
        fs::create_dir_all(&target_dir).unwrap();
        let source_dir = tmp_dir.path().join("my/old/boring");
        fs::create_dir_all(&source_dir).unwrap();

        let source_wasm_path = source_dir.join("source.wasm");
        let _ = File::create(&source_wasm_path).unwrap();

        let manifest = Manifest::new_from_path(Some(manifest_absolute_path)).unwrap();

        let expected_source_path = source_wasm_path;
        let actual_source_path = manifest.source_absolute_path().unwrap();
        assert_eq!(expected_source_path, actual_source_path);

        let expected_target_path = target_dir.join("target.wasm");
        let actual_target_path = manifest.target_absolute_path().unwrap();
        assert_eq!(expected_target_path, actual_target_path);
    }

    #[test]
    fn relative_target_path() {
        let tmp_dir = tempdir::TempDir::new("nested_target_and_source_paths").unwrap();
        let manifest_absolute_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_absolute_path).unwrap();
        file.write_all(
            r#"
name = "test"
version = "1.0.0"
target = "../../target.wasm"
source = "source.wasm"
description = "description"
        "#
            .as_bytes(),
        );

        let source_wasm_path = tmp_dir.path().join("source.wasm");
        let _ = File::create(&source_wasm_path).unwrap();

        let manifest = Manifest::new_from_path(Some(manifest_absolute_path)).unwrap();

        let expected_target_path = tmp_dir.path().join("../../target.wasm");
        let actual_target_path = manifest.target_absolute_path().unwrap();
        assert_eq!(actual_target_path, expected_target_path);
    }
}
