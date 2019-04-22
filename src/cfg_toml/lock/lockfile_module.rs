use crate::abi::Abi;
use crate::cfg_toml::manifest::{Module, PACKAGES_DIR_NAME};
use crate::dependency_resolver::Dependency;
use crate::util::get_package_namespace_and_name;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileModule<'a> {
    pub name: &'a str,
    pub package_version: &'a str,
    pub package_name: &'a str,
    pub source: String,
    pub resolved: String,
    pub abi: Abi,
    pub entry: String,
}

impl<'a> LockfileModule<'a> {
    pub fn from_module(
        name: &'a str,
        version: &'a str,
        module: &'a Module,
        download_url: &'a str,
    ) -> Self {
        // build the entry path
        // this is path like /wapm_packages/_/lua@0.1.3/lua.wasm
        let (namespace, unqualified_pkg_name) = get_package_namespace_and_name(name).unwrap();
        let pkg_dir = format!("{}@{}", unqualified_pkg_name, version);
        let mut path = PathBuf::new();
        path.push(PACKAGES_DIR_NAME);
        path.push(namespace);
        path.push(pkg_dir.as_str());
        path.push(module.source.file_name().unwrap());
        let entry = path.to_string_lossy().to_string();

        let lockfile_module = LockfileModule {
            name: module.name.as_str(),
            package_version: version,
            package_name: name,
            source: format!("registry+{}", module.name),
            resolved: download_url.to_string(),
            abi: module.abi.clone(),
            entry,
        };
        lockfile_module
    }

    pub fn from_dependency(
        dependency: &'a Dependency,
    ) -> Result<(Vec<LockfileModule<'a>>), failure::Error> {
        if let None = dependency.manifest.module {
            return Ok(vec![]);
        }

        let modules = dependency.manifest.module.as_ref().unwrap();

        let package_name = dependency.manifest.package.name.as_str();
        let package_version = dependency.manifest.package.version.as_str();
        let download_url = dependency.download_url.as_str();

        let lockfile_modules: Vec<LockfileModule> = modules
            .iter()
            .map(|m| LockfileModule::from_module(package_name, package_version, m, download_url))
            .collect();
        Ok(lockfile_modules)
    }
}
