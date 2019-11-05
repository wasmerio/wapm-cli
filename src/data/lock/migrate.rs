use crate::data::lock::lockfile::{LockfileV2, LockfileV3, LockfileV4};
use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::{LockfileModuleV2, LockfileModuleV3, LockfileModuleV4};
use crate::dataflow::lockfile_packages::LockfileError;
use crate::dataflow::normalize_global_namespace_package_name;

use std::collections::*;
use std::path::PathBuf;

pub enum LockfileVersion {
    V1(LockfileV2),
    V2(LockfileV2),
    V3(LockfileV3),
    V4(LockfileV4),
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
                let lockfile = toml::from_str::<LockfileV3>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V3(lockfile))
            }
            _ if raw_string.starts_with("# Lockfile v4") => {
                let lockfile = toml::from_str::<LockfileV4>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V4(lockfile))
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

pub fn convert_lockfilev2_to_v3(lockfile: LockfileV2) -> LockfileV3 {
    let mut modules: BTreeMap<String, _> = Default::default();
    for (k1, version_map) in lockfile.modules.into_iter() {
        let mut ver_map: BTreeMap<semver::Version, BTreeMap<_, _>> = Default::default();
        for (k2, module_map) in version_map.into_iter() {
            let mut name_map: BTreeMap<String, LockfileModuleV3> = Default::default();
            for (k3, module_data) in module_map.into_iter() {
                let module = LockfileModuleV3 {
                    name: module_data.name,
                    package_version: module_data.package_version,
                    package_name: module_data.package_name,
                    source: module_data.source,
                    resolved: module_data.resolved,
                    abi: module_data.abi,
                    entry: module_data.entry.clone(),
                    root: {
                        let mut entry = PathBuf::from(module_data.entry);
                        // remove the module.wasm file
                        entry.pop();
                        entry.to_string_lossy().to_string()
                    },
                    prehashed_module_key: None,
                };
                name_map.insert(k3, module);
            }
            ver_map.insert(k2, name_map);
        }
        modules.insert(k1, ver_map);
    }
    LockfileV3 {
        modules,
        commands: lockfile.commands,
    }
}

pub fn convert_lockfilev3_to_v4(lockfile: LockfileV3) -> LockfileV4 {
    let mut modules: BTreeMap<String, _> = Default::default();
    for (k1, version_map) in lockfile.modules.into_iter() {
        let mut ver_map: BTreeMap<semver::Version, BTreeMap<_, _>> = Default::default();
        for (k2, module_map) in version_map.into_iter() {
            let mut name_map: BTreeMap<String, LockfileModuleV4> = Default::default();
            for (k3, module_data) in module_map.into_iter() {
                let module = LockfileModuleV4 {
                    name: module_data.name,
                    package_version: module_data.package_version.clone(),
                    package_name: module_data.package_name.clone(),
                    package_path: format!("{}@{}", module_data.package_name, module_data.package_version),
                    resolved: module_data.resolved,
                    resolved_source: module_data.source,
                    abi: module_data.abi,
                    source: module_data.entry.clone(),
                    // root: {
                    //     let mut entry = PathBuf::from(module_data.entry);
                    //     // remove the module.wasm file
                    //     entry.pop();
                    //     entry.to_string_lossy().to_string()
                    // },
                    prehashed_module_key: None,
                };
                name_map.insert(k3, module);
            }
            ver_map.insert(k2, name_map);
        }
        modules.insert(k1, ver_map);
    }
    LockfileV4 {
        modules,
        commands: lockfile.commands,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::data::lock::lockfile::{LockfileV2, LockfileV3, LockfileV4};

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

        let mut lockfile: LockfileV2 = toml::from_str(&v1_lockfile_toml.to_string()).unwrap();

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

        let expected_v2_lockfile: LockfileV2 =
            toml::from_str(&v2_lockfile_toml.to_string()).unwrap();

        fix_up_v1_package_names(&mut lockfile);

        assert_eq!(expected_v2_lockfile, lockfile);
    }

    #[test]
    fn upgrade_to_v3() {
        let v1_lockfile_toml = toml! {
            [modules.sqlite."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "sqlite"
            source = "registry+sqlite"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            abi = "emscripten"
            entry = "wapm_packages/_/sqlite@0.1.1/sqlite.wasm"
            [commands.sqlite]
            name = "sqlite"
            package_name = "sqlite"
            package_version = "0.1.1"
            module = "sqlite"
            is_top_level_dependency = true
        };

        let mut lockfile_v1: LockfileV2 = toml::from_str(&v1_lockfile_toml.to_string()).unwrap();

        let v3_lockfile_toml = toml! {
            [modules."_/sqlite"."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "_/sqlite"
            source = "registry+sqlite"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            abi = "emscripten"
            entry = "wapm_packages/_/sqlite@0.1.1/sqlite.wasm"
            root = "wapm_packages/_/sqlite@0.1.1"
            [commands.sqlite]
            name = "sqlite"
            package_name = "_/sqlite"
            package_version = "0.1.1"
            module = "sqlite"
            is_top_level_dependency = true
        };

        let expected_v3_lockfile: LockfileV3 = toml::from_str(&v3_lockfile_toml.to_string()).unwrap();

        fix_up_v1_package_names(&mut lockfile_v1);
        let converted_lockfile_v3 = convert_lockfilev2_to_v3(lockfile_v1);

        assert_eq!(expected_v3_lockfile, converted_lockfile_v3);
    }


    #[test]
    fn upgrade_to_v4() {
        let v3_lockfile_toml = toml! {
            [modules."_/sqlite"."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "_/sqlite"
            source = "registry+sqlite"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            abi = "emscripten"
            entry = "wapm_packages/_/sqlite@0.1.1/sqlite.wasm"
            root = "wapm_packages/_/sqlite@0.1.1"
            [commands.sqlite]
            name = "sqlite"
            package_name = "_/sqlite"
            package_version = "0.1.1"
            module = "sqlite"
            is_top_level_dependency = true
        };

        let v3_lockfile: LockfileV3 = toml::from_str(&v3_lockfile_toml.to_string()).unwrap();

        let v4_lockfile_toml = toml! {
            [modules."_/sqlite"."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "_/sqlite"
            package_path = "_/sqlite@0.1.1"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            resolved_source = "registry+sqlite"
            abi = "emscripten"
            source = "sqlite.wasm"
            [commands.sqlite]
            name = "sqlite"
            package_name = "_/sqlite"
            package_version = "0.1.1"
            module = "sqlite"
            is_top_level_dependency = true
        };


        let v4_lockfile: LockfileV4 = toml::from_str(&v4_lockfile_toml.to_string()).unwrap();

        let v4_lockfile_converted = convert_lockfilev3_to_v4(v3_lockfile);

        assert_eq!(v4_lockfile, v4_lockfile_converted);
    }
}
