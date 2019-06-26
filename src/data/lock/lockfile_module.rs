use crate::abi::Abi;
use crate::data::manifest::{Module, PACKAGES_DIR_NAME};
use crate::util::get_package_namespace_and_name;
use semver::Version;
use std::path::PathBuf;

/// legacy Lockfile module struct; which is only used to parse legacy lockfiles which get
/// transformed into up to date ones
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct LockfileModuleV2 {
    pub name: String,
    pub package_version: String,
    pub package_name: String,
    pub source: String,
    pub resolved: String,
    pub abi: Abi,
    pub entry: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct LockfileModule {
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
}

impl LockfileModule {
    fn get_root_dir(name: &str, version: &Version) -> PathBuf {
        let (namespace, unqualified_pkg_name) =
            get_package_namespace_and_name(name.as_ref()).unwrap();
        let pkg_dir = format!("{}@{}", unqualified_pkg_name, version);
        let mut path = PathBuf::new();
        path.push(PACKAGES_DIR_NAME);
        path.push(namespace);
        path.push(pkg_dir.as_str());

        path
    }

    pub fn from_module(name: &str, version: &Version, module: &Module, download_url: &str) -> Self {
        // build the entry path
        // this is path like /wapm_packages/_/lua@0.1.3/path/to/module/lua.wasm
        let mut path = Self::get_root_dir(name, version);
        let pkg_root = path.clone().to_string_lossy().to_string();

        let entry = {
            let mut new_style = path.clone();
            new_style.push(&module.source);
            if new_style.exists() {
                new_style
            } else {
                // to prevent breaking packages published before this change (~2019/06/25)
                path.push(module.source.file_name().unwrap());
                path
            }
            .to_string_lossy()
            .to_string()
        };

        let lockfile_module = LockfileModule {
            name: module.name.to_string(),
            package_version: version.to_string(),
            package_name: name.to_string(),
            source: format!("registry+{}", module.name),
            resolved: download_url.to_string(),
            abi: module.abi.clone(),
            entry,
            root: pkg_root,
        };
        lockfile_module
    }

    pub fn from_local_module(name: &str, version: &Version, module: &Module) -> Self {
        let root = Self::get_root_dir(name, version)
            .to_string_lossy()
            .to_string();

        LockfileModule {
            name: module.name.clone(),
            package_version: version.to_string(),
            package_name: name.to_string(),
            source: "local".to_string(),
            resolved: "local".to_string(),
            abi: module.abi.clone(),
            entry: module.source.to_string_lossy().to_string(),
            root,
        }
    }
}
