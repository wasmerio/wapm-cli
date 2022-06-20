//! The Manifest file is where the core metadata of a wapm package lives

use semver::Version;
use std::collections::hash_map::HashMap;
use std::{fs, fmt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use serde_derive::{Serialize, Deserialize};

/// The ABI is a hint to WebAssembly runtimes about what additional imports to insert.
/// It currently is only used for validation (in the validation subcommand).  The default value is `None`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub enum Abi {
    #[serde(rename = "emscripten")]
    Emscripten,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "wasi")]
    Wasi,
}

impl Abi {
    pub fn to_str(&self) -> &str {
        match self {
            Abi::Emscripten => "emscripten",
            Abi::Wasi => "wasi",
            Abi::None => "generic",
        }
    }
    pub fn is_none(&self) -> bool {
        return self == &Abi::None;
    }
    pub fn from_str(name: &str) -> Self {
        match name.to_lowercase().as_ref() {
            "emscripten" => Abi::Emscripten,
            "wasi" => Abi::Wasi,
            _ => Abi::None,
        }
    }
}

impl fmt::Display for Abi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl Default for Abi {
    fn default() -> Self {
        Abi::None
    }
}

/// The name of the manifest file. This is hard-coded for now.
pub static MANIFEST_FILE_NAME: &str = "wapm.toml";
pub static PACKAGES_DIR_NAME: &str = "wapm_packages";

pub static README_PATHS: &[&'static str;5]  = &[
    "README",
    "README.md",
    "README.markdown",
    "README.mdown",
    "README.mkdn",
];

pub static LICENSE_PATHS: &[&'static str;3] = &[
    "LICENSE", 
    "LICENSE.md", 
    "COPYING"
];

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
    #[serde(
        rename = "disable-command-rename",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub disable_command_rename: bool,
    /// Unlike, `disable-command-rename` which prevents `wapm run <Module name>`,
    /// this flag enables the command rename of `wapm run <COMMAND_NAME>` into
    /// just `<COMMAND_NAME>. This is useful for programs that need to inspect
    /// their argv[0] names and when the command name matches their executable name.
    #[serde(
        rename = "rename-commands-to-raw-command-name",
        default,
        skip_serializing_if = "std::ops::Not::not"
    )]
    pub rename_commands_to_raw_command_name: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Command {
    V1(CommandV1),
    V2(CommandV2),
}

impl Command {

    pub fn get_name(&self) -> String {
        match self {
            Self::V1(c) => c.name.clone(),
            Self::V2(c) => c.name.clone(),
        }
    }

    pub fn get_module(&self) -> String {
        match self {
            Self::V1(c) => c.module.clone(),
            // TODO(felix): how to migrate to the new API?
            Self::V2(_) => String::new(),
        }
    }

    pub fn get_package(&self) -> Option<String> {
        match self {
            Self::V1(c) => c.package.clone(),
            // TODO(felix): how to migrate to the new version / "kind" API?
            Self::V2(_) => None,
        }
    }

    pub fn get_main_args(&self) -> Option<String> {
        match self {
            Self::V1(c) => c.main_args.clone(),
            // TODO(felix): how to migrate to the new API?
            // Self::V2(c) => serde_json::to_string(&c.annotations)
            Self::V2(_) => None,
        }
    }
}

