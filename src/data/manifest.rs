//! The Manifest file is where the core metadata of a wapm package lives
use crate::abi::Abi;
use semver::Version;
use std::collections::hash_map::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The name of the manifest file. This is hard-coded for now.
pub static MANIFEST_FILE_NAME: &str = "wapm.toml";
pub static PACKAGES_DIR_NAME: &str = "wapm_packages";

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub license: Option<String>,
    /// The location of the license file, useful for non-standard licenses
    #[serde(rename = "license-file")]
    pub license_file: Option<PathBuf>,
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
    pub package: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ContractId {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Module {
    pub name: String,
    pub source: PathBuf,
    #[serde(default = "Abi::default")]
    pub abi: Abi,
    #[cfg(feature = "package")]
    pub fs: Option<Table>,
    pub contracts: Vec<ContractId>,
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
    pub dependencies: Option<HashMap<String, String>>,
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
    pub fn add_dependency(&mut self, dependency_name: String, dependency_version: String) {
        let dependencies = self.dependencies.get_or_insert(Default::default());
        dependencies.insert(dependency_name, dependency_version);
    }

    /// remove dependency by package name
    pub fn remove_dependency(&mut self, dependency_name: String) {
        let dependencies = self.dependencies.get_or_insert(Default::default());
        dependencies.remove(&dependency_name);
    }

    pub fn save(&self) -> Result<(), failure::Error> {
        let manifest_string = toml::to_string(self)?;
        let manifest_path = self.base_directory_path.join(MANIFEST_FILE_NAME);
        fs::write(manifest_path, &manifest_string)
            .map_err(|e| ManifestError::CannotSaveManifest(e.to_string()))?;
        Ok(())
    }
}

#[derive(Debug, Fail)]
pub enum ManifestError {
    #[fail(display = "Manifest file not found.")]
    MissingManifest,
    #[fail(display = "Could not save manifest file: {}.", _0)]
    CannotSaveManifest(String),
    #[fail(display = "Could not parse manifest because {}.", _0)]
    TomlParseError(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
    #[fail(
        display = "Package must have version that follows semantic versioning. {}",
        _0
    )]
    SemVerError(String),
}

#[cfg(test)]
mod command_tests {
    use crate::data::manifest::Manifest;

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
            contracts = [{"name" = "wasi", "version" = "0.0.0-unstable"}]
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
    use crate::data::manifest::{Manifest, MANIFEST_FILE_NAME};
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
            contracts = []
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let mut manifest = Manifest::find_in_directory(tmp_dir.as_ref()).unwrap();

        let dependency_name = "dep_pkg";
        let dependency_version = semver::Version::new(0, 1, 0);

        manifest.add_dependency(dependency_name.to_string(), dependency_version.to_string());
        assert_eq!(1, manifest.dependencies.as_ref().unwrap().len());

        // adding the same dependency twice changes nothing
        manifest.add_dependency(dependency_name.to_string(), dependency_version.to_string());
        assert_eq!(1, manifest.dependencies.as_ref().unwrap().len());

        // adding a second different dependency will increase the count
        let dependency_name_2 = "dep_pkg_2";
        let dependency_version_2 = semver::Version::new(0, 2, 0);
        manifest.add_dependency(
            dependency_name_2.to_string(),
            dependency_version_2.to_string(),
        );
        assert_eq!(2, manifest.dependencies.as_ref().unwrap().len());
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::*;

    #[test]
    fn contract_test() {
        let manifest_str = r#"
[package]
name = "test"
version = "0.0.0"
description = "This is a test package"
license = "MIT"

[[module]]
name = "mod"
source = "target/wasm32-wasi/release/mod.wasm"
contracts = [{name = "wasi", version = "0.0.0-unstable"}]

[[command]]
name = "command"
module = "mod"
"#;
        let manifest: Manifest = toml::from_str(manifest_str).unwrap();
        assert_eq!(
            manifest.module.unwrap()[0].contracts,
            vec![ContractId {
                name: "wasi".to_string(),
                version: "0.0.0-unstable".to_string()
            }]
        )
    }
}
