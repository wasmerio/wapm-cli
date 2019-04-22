use crate::abi::Abi;
use crate::cfg_toml::manifest::{Module, PACKAGES_DIR_NAME};
use crate::util::get_package_namespace_and_name;
use std::borrow::Cow;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileModule<'a> {
    pub name: Cow<'a, str>,
    pub package_version: Cow<'a, str>,
    pub package_name: Cow<'a, str>,
    pub source: String,
    pub resolved: String,
    pub abi: Abi,
    pub entry: String,
}

impl<'a> LockfileModule<'a> {
    pub fn from_module(
        name: Cow<'a, str>,
        version: Cow<'a, str>,
        module: &'a Module,
        download_url: Cow<'a, str>,
    ) -> Self {
        // build the entry path
        // this is path like /wapm_packages/_/lua@0.1.3/lua.wasm
        let (namespace, unqualified_pkg_name) =
            get_package_namespace_and_name(name.as_ref()).unwrap();
        let pkg_dir = format!("{}@{}", unqualified_pkg_name, version);
        let mut path = PathBuf::new();
        path.push(PACKAGES_DIR_NAME);
        path.push(namespace);
        path.push(pkg_dir.as_str());
        path.push(module.source.file_name().unwrap());
        let entry = path.to_string_lossy().to_string();

        let lockfile_module = LockfileModule {
            name: Cow::Borrowed(module.name.as_str()),
            package_version: version,
            package_name: name,
            source: format!("registry+{}", module.name),
            resolved: download_url.to_string(),
            abi: module.abi.clone(),
            entry,
        };
        lockfile_module
    }
}