/// Describes a command for a wapm module
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandV1 {
    pub name: String,
    pub module: String,
    pub main_args: Option<String>,
    pub package: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandV2 {
    pub name: String,
    pub runner: String,
    pub annotations: Option<CommandAnnotations>,
}

impl CommandV2 {
    pub fn get_annotations(&self, basepath: &PathBuf) -> Result<Option<serde_cbor::Value>, String> {
        match self.annotations.as_ref() {
            Some(CommandAnnotations::Raw(v)) => Ok(Some(toml_to_cbor_value(v))),
            Some(CommandAnnotations::File(FileCommandAnnotations { file, kind })) => {
                let path = basepath.join(file.clone());
                let file = std::fs::read_to_string(&path).map_err(|e| {
                    format!("Error reading {:?}.annotation ({:?}): {e}", self.name, path.display())
                })?;
                match kind {
                    FileKind::Json => {
                        let value: serde_json::Value = serde_json::from_str(&file)
                        .map_err(|e| {
                            format!("Error reading {:?}.annotation ({:?}): {e}", self.name, path.display())
                        })?;
                        Ok(Some(json_to_cbor_value(&value)))
                    },
                    FileKind::Yaml => {
                        let value: serde_yaml::Value = serde_yaml::from_str(&file)
                        .map_err(|e| {
                            format!("Error reading {:?}.annotation ({:?}): {e}", self.name, path.display())
                        })?;
                        Ok(Some(yaml_to_cbor_value(&value)))
                    }
                }
            },
            None => Ok(None),
        }
    }
}

pub fn toml_to_cbor_value(val: &toml::Value) -> serde_cbor::Value {
    match val {
        toml::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        toml::Value::Integer(i) => serde_cbor::Value::Integer(*i as i128),
        toml::Value::Float(f) => serde_cbor::Value::Float(*f),
        toml::Value::Boolean(b) => serde_cbor::Value::Bool(*b),
        toml::Value::Datetime(d) => serde_cbor::Value::Text(format!("{}", d)),
        toml::Value::Array(sq) => serde_cbor::Value::Array(
            sq.into_iter().map(toml_to_cbor_value).collect()
        ),
        toml::Value::Table(m) => {
            serde_cbor::Value::Map(m.iter().map(|(k, v)| {
                (serde_cbor::Value::Text(k.clone()), toml_to_cbor_value(v))
            }).collect())
        },
    }
}

pub fn json_to_cbor_value(val: &serde_json::Value) -> serde_cbor::Value {
    match val {
        serde_json::Value::Null => serde_cbor::Value::Null,
        serde_json::Value::Bool(b) => serde_cbor::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_cbor::Value::Integer(i as i128)
            } else if let Some(u) = n.as_u64() {
                serde_cbor::Value::Integer(u as i128)
            } else if let Some(f) = n.as_f64() {
                serde_cbor::Value::Float(f as f64)
            } else {
                serde_cbor::Value::Null
            }
        },
        serde_json::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        serde_json::Value::Array(sq) => serde_cbor::Value::Array(
            sq.into_iter().map(json_to_cbor_value).collect()
        ),
        serde_json::Value::Object(m) => {
            serde_cbor::Value::Map(m.iter().map(|(k, v)| {
                (serde_cbor::Value::Text(k.clone()), json_to_cbor_value(v))
            }).collect())
        },
    }
}

pub fn yaml_to_cbor_value(val: &serde_yaml::Value) -> serde_cbor::Value {
    match val {
        serde_yaml::Value::Null => serde_cbor::Value::Null,
        serde_yaml::Value::Bool(b) => serde_cbor::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_cbor::Value::Integer(i as i128)
            } else if let Some(u) = n.as_u64() {
                serde_cbor::Value::Integer(u as i128)
            } else if let Some(f) = n.as_f64() {
                serde_cbor::Value::Float(f as f64)
            } else {
                serde_cbor::Value::Null
            }
        },
        serde_yaml::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        serde_yaml::Value::Sequence(sq) => serde_cbor::Value::Array(
            sq.into_iter().map(yaml_to_cbor_value).collect()
        ),
        serde_yaml::Value::Mapping(m) => {
            serde_cbor::Value::Map(m.iter().map(|(k, v)| {
                (yaml_to_cbor_value(k), yaml_to_cbor_value(v))
            }).collect())
        },
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[repr(C)]
pub enum CommandAnnotations {
    File(FileCommandAnnotations),
    Raw(toml::Value),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FileCommandAnnotations {
    pub file: PathBuf,
    pub kind: FileKind,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Ord, Eq, Deserialize, Serialize)]
