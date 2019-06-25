use crate::data::lock::lockfile::{Lockfile, LockfileV2};
use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::{LockfileModule, LockfileModuleV2};
use crate::dataflow::lockfile_packages::LockfileError;
use crate::dataflow::normalize_global_namespace_package_name;

use std::collections::*;

pub enum LockfileVersion {
    V1(LockfileV2),
    V2(LockfileV2),
    V3(Lockfile),
}

impl LockfileVersion {
    pub fn from_lockfile_string(raw_string: &str) -> Result<Self, LockfileError> {
        match raw_string {
            _ if raw_string.starts_with("# Lockfile v1") => {
                let lockfile = toml::from_str::<LockfileV2>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V1(lockfile))
            }
            _ if raw_string.starts_with("# Lockfile v2") => {
                let lockfile = toml::from_str::<LockfileV2>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V2(lockfile))
            }
            _ if raw_string.starts_with("# Lockfile v3") => {
                let lockfile = toml::from_str::<Lockfile>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V3(lockfile))
            }
            _ => Err(LockfileError::InvalidOrMissingVersion),
        }
    }
}

pub fn fix_up_v1_package_names(lockfile: &mut LockfileV2) {
    for command in lockfile.commands.iter_mut() {
        let (_, command): (&String, &mut LockfileCommand) = command;
        let package_name =
            normalize_global_namespace_package_name(command.package_name.as_str().into());
        command.package_name = package_name.to_string();
    }
    lockfile.modules = lockfile
        .modules
        .clone()
        .into_iter()
        .map(|(package_name, mut versions)| {
            let correct_name =
                normalize_global_namespace_package_name(package_name.as_str().into()).to_string();

            for (_version, modules) in versions.iter_mut() {
                for (_module_name, module) in modules.iter_mut() {
                    let module: &mut LockfileModuleV2 = module;
                    module.package_name = correct_name.clone();
                }
            }
            (correct_name.clone(), versions)
        })
        .collect();
}

pub fn convert_lockfilev2_to_v3(lockfile: LockfileV2) -> Lockfile {
    let mut modules: BTreeMap<String, _> = Default::default();
    for (k1, version_map) in lockfile.modules.into_iter() {
        let mut ver_map: BTreeMap<semver::Version, BTreeMap<_, _>> = Default::default();
        for (k2, module_map) in version_map.into_iter() {
            let mut name_map: BTreeMap<String, LockfileModule> = Default::default();
            for (k3, module_data) in module_map.into_iter() {
                let module = LockfileModule {
                    name: module_data.name,
                    package_version: module_data.package_version,
                    package_name: module_data.package_name,
                    source: module_data.source,
                    resolved: module_data.resolved,
                    abi: module_data.abi,
                    entry: module_data.entry.clone(),
                    root: module_data.entry,
                };
                name_map.insert(k3, module);
            }
            ver_map.insert(k2, name_map);
        }
        modules.insert(k1, ver_map);
    }
    Lockfile {
        modules,
        commands: lockfile.commands,
    }
}

#[cfg(test)]
mod test {
    use crate::data::lock::lockfile::Lockfile;
    use crate::data::lock::migrate::fix_up_v1_package_names;

    #[test]
    fn test_fix_up_v1_lockfile() {
        let v1_lockfile_toml = toml! {
            [modules.sqlite."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "sqlite"
            source = "registry+sqlite"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            abi = "emscripten"
            entry = "wapm_packages\\_\\sqlite@0.1.1\\sqlite.wasm"
            [commands.sqlite]
            name = "sqlite"
            package_name = "sqlite"
            package_version = "0.1.1"
            module = "sqlite"
            is_top_level_dependency = true
        };

        let mut lockfile: Lockfile = toml::from_str(&v1_lockfile_toml.to_string()).unwrap();

        let v2_lockfile_toml = toml! {
            [modules."_/sqlite"."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "_/sqlite"
            source = "registry+sqlite"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            abi = "emscripten"
            entry = "wapm_packages\\_\\sqlite@0.1.1\\sqlite.wasm"
            [commands.sqlite]
            name = "sqlite"
            package_name = "_/sqlite"
            package_version = "0.1.1"
            module = "sqlite"
            is_top_level_dependency = true
        };

        let expected_v2_lockfile: Lockfile = toml::from_str(&v2_lockfile_toml.to_string()).unwrap();

        fix_up_v1_package_names(&mut lockfile);

        assert_eq!(expected_v2_lockfile, lockfile);
    }
}
