use crate::abi::Abi;
use crate::dependency_resolver::Dependency;
use crate::manifest::Module;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileModule<'a> {
    pub name: &'a str,
    pub package_version: &'a str,
    pub package_name: &'a str,
    pub source: String,
    pub resolved: String,
    pub integrity: String,
    pub hash: String,
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
        let lockfile_module = LockfileModule {
            name: module.name.as_str(),
            package_version: version,
            package_name: name,
            source: format!("registry+{}", module.name),
            resolved: download_url.to_string(),
            integrity: "".to_string(),
            hash: "".to_string(),
            abi: module.abi.clone(),
            entry: module.source.to_string_lossy().into_owned(),
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