pub enum FileKind {
    #[serde(rename = "yaml")]
    Yaml,
    #[serde(rename = "json")]
    Json,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Module {
    pub name: String,
    pub source: PathBuf,
    #[serde(default = "Abi::default", skip_serializing_if = "Abi::is_none")]
    pub abi: Abi,
    #[serde(default)]
    pub kind: Option<String>,
    #[cfg(feature = "package")]
    pub fs: Option<Table>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<HashMap<String, String>>,
}

/// The manifest represents the file used to describe a Wasm package.
///
/// The `module` field represents the wasm file to be published.
///
/// The `source` is used to create bundles with the `fs` section.
///
/// The `fs` section represents fs assets that will be made available to the
/// program relative to its starting current directory (there may be issues with WASI).
/// These are pairs of paths.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    pub package: Package,
    pub dependencies: Option<HashMap<String, String>>,
    pub module: Option<Vec<Module>>,
    pub command: Option<Vec<Command>>,
    /// Of the form Guest -> Host path
    pub fs: Option<HashMap<String, PathBuf>>,
    /// private data
    /// store the directory path of the manifest file for use later accessing relative path fields
    #[serde(skip)]
    pub base_directory_path: PathBuf,
}

impl Manifest {
    fn locate_file(path: &Path, candidates: &[&str]) -> Option<PathBuf> {
        for filename in candidates {
            let path_buf = path.join(filename);
            if path_buf.exists() {
                return Some(filename.into());
            }
        }
        None
    }

    /// Construct a manifest by searching in the specified directory for a manifest file
    #[cfg(not(feature = "integration_tests"))]
    pub fn find_in_directory<T: AsRef<Path>>(path: T) -> Result<Self, ManifestError> {
        if !path.as_ref().is_dir() {
            return Err(ManifestError::MissingManifest(
                path.as_ref().to_string_lossy().to_string(),
            ));
        }
        let manifest_path_buf = path.as_ref().join(MANIFEST_FILE_NAME);
        let contents = fs::read_to_string(&manifest_path_buf).map_err(|_e| {
            ManifestError::MissingManifest(manifest_path_buf.to_string_lossy().to_string())
        })?;
        let mut manifest: Self = toml::from_str(contents.as_str())
            .map_err(|e| ManifestError::TomlParseError(e.to_string()))?;
        if manifest.package.readme.is_none() {
            manifest.package.readme = Self::locate_file(
                path.as_ref(),
                &README_PATHS[..],
            );
        }
        if manifest.package.license_file.is_none() {
            manifest.package.license_file =
                Self::locate_file(path.as_ref(), &LICENSE_PATHS[..]);
        }
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        let module_map = self
            .module
            .as_ref()
            .map(|modules| {
                modules
                    .iter()
                    .map(|module| (module.name.clone(), module.clone()))
                    .collect::<HashMap<String, Module>>()
            })
            .unwrap_or_default();

        if let Some(ref commands) = self.command {
            for command in commands {
                if let Some(ref module) = module_map.get(&command.get_module()) {
                    if module.abi == Abi::None {
                        return Err(ManifestError::ValidationError(ValidationError::MissingABI(
                            command.get_name(),
                            module.name.clone(),
                        )));
                    }
                } else {
                    return Err(ManifestError::ValidationError(
                        ValidationError::MissingModuleForCommand(
                            command.get_name(),
                            command.get_module(),
                        ),
                    ));
                }
            }
        }
        Ok(())
    }

    /// add a dependency
    pub fn add_dependency(&mut self, dependency_name: String, dependency_version: String) {
        let dependencies = self.dependencies.get_or_insert(Default::default());
        dependencies.insert(dependency_name, dependency_version);
    }

    /// remove dependency by package name
    pub fn remove_dependency(&mut self, dependency_name: &str) -> Option<String> {
        let dependencies = self.dependencies.get_or_insert(Default::default());
        dependencies.remove(dependency_name)
    }

    pub fn to_string(&self) -> anyhow::Result<String> {
        Ok(toml::to_string(self)?)
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.base_directory_path.join(MANIFEST_FILE_NAME)
    }

