//! The Manifest file is where the core metadata of a wapm package lives

use semver::Version;
use serde_derive::{Deserialize, Serialize};
use std::collections::hash_map::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use thiserror::Error;
use indexmap::IndexMap;
use std::collections::BTreeMap;

/// The ABI is a hint to WebAssembly runtimes about what additional imports to insert.
/// It currently is only used for validation (in the validation subcommand).  The default value is `None`.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Abi {
    #[serde(rename = "emscripten")]
    Emscripten,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "wasi")]
    Wasi,
    #[serde(rename = "wasm4")]
    WASM4,
}

impl Abi {
    pub fn to_str(&self) -> &str {
        match self {
            Abi::Emscripten => "emscripten",
            Abi::Wasi => "wasi",
            Abi::WASM4 => "wasm4",
            Abi::None => "generic",
        }
    }
    pub fn is_none(&self) -> bool {
        self == &Abi::None
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

pub fn get_dependencies(wapm: &str) -> Vec<(String, String)>{
    let wapm: Manifest = match toml::from_str(wapm) {
        Ok(o) => o,
        Err(_) => { return Vec::new(); }, 
    };
    let mut dependencies = wapm.dependencies
    .clone().unwrap_or_default()
    .iter().map(|(k, v)| (k.clone(), v.clone()))
    .collect::<Vec<_>>();

    #[cfg(not(target_os = "wasi"))] {
        let current_registry = wapm_resolve_url::get_current_wapm_registry();

        for (k, _) in dependencies.iter_mut() {
            if k.split("/").count() == 1 {
                // Somebody only specified the package / command as the dependency instead
                // of using the owner/package format
                if let Some(r) = current_registry.as_ref() {
                    let package_info = wapm_resolve_url::get_tar_gz_url_of_package(r, k, None);
                    if let Some(pi) = package_info {
                        *k = pi.resolved_name.clone();
                    }
                }
            }
        }    
    }
    
    dependencies
}

pub fn get_wapm_atom_file_paths(
    paths: &BTreeMap<&PathBuf, &Vec<u8>>
) -> Result<Vec<(String, PathBuf)>, anyhow::Error> {
    
    println!("searching for {MANIFEST_FILE_NAME:?} in {:#?}", paths.keys().collect::<Vec<_>>());

    let wapm_toml = paths.get(&Path::new(MANIFEST_FILE_NAME).to_path_buf())
    .ok_or(anyhow::anyhow!("Could not find wapm.toml in FileMap"))?;

    let wapm_toml: Manifest = toml::from_slice(&wapm_toml)
    .map_err(|e| anyhow::anyhow!("Could not parse wapm.toml: {e}"))?;

    Ok(wapm_toml.module.clone().unwrap_or_default().into_iter().map(|m| {
        (m.name.clone(), Path::new(&m.source).to_path_buf())
    }).collect())
}

pub fn get_wapm_manifest_file_name() -> PathBuf {
    Path::new(MANIFEST_FILE_NAME).to_path_buf()
}

pub static README_PATHS: &[&str; 5] = &[
    "README",
    "README.md",
    "README.markdown",
    "README.mdown",
    "README.mkdn",
];

pub static LICENSE_PATHS: &[&str; 3] = &["LICENSE", "LICENSE.md", "COPYING"];

pub fn get_modules(wapm: &str) -> Vec<(String, String, String)> {
    let wapm: Manifest = match toml::from_str(wapm) {
        Ok(o) => o,
        Err(_) => { return Vec::new(); }, 
    };
    wapm.module.clone().unwrap_or_default().iter()
    .map(|m| (
        m.name.to_string(), 
        m.abi.to_string(),
        m.kind.as_ref().map(|s| s.as_str()).unwrap_or("wasm").to_string(),
    )).collect()
}

pub fn get_package_annotations(wapm: &str) -> serde_cbor::Value {
    let wapm: Manifest = match toml::from_str(wapm) {
        Ok(o) => o,
        Err(_) => { return serde_cbor::Value::Null; }, 
    };
    transform_package_meta_to_annotations(&wapm.package)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PathBufWithVolume {
    pub volume: String,
    pub path: PathBuf,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub struct InternalPackageMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(
        rename = "license-file",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub license_file: Option<PathBufWithVolume>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readme: Option<PathBufWithVolume>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

fn transform_package_meta_to_annotations(package: &Package) -> serde_cbor::Value {
    let internal_package = InternalPackageMeta {
        name: package.name.clone(),
        version: format!("{}", package.version),
        description: package.description.clone(),
        license: package.license.clone(),
        license_file: package.license_file.as_ref().map(|path| PathBufWithVolume {
            volume: format!("metadata"),
            path: path.clone(),
        }),
        readme: package.readme.as_ref().map(|path| PathBufWithVolume {
            volume: format!("metadata"),
            path: path.clone(),
        }),
        repository: package.repository.clone(),
        homepage: package.homepage.clone(),
    };

    // convert InternalPackageMeta to a serde_cbor::Value
    serde_cbor::to_vec(&internal_package)
        .ok()
        .and_then(|s| serde_cbor::from_slice(&s).ok())
        .unwrap_or(serde_cbor::Value::Null)
}

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
    /// just `<COMMAND_NAME>`. This is useful for programs that need to inspect
    /// their `argv[0]` names and when the command name matches their executable
    /// name.
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
    pub module: Option<String>,
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
                    format!(
                        "Error reading {:?}.annotation ({:?}): {e}",
                        self.name,
                        path.display()
                    )
                })?;
                match kind {
                    FileKind::Json => {
                        let value: serde_json::Value =
                            serde_json::from_str(&file).map_err(|e| {
                                format!(
                                    "Error reading {:?}.annotation ({:?}): {e}",
                                    self.name,
                                    path.display()
                                )
                            })?;
                        Ok(Some(json_to_cbor_value(&value)))
                    }
                    FileKind::Yaml => {
                        let value: serde_yaml::Value =
                            serde_yaml::from_str(&file).map_err(|e| {
                                format!(
                                    "Error reading {:?}.annotation ({:?}): {e}",
                                    self.name,
                                    path.display()
                                )
                            })?;
                        Ok(Some(yaml_to_cbor_value(&value)))
                    }
                }
            }
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
        toml::Value::Array(sq) => {
            serde_cbor::Value::Array(sq.iter().map(toml_to_cbor_value).collect())
        }
        toml::Value::Table(m) => serde_cbor::Value::Map(
            m.iter()
                .map(|(k, v)| (serde_cbor::Value::Text(k.clone()), toml_to_cbor_value(v)))
                .collect(),
        ),
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
        }
        serde_json::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        serde_json::Value::Array(sq) => {
            serde_cbor::Value::Array(sq.iter().map(json_to_cbor_value).collect())
        }
        serde_json::Value::Object(m) => serde_cbor::Value::Map(
            m.iter()
                .map(|(k, v)| (serde_cbor::Value::Text(k.clone()), json_to_cbor_value(v)))
                .collect(),
        ),
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
        }
        serde_yaml::Value::String(s) => serde_cbor::Value::Text(s.clone()),
        serde_yaml::Value::Sequence(sq) => {
            serde_cbor::Value::Array(sq.iter().map(yaml_to_cbor_value).collect())
        }
        serde_yaml::Value::Mapping(m) => serde_cbor::Value::Map(
            m.iter()
                .map(|(k, v)| (yaml_to_cbor_value(k), yaml_to_cbor_value(v)))
                .collect(),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[repr(C)]
pub enum CommandAnnotations {
    File(FileCommandAnnotations),
    Raw(toml::Value),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Module {
    pub name: String,
    pub source: PathBuf,
    #[serde(default = "Abi::default", skip_serializing_if = "Abi::is_none")]
    pub abi: Abi,
    #[serde(default)]
    pub kind: Option<String>,
    #[cfg(feature = "package")]
    pub fs: Option<toml::value::Table>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<HashMap<String, String>>,
    pub bindings: Option<Bindings>,
}

/// The interface exposed by a [`Module`].
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Bindings {
    /// The `*.wit` file's location on disk.
    pub wit_exports: PathBuf,
    /// The version of the WIT format being used.
    pub wit_bindgen: Version,
}

impl Bindings {
    /// Get all `*.wit` files that make up this interface.
    ///
    /// This includes the [`Bindings::wit`] field, but also anything it may
    /// recursively depend on.
    pub fn referenced_files(&self, _base_directory: &Path) -> Vec<PathBuf> {
        // TODO: Parse `self.wit` to find any `*.wit` files we might
        // transitively depend on and resolve them relative to self.wit's
        // parent directory.
        //
        // For now, any `*.wit` files that import other files will error out
        // further down the track.
        vec![self.wit_exports.clone()]
    }
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


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitBindingsExtended {
    pub wit: WitBindings,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WitBindings {
    pub exports: String,
    pub module: String,
}

pub type WebcBinding = (String, String, serde_cbor::Value);

pub fn get_bindings(
    wapm: &str, 
    _base_path: &PathBuf, 
    _atom_kinds: &BTreeMap<String, String>
) -> Result<Vec<WebcBinding>, anyhow::Error> {

    let wapm: Manifest = toml::from_str(wapm)?;
    let default_modules = Vec::new();
    let mut bindings = Vec::new();

    for module in wapm.module.as_ref().unwrap_or(&default_modules).iter() {
        if let Some(b) = module.bindings.as_ref() {
            let value = serde_cbor::from_slice(&serde_cbor::to_vec(&WitBindingsExtended {
                wit: WitBindings { 
                    exports: format!("metadata://{}", b.wit.display()), 
                    module: format!("atoms://{}", module.name), 
                }
            })?)?;
            bindings.push(("library-bindings".to_string(), format!("wit@{}", b.wit_bindgen), value));
        }
    }

    Ok(bindings)
}

// command name => (runner, annotations)
pub type WebcCommand = (String, Vec<(String, serde_cbor::Value)>);

pub fn get_commands(
    wapm: &str, 
    base_path: &PathBuf, 
    atom_kinds: &BTreeMap<String, String>
) -> Result<Vec<(String, WebcCommand)>, anyhow::Error> {

    let wapm: Manifest = toml::from_str(wapm)?;
    let default_commands = Vec::new();
    let mut commands = Vec::new();
    
    for command in wapm.command.as_ref().unwrap_or(&default_commands).iter() {
        match command {
            Command::V1(command) => {
                let name = &command.name;
                let module = &command.module;
                let main_args = command.main_args.as_ref();
                let package = command.package.as_ref();

                if commands.iter().any(|(k, _)| k == name) {
                    return Err(anyhow::anyhow!("Command {name} is defined more than once"));
                }

                let abi = atom_kinds.get(module).map(|s| s.as_str());
                let runner = match abi {
                    Some("emscripten") => "https://webc.org/runner/emscripten/command@unstable_",
                    Some("wasm4") => "https://webc.org/runner/wasm4/command@unstable_",
                    Some("wasi") | Some("generic") => "https://webc.org/runner/wasi/command@unstable_",
                    _ => { return Err(anyhow::anyhow!("Unknown ABI in command {name:?}: {:?}", abi.unwrap_or(""))); },
                };

                let annotations_str = match abi {
                    Some("emscripten") => "emscripten",
                    Some("wasm4") => "wasm4",
                    Some("wasi") | Some("generic") => "wasi",
                    _ => { return Err(anyhow::anyhow!("Unknown ABI in command {name:?}: {:?}", abi.unwrap_or(""))); },
                };

                let runner = runner.to_string();
                let annotations = {
                    let mut map = Vec::new();
                    map.push((
                        annotations_str.to_string(),
                        transform_cmd_args(&TransformCmdArgs {
                            atom: module.clone(),
                            main_args: main_args.cloned(),
                            package: package.cloned(),
                        }),
                    ));
                    map
                };

                commands.push((
                    name.clone(),
                    (runner, annotations)
                ));
            },
            Command::V2(command) => {

                let runner = if validator::validate_url(&command.runner) {
                    command.runner.to_string()
                } else {
                    format!("https://webc.org/runner/{}", command.runner.to_string())
                };

                let annotations = {
                    let mut map = Vec::new();

                    let annotations = command
                        .get_annotations(base_path)
                        .map_err(|e| anyhow::anyhow!("command {}: {e}", command.name))?;

                    if let Some(s) = annotations {
                        map.push((command.runner.clone(), s));
                    }
                    map
                };

                commands.push((
                    command.name.clone(),
                    (runner, annotations),
                ));
            }
        }
    }

    Ok(commands)
}

pub fn get_manifest_file_names() -> Vec<PathBuf> {
    vec![Path::new(MANIFEST_FILE_NAME).to_path_buf()]
}

pub fn get_metadata_paths(bindings: &[serde_cbor::Value]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    for b in bindings {
        if let Ok(wit) = serde_cbor::from_slice::<WitBindingsExtended>(&serde_cbor::to_vec(b).unwrap()) {
            paths.push(Path::new(&wit.wit.exports.replacen("metadata://", "", 1)).to_path_buf());
        }
    }

    for p in README_PATHS.iter() {
        paths.push(Path::new(p).to_path_buf());
    }
    for p in LICENSE_PATHS.iter() {
        paths.push(Path::new(p).to_path_buf());
    }
    paths
}

#[derive(Serialize, Deserialize)]
struct TransformCmdArgs {
    atom: String,
    main_args: Option<String>,
    package: Option<String>,
}

fn transform_cmd_args(args: &TransformCmdArgs) -> serde_cbor::Value {
    serde_cbor::to_vec(&args)
        .ok()
        .and_then(|s| serde_cbor::from_slice(&s).ok())
        .unwrap_or(serde_cbor::Value::Null)
}

#[cfg(feature = "integration_tests")]
pub mod integration_tests {
    pub mod data {
        //! Global data definitions used for testing

        use std::cell::RefCell;
        use std::thread_local;

        thread_local! {
            /// The string is the contents of the manifest, the Option is whether or not the manifest exists.
            /// Used to mock reading and writing the manifest to the file system.
            // for now we just have one manifest, a more complex implementation may be useful later
            pub static RAW_MANIFEST_DATA: RefCell<Option<String>> = RefCell::new(None);

            /// The string is the contents of the manifest, the Option is whether or not the manifest exists.
            /// Used to mock reading and writing the manifest to the file system.
            pub static RAW_CONFIG_DATA: RefCell<Option<String>> = RefCell::new(None);
        }
    }
}

impl Manifest {
    #[cfg(not(feature = "integration_tests"))]
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
        let contents = std::fs::read_to_string(&manifest_path_buf).map_err(|_e| {
            ManifestError::MissingManifest(manifest_path_buf.to_string_lossy().to_string())
        })?;
        let mut manifest: Self = toml::from_str(contents.as_str())
            .map_err(|e| ManifestError::TomlParseError(e.to_string()))?;
        if manifest.package.readme.is_none() {
            manifest.package.readme = Self::locate_file(path.as_ref(), &README_PATHS[..]);
        }
        if manifest.package.license_file.is_none() {
            manifest.package.license_file = Self::locate_file(path.as_ref(), &LICENSE_PATHS[..]);
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
                if let Some(module) = module_map.get(&command.get_module()) {
                    if module.abi == Abi::None && module.interfaces.is_none() {
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
        std::fs::write(manifest_path, &manifest_string)
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
    use super::*;
    use toml::toml;

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
    use super::*;
    use toml::toml;

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
    use super::*;
    use std::{fs::File, io::Write};
    use toml::toml;

    #[test]
    #[cfg_attr(
        feature = "integration_tests",
        ignore = "Requires the actual Manifest::find_in_directory() implementation which has been mocked out"
    )]
    fn add_new_dependency() {
        let tmp_dir = tempfile::tempdir().unwrap();
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

[[module]]
name = "mod-with-exports"
source = "target/wasm32-wasi/release/mod-with-exports.wasm"
bindings = { wit-exports = "exports.wit", wit-bindgen = "0.0.0" }

[[command]]
name = "command"
module = "mod"
"#;
        let manifest: Manifest = toml::from_str(manifest_str).unwrap();
        let modules = manifest.module.as_deref().unwrap();
        assert_eq!(
            modules[0].interfaces.as_ref().unwrap().get("wasi"),
            Some(&"0.0.0-unstable".to_string())
        );

        assert_eq!(
            modules[1],
            Module {
                name: "mod-with-exports".to_string(),
                source: PathBuf::from("target/wasm32-wasi/release/mod-with-exports.wasm"),
                abi: Abi::None,
                kind: None,
                interfaces: None,
                #[cfg(feature = "package")]
                fs: None,
                bindings: Some(Bindings {
                    wit_exports: PathBuf::from("exports.wit"),
                    wit_bindgen: "0.0.0".parse().unwrap()
                }),
            },
        );
    }
}
