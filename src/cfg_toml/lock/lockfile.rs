use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::cfg_toml::lock::{LOCKFILE_HEADER, LOCKFILE_NAME};
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::io::{Read, Write};
use std::path::Path;

pub type ModuleMap<'a> =
    BTreeMap<&'a str, BTreeMap<&'a str, BTreeMap<&'a str, LockfileModule<'a>>>>;
pub type CommandMap<'a> = BTreeMap<&'a str, LockfileCommand<'a>>;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct Lockfile<'a> {
    #[serde(borrow = "'a")]
    pub modules: ModuleMap<'a>, // PackageName -> VersionNumber -> ModuleName -> Module
    #[serde(borrow = "'a")]
    pub commands: CommandMap<'a>, // CommandName -> Command
}

impl<'a> Lockfile<'a> {
    pub fn open<P: AsRef<Path>>(
        directory: P,
        lockfile_string: &'a mut String,
    ) -> Result<Lockfile<'a>, LockfileError> {
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut lockfile_file =
            File::open(lockfile_path).map_err(|_| LockfileError::MissingLockfile)?;
        lockfile_file
            .read_to_string(lockfile_string)
            .map_err(|e| LockfileError::FileIoErrorReadingLockfile(e))?;
        toml::from_str(lockfile_string.as_str()).map_err(|e| LockfileError::TomlParseError(e))
    }

    /// Save the lockfile to the directory.
    pub fn save<P: AsRef<Path>>(&self, directory: P) -> Result<(), failure::Error> {
        let lockfile_string = toml::to_string(self)?;
        let lockfile_string = format!("{}\n{}", LOCKFILE_HEADER, lockfile_string);
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut file = File::create(&lockfile_path)?;
        file.write_all(lockfile_string.as_bytes())?;
        Ok(())
    }

    pub fn get_command(&self, command_name: &str) -> Result<&LockfileCommand, failure::Error> {
        self.commands
            .get(command_name)
            .ok_or(LockfileError::CommandNotFound(command_name.to_string()).into())
    }

    pub fn get_module(
        &self,
        package_name: &str,
        package_version: &str,
        module_name: &str,
    ) -> Result<&LockfileModule, failure::Error> {
        let version_map = self.modules.get(package_name).ok_or::<failure::Error>(
            LockfileError::PackageWithVersionNotFoundWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module_map = version_map.get(package_version).ok_or::<failure::Error>(
            LockfileError::VersionNotFoundForPackageWhenFindingModule(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        let module = module_map.get(module_name).ok_or::<failure::Error>(
            LockfileError::ModuleForPackageVersionNotFound(
                package_name.to_string(),
                package_version.to_string(),
                module_name.to_string(),
            )
            .into(),
        )?;
        Ok(module)
    }
}

#[derive(Debug, Fail)]
pub enum LockfileError {
    #[fail(display = "Command not found: {}", _0)]
    CommandNotFound(String),
    #[fail(display = "module {} in package \"{} {}\" was not found", _2, _0, _1)]
    ModuleForPackageVersionNotFound(String, String, String),
    #[fail(
        display = "Package \"{}\" with version \"{}\" was nto found searching for module \"{}\"",
        _0, _1, _2
    )]
    PackageWithVersionNotFoundWhenFindingModule(String, String, String),
    #[fail(
        display = "version \"{}\" for package \"{}\" was not found when searching for module \"{}\".",
        _1, _0, _2
    )]
    VersionNotFoundForPackageWhenFindingModule(String, String, String),
    #[fail(display = "Lockfile file not found.")]
    MissingLockfile,
    #[fail(display = "File I/O error reading lockfile. I/O error: {:?}", _0)]
    FileIoErrorReadingLockfile(io::Error),
    #[fail(
        display = "Failed to parse lockfile toml. Did you modify the generated lockfile? Toml error: {:?}",
        _0
    )]
    TomlParseError(toml::de::Error),
}

#[cfg(test)]
mod get_command_tests {
    use crate::lock::Lockfile;
    use toml;

