//! Code for migrating between Lockfile versions
//!
//! Lock file versions are stored as the first line in the lockfile, of the form:
//! `# Lockfile vN` where `N` is 1 or more digits.

use crate::data::lock::lockfile::{LockfileV2, LockfileV3, LockfileV4};
use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::{LockfileModuleV2, LockfileModuleV3, LockfileModuleV4};
use crate::data::manifest::PACKAGES_DIR_NAME;
use crate::dataflow::lockfile_packages::LockfileError;
use crate::dataflow::normalize_global_namespace_package_name;

use lazy_static::lazy_static;
use regex::Regex;
use std::collections::*;
use std::path::{Path, PathBuf};

/// All the lockfile versions
pub enum LockfileVersion {
    V1(LockfileV2),
    V2(LockfileV2),
    V3(LockfileV3),
    V4(LockfileV4),
}

lazy_static! {
    static ref VER_NUM_RE: Regex = Regex::new(r"^# Lockfile v(\d+)$").unwrap();
}

impl LockfileVersion {
    pub fn from_lockfile_string(raw_string: &str) -> Result<Self, LockfileError> {
        let first_line = raw_string
            .lines()
            .next()
            .ok_or(LockfileError::InvalidOrMissingVersion)?;
        let lockfile_version = (*VER_NUM_RE)
            .captures_iter(first_line)
            .map(|captures| captures[1].parse::<usize>())
            .next()
            .ok_or(LockfileError::InvalidOrMissingVersion)?
            .map_err(|_| LockfileError::InvalidOrMissingVersion)?;

        match lockfile_version {
            1 => {
                let lockfile_v1 = toml::from_str::<LockfileV2>(raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V1(lockfile_v1))
            }
            2 => {
                let lockfile_v2 = toml::from_str::<LockfileV2>(raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V2(lockfile_v2))
            }
            3 => {
                let lockfile_v3 = toml::from_str::<LockfileV3>(raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V3(lockfile_v3))
            }
            4 => {
                let lockfile_v4 = toml::from_str::<LockfileV4>(raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V4(lockfile_v4))
            }
            0 => Err(LockfileError::InvalidOrMissingVersion),
            _ => Err(LockfileError::VersionTooHigh),
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
            (correct_name, versions)
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

pub fn convert_lockfilev3_to_v4(lockfile: LockfileV3, directory: &Path) -> LockfileV4 {
    let dir_prefix = directory.join(PACKAGES_DIR_NAME);

    let mut modules: BTreeMap<String, _> = Default::default();
    for (k1, version_map) in lockfile.modules.into_iter() {
        let mut ver_map: BTreeMap<semver::Version, BTreeMap<_, _>> = Default::default();
        for (k2, module_map) in version_map.into_iter() {
            let mut name_map: BTreeMap<String, LockfileModuleV4> = Default::default();
            for (k3, module_data) in module_map.into_iter() {
                let package_path = format!(
                    "{}@{}",
                    &module_data.package_name, &module_data.package_version
                );
                let module = LockfileModuleV4 {
                    name: module_data.name,
                    package_version: module_data.package_version,
                    package_name: module_data.package_name,
                    resolved: module_data.resolved,
                    resolved_source: module_data.source,
                    abi: module_data.abi,
                    source: {
                        let full_prefix = dir_prefix.join(&package_path);
                        // Somtimes .entry will be absolute and other times relative
                        // We make it absolute to make it easier to work with and avoid errors
                        let path = directory.join(&module_data.entry);
                        path.strip_prefix(&full_prefix)
                            .expect("Corrupt data in Lockfile, please delete it and try again")
                            .to_string_lossy()
                            .to_string()
                    },
                    package_path,
                    prehashed_module_key: module_data.prehashed_module_key,
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
    use std::path::Path;

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

        let expected_v3_lockfile: LockfileV3 =
            toml::from_str(&v3_lockfile_toml.to_string()).unwrap();

        fix_up_v1_package_names(&mut lockfile_v1);
        let converted_lockfile_v3 = convert_lockfilev2_to_v3(lockfile_v1);

        assert_eq!(expected_v3_lockfile, converted_lockfile_v3);
    }

    #[test]
    fn upgrade_to_v4() {
        let directory = Path::new("/home/shiba/project");

        let v3_lockfile_toml = toml! {
            [modules."_/sqlite"."0.1.1".sqlite]
            name = "sqlite"
            package_version = "0.1.1"
            package_name = "_/sqlite"
            source = "registry+sqlite"
            resolved = "https://registry-cdn.wapm.dev/packages/_/sqlite/sqlite-0.1.1.tar.gz"
            abi = "emscripten"
            entry = "/home/shiba/project/wapm_packages/_/sqlite@0.1.1/sqlite.wasm"
            root = "/home/shiba/project/wapm_packages/_/sqlite@0.1.1"
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

        let v4_lockfile_converted = convert_lockfilev3_to_v4(v3_lockfile, directory);

        assert_eq!(v4_lockfile, v4_lockfile_converted);
    }
}
