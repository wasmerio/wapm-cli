use crate::data::lock::lockfile::Lockfile;
use crate::data::lock::lockfile_command::LockfileCommand;
use crate::data::lock::lockfile_module::LockfileModule;
use crate::dataflow::lockfile_packages::LockfileError;
use crate::dataflow::normalize_global_namespace_package_name;

pub enum LockfileVersion {
    V1(Lockfile),
    V2(Lockfile),
}

impl LockfileVersion {
    pub fn from_lockfile_string(raw_string: &str) -> Result<Self, LockfileError> {
        match raw_string {
            _ if raw_string.starts_with("# Lockfile v1") => {
                let lockfile = toml::from_str::<Lockfile>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V1(lockfile))
            }
            _ if raw_string.starts_with("# Lockfile v2") => {
                let lockfile = toml::from_str::<Lockfile>(&raw_string)
                    .map_err(|e| LockfileError::LockfileTomlParseError(e.to_string()))?;
                Ok(LockfileVersion::V2(lockfile))
            }
            _ => Err(LockfileError::InvalidOrMissingVersion),
        }
    }
}

pub fn fix_up_v1_package_names(lockfile: &mut Lockfile) {
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
                    let module: &mut LockfileModule = module;
                    module.package_name = correct_name.clone();
                }
            }
            (correct_name.clone(), versions)
        })
        .collect();
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
