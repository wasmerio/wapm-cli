use crate::abi::Abi;
use crate::data::manifest::{Module, PACKAGES_DIR_NAME};
use crate::util;
use semver::Version;
use std::path::{Path, PathBuf};

/// legacy Lockfile module struct; which is only used to parse legacy lockfiles which get
/// transformed into up to date ones (V1, V2)
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LockfileModuleV2 {
    pub name: String,
    pub package_version: String,
    pub package_name: String,
    pub source: String,
    pub resolved: String,
    pub abi: Abi,
    pub entry: String,
}

/// legacy Lockfile module struct; which is only used to parse legacy lockfiles which get
/// transformed into up to date ones (V3)
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LockfileModuleV3 {
    pub name: String,
    pub package_version: String,
    pub package_name: String,
    pub source: String,
    pub resolved: String,
    pub abi: Abi,
    /// The entry is where the wasm module lives
    pub entry: String,
    /// The root is where the manifest lives
    pub root: String,
    /// The hash of the wasm module cached here for faster startup time
    pub prehashed_module_key: Option<String>,
}

/// The latest Lockfile module struct (V4)
/// It contains data relating to the Wasm module itself
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct LockfileModule {
    pub name: String,
    pub package_version: String,
    pub package_name: String,
    pub package_path: String,
    pub resolved: String,
    pub resolved_source: String,
    pub abi: Abi,
    /// The source path is where the wasm module lives
    pub source: String,
    /// The hash of the wasm module cached here for faster startup time
    pub prehashed_module_key: Option<String>,
}

pub type LockfileModuleV4 = LockfileModule;

impl LockfileModule {
    pub fn from_module(
        manifest_base_dir_path: &Path,
        name: &str,
        version: &Version,
        module: &Module,
        download_url: &str,
    ) -> Self {
        // build the entry path
        // this is path like /wapm_packages/_/lua@0.1.3/path/to/module/lua.wasm
        let path = PathBuf::from(manifest_base_dir_path);

        let source = {
            let mut new_style = path.clone();
            new_style.push(&module.source);
            if new_style.exists() {
                module.source.to_string_lossy().to_string()
            } else {
                // to prevent breaking packages published before this change (~2019/06/25)
                module
                    .source
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            }
        };

        LockfileModule {
            name: module.name.clone(),
            package_version: version.to_string(),
            package_name: name.to_string(),
            package_path: format!("{}@{}", name, version),
            resolved: download_url.to_string(),
            resolved_source: format!("registry+{}", module.name),
            abi: module.abi,
            prehashed_module_key: util::get_hashed_module_key(&path.join(&source)),
            source,
        }
    }

    pub fn from_local_module(
        manifest_base_dir_path: &Path,
        name: &str,
        version: &Version,
        module: &Module,
    ) -> Self {
        let mut wasm_module_full_path = PathBuf::from(manifest_base_dir_path);
        wasm_module_full_path.push(&module.source);

        LockfileModule {
            name: module.name.clone(),
            package_version: version.to_string(),
            package_name: name.to_string(),
            package_path: format!("{}@{}", name, version),
            resolved: "local".to_string(),
            resolved_source: "local".to_string(),
            abi: module.abi,
            source: module.source.to_string_lossy().to_string(),
            prehashed_module_key: util::get_hashed_module_key(&wasm_module_full_path),
        }
    }

    /// Returns the full, absolute path to the WASM module
    pub fn get_canonical_source_path_from_lockfile_dir(
        &self,
        mut lockfile_dir: PathBuf,
    ) -> PathBuf {
        lockfile_dir.push(PACKAGES_DIR_NAME);
        lockfile_dir.push(&self.package_path);
        lockfile_dir.push(&self.source);

        lockfile_dir
    }

    /// Returns the Manifest path from the lockfile
    ///
    /// This method does extra logic to detect if the lockfile is global and adjusts accordingly.
    ///
    /// The `local_dep` flag should be passed when its known that the manifest we want to access
    /// is not in the current directory and that we need to add `wapm_packages/...` to it.
    pub fn get_canonical_manifest_path_from_lockfile_dir(
        &self,
        mut lockfile_dir: PathBuf,
        local_dep: bool,
    ) -> PathBuf {
        if crate::config::Config::get_globals_directory().expect("Could not get globals direcotry")
            == lockfile_dir
            || local_dep
        {
            lockfile_dir.push(PACKAGES_DIR_NAME);
            lockfile_dir.push(&self.package_path);

            lockfile_dir
        } else {
            lockfile_dir
        }
    }
}
