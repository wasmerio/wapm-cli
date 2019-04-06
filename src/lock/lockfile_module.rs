use crate::abi::Abi;
use crate::manifest::Module;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LockfileModule {
    pub name: String,
    pub version: String,
    pub source: String,
    pub resolved: String,
    pub integrity: String,
    pub hash: String,
    pub abi: Abi,
    pub entry: String,
}

impl LockfileModule {
    pub fn from_module(module: &Module, download_url: &str) -> Self {
        let lockfile_module = LockfileModule {
            name: module.name.clone(),
            version: module.version.to_string(),
            source: format!("registry+{}", module.name),
            resolved: download_url.to_string(),
            integrity: "".to_string(),
            hash: "".to_string(),
            abi: module.abi.clone(),
            entry: module
                .module
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        };
        lockfile_module
    }
}
