use crate::abi::Abi;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use toml::value::Table;
use toml::Value;

/// The name of the manifest file. This is hard-coded for now.
static MANIFEST_FILE_NAME: &str = "wapm.toml";

/// The manifest represents the file used to describe a Wasm package.
///
/// The `module` field represents the wasm file to be published.
///
/// The `source` is used to create bundles with the `fs` section.
///
/// The `fs` section represents assets that will be embedded into the Wasm module as custom sections.
/// These are pairs of paths.
#[derive(Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: Option<String>,
    pub readme: Option<PathBuf>,
    source: Option<PathBuf>,
    module: PathBuf,
    pub fs: Option<Table>,
    #[serde(default = "Abi::default")]
    pub abi: Abi,
    pub dependencies: Option<Table>,

    /// private data
    /// store the directory path of the manifest file for use later accessing relative path fields
    #[serde(skip)]
    base_directory_path: PathBuf,
}

impl Manifest {
    /// Construct a manifest by searching for a manifest file with a file path
    pub fn open<P: AsRef<Path>>(manifest_file_path: P) -> Result<Self, failure::Error> {
        let contents =
            fs::read_to_string(&manifest_file_path).map_err(|_e| ManifestError::MissingManifest)?;
        let mut manifest: Self = toml::from_str(contents.as_str())?;
        let parent_directory = manifest_file_path.as_ref().parent().unwrap();
        manifest.base_directory_path = dunce::canonicalize(parent_directory)?;
        Ok(manifest)
    }

    /// Construct a manifest by searching in the current directory for a manifest file
    pub fn find_in_current_directory() -> Result<Self, failure::Error> {
        let cwd = env::current_dir()?;
        let manifest_path_buf = cwd.join(MANIFEST_FILE_NAME);
        let contents = fs::read_to_string(&manifest_path_buf)
            .map_err(|_e| ManifestError::MissingManifestInCwd)?;
        let manifest: Self = toml::from_str(contents.as_str())?;
        Ok(manifest)
    }

    /// read the contents of the readme into a string
    pub fn read_readme_to_string(&self) -> Option<String> {
        match self.readme {
            Some(ref readme_path) => {
                let readme_path = self.base_directory_path.join(readme_path);
                fs::read_to_string(&readme_path).ok()
            }
            None => None,
        }
    }

    /// get a canonical path to the wasm module
    pub fn module_path(&self) -> Result<PathBuf, failure::Error> {
        self.canonicalize_path(self.module.as_path())
    }

    /// get the source absolute path
    pub fn source_path(&self) -> Result<PathBuf, failure::Error> {
        match self.canonicalize_optional_path(&self.source) {
            Some(source_path_result) => source_path_result,
            None => Err(ManifestError::MissingSource.into()),
        }
    }

    /// add a dependency
    pub fn add_dependency(&mut self, dependency_name: &str, dependency_version: &str) {
        let dependencies = self.dependencies.get_or_insert(BTreeMap::new());
        dependencies.insert(
            dependency_name.to_string(),
            Value::String(dependency_version.to_string()),
        );
    }

    /// internal helper for canonicalizing a path that may be relative or absolute
    fn canonicalize_path<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, failure::Error> {
        if path.as_ref().is_relative() {
            let path_buf = self.base_directory_path.join(path.as_ref());
            dunce::canonicalize(&path_buf).map_err(|e| e.into())
        } else {
            Ok(path.as_ref().to_path_buf())
        }
    }

    /// internal helper for canonicalizing an optional path that may be relative or absolute
    fn canonicalize_optional_path(
        &self,
        path_buf: &Option<PathBuf>,
    ) -> Option<Result<PathBuf, failure::Error>> {
        match path_buf {
            Some(ref path_buf) => {
                let path = &**path_buf;
                Some(self.canonicalize_path(path))
            }
            None => None,
        }
    }
}

#[derive(Debug, Fail)]
pub enum ManifestError {
    #[fail(display = "Manifest file not found.")]
    MissingManifest,
    #[fail(display = "Manifest file not found in current directory.")]
    MissingManifestInCwd,
    #[fail(
        display = "Manifest target doesn't  ({:?}). Did you forgot to run `wapm package`?",
        path
    )]
    #[allow(dead_code)]
    MissingTarget { path: PathBuf },
    #[fail(display = "Source wasm file not found.")]
    MissingSource,
}

#[cfg(test)]
mod dependency_tests {
    use crate::abi::Abi;
    use crate::manifest::Manifest;
    use std::path::PathBuf;

