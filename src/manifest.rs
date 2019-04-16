use crate::abi::Abi;
use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use toml::value::Table;
use toml::Value;

/// The name of the manifest file. This is hard-coded for now.
pub static MANIFEST_FILE_NAME: &str = "wapm.toml";
pub static PACKAGES_DIR_NAME: &str = "wapm_packages";

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: String,
    pub license: Option<String>,
    pub readme: Option<PathBuf>,
    pub repository: Option<String>,
    pub homepage: Option<String>,
    #[serde(rename = "wasmer-extra-flags")]
    pub wasmer_extra_flags: Option<String>,
}

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Command {
    pub name: String,
    pub module: String,
    pub main_args: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Module {
    pub name: String,
    pub source: PathBuf,
    #[serde(default = "Abi::default")]
    pub abi: Abi,
    pub fs: Option<Table>,
}

/// The manifest represents the file used to describe a Wasm package.
///
/// The `module` field represents the wasm file to be published.
///
/// The `source` is used to create bundles with the `fs` section.
///
/// The `fs` section represents assets that will be embedded into the Wasm module as custom sections.
/// These are pairs of paths.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub package: Package,
    pub module: Option<Vec<Module>>,
    pub dependencies: Option<Table>,
    pub command: Option<Vec<Command>>,
    /// private data
    /// store the directory path of the manifest file for use later accessing relative path fields
    #[serde(skip)]
    pub base_directory_path: PathBuf,
}

impl Manifest {
    /// Construct a manifest by searching in the specified directory for a manifest file
    pub fn find_in_directory<T: AsRef<Path>>(path: T) -> Result<Self, ManifestError> {
        if !path.as_ref().is_dir() {
            return Err(ManifestError::MissingManifest);
        }
        let manifest_path_buf = path.as_ref().join(MANIFEST_FILE_NAME);
        let contents =
            fs::read_to_string(&manifest_path_buf).map_err(|_e| ManifestError::MissingManifest)?;
        let manifest: Self = toml::from_str(contents.as_str())
            .map_err(|e| ManifestError::TomlParseError(e.to_string()))?;
        Ok(manifest)
    }

    /// add a dependency
    pub fn add_dependency(&mut self, dependency_name: &str, dependency_version: &str) {
        let dependencies = self.dependencies.get_or_insert(BTreeMap::new());
        dependencies.insert(
            dependency_name.to_string(),
            Value::String(dependency_version.to_string()),
        );
    }

    pub fn save(&self) -> Result<(), failure::Error> {
        let manifest_string = toml::to_string(self)?;
        let manifest_path = self.base_directory_path.join(MANIFEST_FILE_NAME);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&manifest_path)
            .map_err(|err| ManifestError::CannotSaveManifest(format!("{}", err)))?;
        file.write_all(manifest_string.as_bytes())?;
        Ok(())
    }

    pub fn extract_dependencies(&self) -> Result<Vec<(&str, &str)>, failure::Error> {
        if self.dependencies.is_none() {
            return Ok(vec![]);
        }
        let dependencies = self.dependencies.as_ref().unwrap();
        let mut extracted_dependencies = vec![];
        for (name, version_value) in dependencies.iter() {
            match version_value {
                Value::String(version) => {
                    extracted_dependencies.push((name.as_str(), version.as_str()))
                }
                _ => {
                    return Err(
                        ManifestError::DependencyVersionMustBeString(name.to_string()).into(),
                    );
                }
            }
        }
        Ok(extracted_dependencies)
    }
}

#[derive(Debug, Fail)]
pub enum ManifestError {
    #[fail(display = "Manifest file not found.")]
    MissingManifest,
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
    #[fail(display = "Could not save manifest file: {}.", _0)]
    CannotSaveManifest(String),
    #[fail(display = "Could not parse manifest because {}.", _0)]
    TomlParseError(String),
}

#[cfg(test)]
mod command_tests {
    use crate::manifest::Manifest;

    #[test]
    fn get_commands() {
        let wapm_toml = toml! {
            [package]
            name = "test"
            version = "1.0.0"
            repository = "test.git"
            homepage = "test.com"
            description = "The best package."
            [[module]]
            name = "test-pkg"
            module = "target.wasm"
            source = "source.wasm"
            description = "description"
            [[command]]
            name = "foo"
            module = "test"
            [[command]]
            name = "baz"
            module = "test"
            main_args = "$@"
        };
        let manifest: Manifest = wapm_toml.try_into().unwrap();
        let commands = &manifest.command.unwrap();
        assert_eq!(2, commands.len());
    }
}

#[cfg(test)]
mod dependency_tests {
    use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn add_new_dependency() {
        let tmp_dir = tempdir::TempDir::new("add_new_dependency").unwrap();
        let manifest_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_path).unwrap();
        let wapm_toml = toml! {
            [package]
            name = "_/test"
            version = "1.0.0"
            description = "description"
            [[module]]
            name = "test"
            source = "test.wasm"
            abi = "none"
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let mut manifest = Manifest::find_in_directory(tmp_dir.as_ref()).unwrap();

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