    /// Write the manifest to permanent storage
    #[cfg(not(feature = "integration_tests"))]
    pub fn save(&self) -> anyhow::Result<()> {
        let manifest_string = self.to_string()?;
        let manifest_path = self.manifest_path();
        fs::write(manifest_path, &manifest_string)
            .map_err(|e| ManifestError::CannotSaveManifest(e.to_string()))?;
        Ok(())
    }

    /// Mock version of `save`
    #[cfg(feature = "integration_tests")]
    pub fn save(&self) -> anyhow::Result<()> {
        let manifest_string = self.to_string()?;
        crate::integration_tests::data::RAW_MANIFEST_DATA.with(|rmd| {
            *rmd.borrow_mut() = Some(manifest_string);
        });
        Ok(())
    }

    /// Mock version of `find_in_directory`
    #[cfg(feature = "integration_tests")]
    pub fn find_in_directory<T: AsRef<Path>>(_path: T) -> Result<Self, ManifestError> {
        // ignore path for now
        crate::integration_tests::data::RAW_MANIFEST_DATA.with(|rmd| {
            if let Some(ref manifest_toml) = *rmd.borrow() {
                let manifest: Self = toml::from_str(&manifest_toml)
                    .map_err(|e| ManifestError::TomlParseError(e.to_string()))?;
                manifest.validate()?;
                Ok(manifest)
            } else {
                Err(ManifestError::MissingManifest(
                    "Integration test manifest not found".to_string(),
                ))
            }
        })
    }
}

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("Manifest file not found at {0}")]
    MissingManifest(String),
    #[error("Could not save manifest file: {0}.")]
    CannotSaveManifest(String),
    #[error("Could not parse manifest because {0}.")]
    TomlParseError(String),
    #[error("Dependency version must be a string. Package name: {0}.")]
    DependencyVersionMustBeString(String),
    #[error("Package must have version that follows semantic versioning. {0}")]
    SemVerError(String),
    #[error("There was an error validating the manifest: {0}")]
    ValidationError(ValidationError),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(
        "missing ABI field on module {0} used by command {1}; an ABI of `wasi` or `emscripten` is required",
    )]
    MissingABI(String, String),
    #[error("missing module {0} in manifest used by command {1}")]
    MissingModuleForCommand(String, String),
}

#[cfg(test)]
mod serialization_tests {
    use crate::data::manifest::Manifest;

    #[test]
    fn get_manifest() {
        let wapm_toml = toml! {
            [package]
            name = "test"
            version = "1.0.0"
            repository = "test.git"
            homepage = "test.com"
            description = "The best package."
        };
        let manifest: Manifest = wapm_toml.try_into().unwrap();
        assert_eq!(false, manifest.package.disable_command_rename);
    }
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
            interfaces = {"wasi" = "0.0.0-unstable"}
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
    use crate::util::create_temp_dir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn add_new_dependency() {
        let tmp_dir = create_temp_dir().unwrap();
        let tmp_dir_path: &std::path::Path = tmp_dir.as_ref();
        let manifest_path = tmp_dir_path.join(MANIFEST_FILE_NAME);
        let mut file = File::create(&manifest_path).unwrap();
        let wapm_toml = toml! {
            [package]
            name = "_/test"
            version = "1.0.0"
            description = "description"
            [[module]]
            name = "test"
            source = "test.wasm"
            interfaces = {}
        };
        let toml_string = toml::to_string(&wapm_toml).unwrap();
        file.write_all(toml_string.as_bytes()).unwrap();
        let mut manifest = Manifest::find_in_directory(tmp_dir).unwrap();

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
    fn interface_test() {
        let manifest_str = r#"
[package]
name = "test"
version = "0.0.0"
description = "This is a test package"
license = "MIT"

[[module]]
name = "mod"
source = "target/wasm32-wasi/release/mod.wasm"
interfaces = {"wasi" = "0.0.0-unstable"}

[[command]]
name = "command"
module = "mod"
"#;
        let manifest: Manifest = toml::from_str(manifest_str).unwrap();
        assert_eq!(
            manifest.module.unwrap()[0]
                .interfaces
                .as_ref()
                .unwrap()
                .get("wasi"),
            Some(&"0.0.0-unstable".to_string())
        )
    }
}