    #[test]
    fn add_new_dependency() {
        let mut manifest = Manifest {
            name: "test_pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "description".to_string(),
            license: None,
            readme: None,
            source: None,
            module: PathBuf::new(),
            fs: None,
            abi: Abi::Emscripten,
            dependencies: None,
            base_directory_path: PathBuf::new(),
        };

        let dependency_name = "dep_pkg";
        let dependency_version = "0.1.0";

        manifest.add_dependency(dependency_name, dependency_version);
        assert_eq!(1, manifest.dependencies.as_ref().unwrap().len());

        // adding the same dependency twice changes nothing
        manifest.add_dependency(dependency_name, dependency_version);
        assert_eq!(1, manifest.dependencies.as_ref().unwrap().len());

        // adding a second different dependency will increase the count
        let dependency_name_2 = "dep_pkg_2";
        let dependency_version_2 = "0.2.0";
        manifest.add_dependency(dependency_name_2, dependency_version_2);
        assert_eq!(2, manifest.dependencies.as_ref().unwrap().len());
    }
}

#[cfg(test)]
mod module_path_tests {
    use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
    use std::fs;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn target_and_source_paths() {
        let tmp_dir = tempdir::TempDir::new("target_and_source_paths").unwrap();
        // setup the source wasm file
        let source_wasm_path = tmp_dir.path().join("source.wasm");
        File::create(&source_wasm_path).unwrap();
        // simulate the creation of the module file
        let module_wasm_path = tmp_dir.path().join("target.wasm");
        File::create(&module_wasm_path).unwrap();
        // open the manifest file
        let manifest_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_path).unwrap();
        let wapm_toml = toml! {
            name = "test"
            version = "1.0.0"
            module = "target.wasm"
            source = "source.wasm"
            description = "description"
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let manifest = Manifest::open(manifest_path).unwrap();
        // assert paths are correct
        let expected_source_path = source_wasm_path;
        let actual_source_path = manifest.source_path().unwrap();
        assert_eq!(actual_source_path, expected_source_path);
        let expected_target_path = module_wasm_path;
        let actual_target_path = manifest.module_path().unwrap();
        assert_eq!(actual_target_path, expected_target_path);
    }

    #[test]
    fn nested_target_and_source_paths() {
        let tmp_dir = tempdir::TempDir::new("nested_target_and_source_paths").unwrap();
        // setup the source wasm file
        let source_dir = tmp_dir.path().join("my/old/boring");
        let source_wasm_path = source_dir.join("source.wasm");
        fs::create_dir_all(&source_dir).unwrap();
        File::create(&source_wasm_path).unwrap();
        // simulate the creation of the module file
        let target_dir = tmp_dir.path().join("my/awesome");
        let module_wasm_path = target_dir.join("target.wasm");
        fs::create_dir_all(&target_dir).unwrap();
        File::create(&module_wasm_path).unwrap();
        // open the manifest file
        let manifest_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_path).unwrap();
        let wapm_toml = toml! {
            name = "test"
            version = "1.0.0"
            module = "my/awesome/target.wasm"
            source = "my/old/boring/source.wasm"
            description = "description"
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let manifest = Manifest::open(manifest_path).unwrap();
        // assert paths are correct
        let expected_source_path = source_wasm_path;
        let actual_source_path = manifest.source_path().unwrap();
        assert_eq!(expected_source_path, actual_source_path);
        let expected_target_path = target_dir.join("target.wasm");
        let actual_target_path = manifest.module_path().unwrap();
        assert_eq!(expected_target_path, actual_target_path);
    }

    #[test]
    fn relative_target_path() {
        let tmp_dir = tempdir::TempDir::new("nested_target_and_source_paths").unwrap();
        // setup the source wasm file
        let source_wasm_path = tmp_dir.path().join("source.wasm");
        File::create(&source_wasm_path).unwrap();
        // simulate the creation of the module file
        let module_wasm_path = tmp_dir.path().join("target.wasm");
        File::create(&module_wasm_path).unwrap();
        let manifest_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_path).unwrap();
        let wapm_toml = toml! {
            name = "test"
            version = "1.0.0"
            module = "nested/relative/../relative/../../target.wasm"
            source = "source.wasm"
            description = "description"
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let manifest = Manifest::open(manifest_path).unwrap();
        // assert
        let expected_target_path = module_wasm_path;
        let actual_target_path = manifest.module_path().unwrap();
        assert_eq!(actual_target_path, expected_target_path);
    }
}