    #[test]
    fn get_commands() {
        let wapm_lock_toml = toml! {
            [modules."abc/foo"."1.0.0"."foo"]
            name = "foo"
            package_name = "abc/foo"
            package_version = "1.0.0"
            source = ""
            resolved = ""
            abi = "none"
            entry = "target.wasm"
            [modules."xyz/bar"."3.0.0"."bar_module_a"]
            name = "bar_module_a"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            source = ""
            resolved = ""
            abi = "none"
            entry = "target.wasm"
            [modules."xyz/bar"."3.0.0"."bar_module_b"]
            name = "bar_module_b"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            source = ""
            resolved = ""
            abi = "none"
            entry = "target.wasm"
            // zero commands in the "abc/foo" package
            // one command in module "bar_module_a" of package "xyz/bar"
            [commands."bar_cmd_a"]
            name = "bar_cmd_a"
            module = "bar_module_a"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            is_top_level_dependency = true
            // two commands in module "bar_module_b" of package "xyz/bar"
            [commands."bar_cmd_b"]
            name = "bar_cmd_b"
            module = "bar_module_b"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            is_top_level_dependency = true
            [commands."bar_cmd_c"]
            name = "bar_cmd_c"
            module = "bar_module_b"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            is_top_level_dependency = true
        };
        // must stringify this first
        let s = wapm_lock_toml.to_string();
        let lockfile: Lockfile = toml::from_str(&s).unwrap();

        let not_a_command = "not_a_command";
        let bar_cmd_a = "bar_cmd_a";
        let bar_cmd_b = "bar_cmd_b";
        let bar_cmd_c = "bar_cmd_c";
        let bar_cmd_d = "bar_cmd_d"; // not a command

        let result = lockfile.get_command(not_a_command);
        assert!(result.is_err());
        let result = lockfile.get_command(bar_cmd_a);
        assert!(result.is_ok());
        let result = lockfile.get_command(bar_cmd_b);
        assert!(result.is_ok());
        let result = lockfile.get_command(bar_cmd_c);
        assert!(result.is_ok());
        let result = lockfile.get_command(bar_cmd_d);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod create_from_manifest_tests {
    use crate::dependency_resolver::Dependency;
    use crate::dependency_resolver::TestRegistry;
    use crate::lock::Lockfile;
    use crate::manifest::Manifest;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn create_from_manifest() {
        let foo_toml: toml::Value = toml! {
            [package]
            name = "_/root_pkg"
            description = "foo in the ns namespace"
            version = "1.1.1"
            [dependencies]
            "_/test_dep_a" = "1.0.0"
            "_/test_dep_b" = "2.0.0"
            [[module]]
            name = "root_module"
            source = "root.wasm"
            [[command]]
            module = "root_module"
            name = "root_pkg_command_a"
            [[command]]
            module = "root_module"
            name = "root_pkg_command_b"
        };
        let toml_string = foo_toml.to_string();
        let foo_manifest: Manifest = toml::from_str(&toml_string).unwrap();

        let test_dep_a_manifest_toml = toml! {
            [package]
            name = "_/test_dep_a"
            version = "1.0.0"
            description = "test dep a"
            [[module]]
            name = "test_dep_a_module"
            source = "a.wasm"
            [[command]]
            name = "mod_a_command"
            module = "test_dep_a_module"
        };
        let test_dep_a_manifest_string = test_dep_a_manifest_toml.to_string();
        let test_dep_a_manifest: Manifest = toml::from_str(&test_dep_a_manifest_string).unwrap();

        let test_dep_a = Dependency::new(
            "_/test_dep_a",
            "1.0.0",
            test_dep_a_manifest,
            "dep_a_test.com",
        );

        let test_dep_b_manifest_toml = toml! {
            [package]
            name = "_/test_dep_b"
            version = "2.0.0"
            description = "test dep b"
            [[module]]
            name = "test_dep_b_module"
            source = "b.wasm"
            [[command]]
            name = "mod_b_command"
            module = "test_dep_b_module"
        };
        let test_dep_b_manifest_string = test_dep_b_manifest_toml.to_string();
        let test_dep_b_manifest: Manifest = toml::from_str(&test_dep_b_manifest_string).unwrap();

        let test_dep_b = Dependency::new(
            "_/test_dep_b",
            "2.0.0",
            test_dep_b_manifest,
            "dep_b_test.com",
        );

        let mut test_registry_map = BTreeMap::new();
        let version_vec_a = vec![test_dep_a];
        let version_vec_b = vec![test_dep_b];
        test_registry_map.insert("_/test_dep_a", version_vec_a);
        test_registry_map.insert("_/test_dep_b", version_vec_b);
        let mut test_registry = TestRegistry(test_registry_map);
        let actual_lockfile =
            Lockfile::new_from_manifest(&foo_manifest, &mut test_registry).unwrap();

        let dep_a_entry = ["wapm_packages", "_", "test_dep_a@1.0.0", "a.wasm"]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();
        let dep_b_entry = ["wapm_packages", "_", "test_dep_b@2.0.0", "b.wasm"]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();

        let expected_lockfile_toml = toml! {
            [modules."_/test_dep_a"."1.0.0"."test_dep_a_module"]
            name = "test_dep_a_module"
            package_name = "_/test_dep_a"
            package_version = "1.0.0"
            source = "registry+test_dep_a_module"
            resolved = "dep_a_test.com"
            abi = "none"
            entry = dep_a_entry
            [modules."_/test_dep_b"."2.0.0"."test_dep_b_module"]
            name = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.0.0"
            source = "registry+test_dep_b_module"
            resolved = "dep_b_test.com"
            abi = "none"
            entry = dep_b_entry
            [commands."mod_a_command"]
            name = "mod_a_command"
            module = "test_dep_a_module"
            package_name = "_/test_dep_a"
            package_version = "1.0.0"
            is_top_level_dependency = true
            [commands."mod_b_command"]
            name = "mod_b_command"
            module = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.0.0"
            is_top_level_dependency = true
            [commands."root_pkg_command_a"]
            name = "root_pkg_command_a"
            module = "root_module"
            package_name = "_/root_pkg"
            package_version = "1.1.1"
            is_top_level_dependency = true
            [commands."root_pkg_command_b"]
            name = "root_pkg_command_b"
            module = "root_module"
            package_name = "_/root_pkg"
            package_version = "1.1.1"
            is_top_level_dependency = true
        };
        let expected_lockfile_string = expected_lockfile_toml.to_string();

        let expected_lockfile: Lockfile = toml::from_str(&expected_lockfile_string).unwrap();

        assert_eq!(expected_lockfile, actual_lockfile);
    }
}

#[cfg(test)]
mod create_from_manifest_and_lockfile_tests {
    use crate::dependency_resolver::Dependency;
    use crate::dependency_resolver::TestRegistry;
    use crate::lock::Lockfile;
    use crate::manifest::Manifest;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn create_from_manifest_and_lockfile() {
        let foo_toml: toml::Value = toml! {
            [package]
            name = "_/root_pkg"
            description = "foo in the ns namespace"
            // updated version number
            version = "1.1.0"
            [dependencies]
            // removed dependency a, upgrade dependency b and added dependency c
            "_/test_dep_b" = "2.1.0"
            "_/test_dep_c" = "4.0.0"
            [[module]]
            name = "root_module"
            source = "root.wasm"
            // added a new root module
            [[module]]
            name = "root_module_2"
            source = "root_2.wasm"
            // removed root_pkg_command_a
            [[command]]
            module = "root_module"
            name = "root_pkg_command_b"
            // added root_pkg_command_c
            [[command]]
            module = "root_module"
            name = "root_pkg_command_c"
        };
        let toml_string = foo_toml.to_string();
        let foo_manifest: Manifest = toml::from_str(&toml_string).unwrap();

        let test_dep_a_manifest_toml = toml! {
            [package]
            name = "_/test_dep_a"
            version = "1.0.0"
            description = "test dep a"
            [[module]]
            name = "test_dep_a_module"
            source = "a.wasm"
            [[command]]
            name = "mod_a_command"
            module = "test_dep_a_module"
        };
        let test_dep_a_manifest_string = test_dep_a_manifest_toml.to_string();
        let test_dep_a_manifest: Manifest = toml::from_str(&test_dep_a_manifest_string).unwrap();

        let test_dep_a = Dependency::new(
            "_/test_dep_a",
            "1.0.0",
            test_dep_a_manifest,
            "dep_a_test.com",
        );

        let test_dep_b_manifest_toml = toml! {
            [package]
            name = "_/test_dep_b"
            version = "2.0.0"
            description = "test dep b"
            [[module]]
            name = "test_dep_b_module"
            source = "b.wasm"
            [[command]]
            name = "mod_b_command"
            module = "test_dep_b_module"
        };
        let test_dep_b_manifest_string = test_dep_b_manifest_toml.to_string();
        let test_dep_b_manifest: Manifest = toml::from_str(&test_dep_b_manifest_string).unwrap();

        let test_dep_b = Dependency::new(
            "_/test_dep_b",
            "2.0.0",
            test_dep_b_manifest,
            "dep_b_test.com",
        );

        let test_dep_b_manifest_update_toml = toml! {
            [package]
            name = "_/test_dep_b"
            version = "2.1.0"
            description = "test dep b"
            [[module]]
            name = "test_dep_b_module"
            source = "b.wasm"
            [[command]]
            name = "mod_b_command"
            module = "test_dep_b_module"
            // added new command
            [[command]]
            name = "mod_b_command_2"
            module = "test_dep_b_module"
        };
        let test_dep_b_manifest_update_string = test_dep_b_manifest_update_toml.to_string();
        let test_dep_b_manifest_update: Manifest =
            toml::from_str(&test_dep_b_manifest_update_string).unwrap();

        let test_dep_b_update = Dependency::new(
            "_/test_dep_b",
            "2.1.0",
            test_dep_b_manifest_update,
            "dep_b_test.com",
        );

        let test_dep_c_manifest_toml = toml! {
            [package]
            name = "_/test_dep_c"
            version = "4.0.0"
            description = "test dep c"
            [[module]]
            name = "test_dep_c_module"
            source = "c.wasm"
        };
        let test_dep_c_manifest_string = test_dep_c_manifest_toml.to_string();
        let test_dep_c_manifest: Manifest = toml::from_str(&test_dep_c_manifest_string).unwrap();

        let test_dep_c = Dependency::new(
            "_/test_dep_c",
            "4.0.0",
            test_dep_c_manifest,
            "dep_c_test.com",
        );

        let mut test_registry_map = BTreeMap::new();
        let version_vec_a = vec![test_dep_a];
        let version_vec_b = vec![test_dep_b, test_dep_b_update];
        let version_vec_c = vec![test_dep_c];
        test_registry_map.insert("_/test_dep_a", version_vec_a);
        test_registry_map.insert("_/test_dep_b", version_vec_b);
        test_registry_map.insert("_/test_dep_c", version_vec_c);

        let mut test_registry = TestRegistry(test_registry_map);

        let dep_a_entry = ["wapm_packages", "_", "test_dep_a@1.0.0", "a.wasm"]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();
        let dep_b_entry = ["wapm_packages", "_", "test_dep_b@2.0.0", "b.wasm"]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();

        let existing_lockfile_toml = toml! {
            [modules."_/test_dep_a"."1.0.0"."test_dep_a_module"]
            name = "test_dep_a_module"
            package_name = "_/test_dep_a"
            package_version = "1.0.0"
            source = "registry+test_dep_a_module"
            resolved = "dep_a_test.com"
            abi = "none"
            entry = dep_a_entry
            [modules."_/test_dep_b"."2.0.0"."test_dep_b_module"]
            name = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.0.0"
            source = "registry+test_dep_b_module"
            resolved = "dep_b_test.com"
            abi = "none"
            entry = dep_b_entry
            [commands."mod_a_command"]
            name = "mod_a_command"
            module = "test_dep_a_module"
            package_name = "_/test_dep_a"
            package_version = "1.0.0"
            is_top_level_dependency = true
            [commands."mod_b_command"]
            name = "mod_b_command"
            module = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.0.0"
            is_top_level_dependency = true
            [commands."root_pkg_command_a"]
            name = "root_pkg_command_a"
            module = "root_module"
            package_name = "_/root_pkg"
            package_version = "1.0.0"
            is_top_level_dependency = true
            [commands."root_pkg_command_b"]
            name = "root_pkg_command_b"
            module = "root_module"
            package_name = "_/root_pkg"
            package_version = "1.0.0"
            is_top_level_dependency = true
        };

        let existing_lockfile_string = existing_lockfile_toml.to_string();
        let existing_lockfile: Lockfile = toml::from_str(&existing_lockfile_string).unwrap();

        let dep_b_entry = ["wapm_packages", "_", "test_dep_b@2.1.0", "b.wasm"]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();
        let dep_c_entry = ["wapm_packages", "_", "test_dep_c@4.0.0", "c.wasm"]
            .iter()
            .collect::<PathBuf>()
            .to_string_lossy()
            .to_string();
        let expected_lockfile_toml = toml! {
            [modules."_/test_dep_b"."2.1.0"."test_dep_b_module"]
            name = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.1.0"
            source = "registry+test_dep_b_module"
            resolved = "dep_b_test.com"
            abi = "none"
            entry = dep_b_entry
            [modules."_/test_dep_c"."4.0.0"."test_dep_c_module"]
            name = "test_dep_c_module"
            package_name = "_/test_dep_c"
            package_version = "4.0.0"
            source = "registry+test_dep_c_module"
            resolved = "dep_c_test.com"
            abi = "none"
            entry = dep_c_entry
            [commands."mod_b_command"]
            name = "mod_b_command"
            module = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.1.0"
            is_top_level_dependency = true
            [commands."mod_b_command_2"]
            name = "mod_b_command_2"
            module = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.1.0"
            is_top_level_dependency = true
            [commands."root_pkg_command_b"]
            name = "root_pkg_command_b"
            module = "root_module"
            package_name = "_/root_pkg"
            package_version = "1.1.0"
            is_top_level_dependency = true
            [commands."root_pkg_command_c"]
            name = "root_pkg_command_c"
            module = "root_module"
            package_name = "_/root_pkg"
            package_version = "1.1.0"
            is_top_level_dependency = true
        };

        let expected_lockfile_string = expected_lockfile_toml.to_string();
        let expected_lockfile: Lockfile = toml::from_str(&expected_lockfile_string).unwrap();

        let actual_lockfile = Lockfile::new_from_manifest_and_lockfile(
            &foo_manifest,
            existing_lockfile,
            &mut test_registry,
        )
        .unwrap();
        assert_eq!(expected_lockfile, actual_lockfile);
    }
}
