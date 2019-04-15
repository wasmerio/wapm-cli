use crate::dependency_resolver::PackageRegistryLike;
use crate::lock::lockfile_command::LockfileCommand;
use crate::lock::lockfile_module::LockfileModule;
use crate::lock::{LOCKFILE_HEADER, LOCKFILE_NAME};
use crate::manifest::Manifest;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

type ModuleMap<'a> = BTreeMap<&'a str, BTreeMap<&'a str, BTreeMap<&'a str, LockfileModule<'a>>>>;
type CommandMap<'a> = BTreeMap<&'a str, LockfileCommand<'a>>;

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
    ) -> Result<Lockfile<'a>, failure::Error> {
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut lockfile_file = File::open(lockfile_path)?;
        lockfile_file.read_to_string(lockfile_string)?;
        toml::from_str(lockfile_string.as_str()).map_err(|e| e.into())
    }

    /// This method constructs a new lockfile with just a manifest. This is typical if no lockfile
    /// previously exists. All dependencies will be fetched.
    pub fn new_from_manifest<D: PackageRegistryLike>(
        manifest: &'a Manifest,
        dependency_resolver: &'a mut D,
    ) -> Result<Self, failure::Error> {
        let mut lockfile_modules = BTreeMap::new();
        let mut lockfile_commands = BTreeMap::new();
        let unresolved_dependencies = manifest.extract_dependencies()?;
        let dependencies = dependency_resolver.get_all_dependencies(
            unresolved_dependencies,
        )?;
        for dependency in dependencies.iter() {
            let package_name = dependency.manifest.package.name.as_str();
            let package_version = dependency.manifest.package.version.as_str();
            let lockfile_modules_vec = LockfileModule::from_dependency(*dependency)?;
            for lockfile_module in lockfile_modules_vec.into_iter() {
                let module_name = lockfile_module.name.clone();
                let version_map = lockfile_modules
                    .entry(package_name)
                    .or_insert(BTreeMap::new());
                let module_map = version_map
                    .entry(package_version)
                    .or_insert(BTreeMap::new());
                module_map.insert(module_name, lockfile_module);
            }
            let lockfile_commands_vec = LockfileCommand::from_dependency(*dependency)?;
            for lockfile_command in lockfile_commands_vec {
                if lockfile_command.is_top_level_dependency {
                    lockfile_commands.insert(lockfile_command.name, lockfile_command);
                }
            }
        }

        let pkg_name = &manifest.package.name;
        let pkg_version = &manifest.package.version;
        // handle this manifest's commands
        if let Some(commands) = manifest.command.as_ref() {
            for command in commands {
                let lockfile_command =
                    LockfileCommand::from_command(pkg_name, pkg_version, command);
                lockfile_commands.insert(&command.name, lockfile_command);
            }
        }

        let new_lockfile = Lockfile {
            modules: lockfile_modules,
            commands: lockfile_commands,
        };
        Ok(new_lockfile)
    }

    pub fn new_from_manifest_and_lockfile<D: PackageRegistryLike>(
        manifest: &'a Manifest,
        existing_lockfile: Lockfile<'a>,
        dependency_resolver: &'a mut D,
    ) -> Result<Self, failure::Error> {
        let mut existing_lockfile = existing_lockfile;
        // get all dependencies that changed and references to unchanged lockfile modules
        let manifest_dependencies = manifest.extract_dependencies()?;
        // mutate the existing lockfile: prune changed modules and commands, leave everything else
        let changed_dependencies = resolve_changes(
            manifest_dependencies,
            &mut existing_lockfile.modules,
            &mut existing_lockfile.commands,
        );
        let dependencies = dependency_resolver.get_all_dependencies(
            changed_dependencies,
        )?;
        for dependency in dependencies.iter() {
            let package_name = dependency.manifest.package.name.as_str();
            let package_version = dependency.manifest.package.version.as_str();
            let lockfile_modules_vec = LockfileModule::from_dependency(*dependency)?;
            for lockfile_module in lockfile_modules_vec.into_iter() {
                let module_name = lockfile_module.name.clone();
                let version_map = existing_lockfile
                    .modules
                    .entry(package_name)
                    .or_insert(BTreeMap::new());
                let module_map = version_map
                    .entry(package_version)
                    .or_insert(BTreeMap::new());
                module_map.insert(module_name, lockfile_module);
            }
            let lockfile_commands_vec = LockfileCommand::from_dependency(*dependency)?;
            for lockfile_command in lockfile_commands_vec {
                if lockfile_command.is_top_level_dependency {
                    existing_lockfile
                        .commands
                        .insert(lockfile_command.name, lockfile_command)
                        .is_some();
                }
            }
        }

        let pkg_name = &manifest.package.name;
        let pkg_version = &manifest.package.version;

        // just clear commands that are marked for the root package and re-add them from the manifest
        let mut root_commands_to_remove: Vec<&str> = vec![];
        for (command_name, command) in existing_lockfile.commands.iter() {
            if command.package_name == manifest.package.name {
                root_commands_to_remove.push(command_name.clone());
            }
        }
        for command_name in root_commands_to_remove {
            existing_lockfile.commands.remove(command_name);
        }

        // handle this manifest's commands
        if let Some(commands) = manifest.command.as_ref() {
            for command in commands {
                let lockfile_command =
                    LockfileCommand::from_command(pkg_name, pkg_version, command);
                existing_lockfile
                    .commands
                    .insert(&command.name, lockfile_command);
            }
        }

        let new_lockfile = Lockfile {
            modules: existing_lockfile.modules,
            commands: existing_lockfile.commands,
        };

        Ok(new_lockfile)
    }

    pub fn new_from_lockfile_and_installed_dependencies<D: PackageRegistryLike>(
        installed_dependencies: Vec<(&'a str, &'a str)>,
        mut existing_lockfile: Lockfile<'a>,
        dependency_resolver: &'a mut D,
    ) -> Result<Self, failure::Error> {
        let dependencies =
            dependency_resolver.get_all_dependencies(installed_dependencies)?;
        for dependency in dependencies.iter() {
            let package_name = dependency.manifest.package.name.as_str();
            let package_version = dependency.manifest.package.version.as_str();
            let lockfile_modules_vec = LockfileModule::from_dependency(*dependency)?;
            // if the package is already in the lockfile, then we are changing the version,
            // simply clear the map and below we will re-insert the new version
            if existing_lockfile.modules.contains_key(package_name) {
                existing_lockfile.modules.clear();
                // remove the commands for the module
                let commands_to_remove = existing_lockfile
                    .commands
                    .iter()
                    .filter(|(_, command)| command.package_name == package_name)
                    .map(|(command_name, _)| command_name.clone())
                    .collect::<Vec<_>>();
                for command_to_remove in commands_to_remove {
                    existing_lockfile.commands.remove(command_to_remove);
                }
            }
            for lockfile_module in lockfile_modules_vec.into_iter() {
                let module_name = lockfile_module.name.clone();
                let version_map = existing_lockfile.modules.entry(package_name).or_default();
                let module_map = version_map.entry(package_version).or_default();
                module_map.insert(module_name, lockfile_module);
            }
            let lockfile_commands_vec = LockfileCommand::from_dependency(*dependency)?;
            for lockfile_command in lockfile_commands_vec {
                if lockfile_command.is_top_level_dependency {
                    existing_lockfile
                        .commands
                        .insert(lockfile_command.name, lockfile_command)
                        .is_some();
                }
            }
        }

        let new_lockfile = Lockfile {
            modules: existing_lockfile.modules,
            commands: existing_lockfile.commands,
        };

        Ok(new_lockfile)
    }

    pub fn new_from_installed_dependencies<D: PackageRegistryLike>(
        installed_dependencies: Vec<(&'a str, &'a str)>,
        dependency_resolver: &'a mut D,
    ) -> Result<Self, failure::Error> {
        let mut lockfile_modules: ModuleMap = BTreeMap::new();
        let mut lockfile_commands = BTreeMap::new();
        let dependencies =
            dependency_resolver.get_all_dependencies(installed_dependencies)?;
        for dependency in dependencies.iter() {
            let package_name = dependency.manifest.package.name.as_str();
            let package_version = dependency.manifest.package.version.as_str();
            let lockfile_modules_vec = LockfileModule::from_dependency(*dependency)?;
            for lockfile_module in lockfile_modules_vec.into_iter() {
                let module_name = lockfile_module.name.clone();
                let version_map = lockfile_modules.entry(package_name).or_default();
                let module_map = version_map.entry(package_version).or_default();
                module_map.insert(module_name, lockfile_module);
            }
            let lockfile_commands_vec = LockfileCommand::from_dependency(*dependency)?;
            for lockfile_command in lockfile_commands_vec {
                if lockfile_command.is_top_level_dependency {
                    lockfile_commands.insert(lockfile_command.name, lockfile_command);
                }
            }
        }

        let new_lockfile = Lockfile {
            modules: lockfile_modules,
            commands: lockfile_commands,
        };
        Ok(new_lockfile)
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
            LockfileError::ModuleNotFound(module_name.to_string()).into(),
        )?;
        let module_map = version_map.get(package_version).ok_or::<failure::Error>(
            LockfileError::ModuleNotFound(module_name.to_string()).into(),
        )?;
        let module = module_map.get(module_name).ok_or::<failure::Error>(
            LockfileError::ModuleNotFound(module_name.to_string()).into(),
        )?;
        Ok(module)
    }
}

#[derive(Debug, Fail)]
pub enum LockfileError {
    #[fail(display = "Command not found: {}", _0)]
    CommandNotFound(String),
    #[fail(display = "Module not found: {}", _0)]
    ModuleNotFound(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
    #[fail(
        display = "Could not resolve dependency in manifest. Package name: {}. Package version: ",
        _0
    )]
    CouldNotResolveManifestDependency(String, String),
    #[fail(
        display = "Multiple errors encountered while constructing lockfile {:?}",
        _0
    )]
    AggregateLockfileError(Vec<failure::Error>),
}

/// dependencies that are unchanged remain in the BTreeMap. A vec of string refs are returned which
/// are the package name and the version which is different or new. The strings live as long as the
/// dependencies vec.
fn resolve_changes<'dependencies, 'modules: 'dependencies>(
    dependencies: Vec<(&'dependencies str, &'dependencies str)>,
    lockfile_modules: &mut ModuleMap<'dependencies>,
    lockfile_commands: &mut CommandMap<'modules>,
) -> Vec<(&'dependencies str, &'dependencies str)> {
    let mut changes = vec![];
    let mut changed_package_names = vec![];
    for (name, version) in dependencies.iter() {
        match lockfile_modules.get(name) {
            Some(ref modules) if !modules.contains_key(version) => {
                lockfile_modules.remove(name);
                changed_package_names.push(name);
                changes.push((*name, *version));
            }
            None => {
                changes.push((*name, *version));
            }
            Some(_) => {}
        }
    }

    // remove all package dependencies modules and commands that were eliminated from the manifest
    let dependency_packages: Vec<_> = dependencies
        .iter()
        .map(|(package_name, _)| package_name)
        .collect();
    let removed_packages: Vec<_> = lockfile_modules
        .keys()
        .filter(|package_name| {
            dependency_packages
                .iter()
                .find(|name| name == &package_name)
                .is_none()
        })
        .map(|n| n.clone())
        .collect();
    for removed_package in removed_packages.iter() {
        lockfile_modules.remove(*removed_package);
        let removed_commands: Vec<_> = lockfile_commands
            .iter()
            .filter(|(_command_name, command)| &command.package_name == removed_package)
            .map(|(command_name, _)| command_name.clone())
            .collect();
        for removed_command_name in removed_commands {
            let r = lockfile_commands.remove(removed_command_name).is_some();
            eprintln!(
                "second round: removing command: {}, succeded: {}",
                removed_command_name, r
            );
        }
    }

    // prune all commands that have changed
    for changed_package_name in changed_package_names {
        let removed_commands: Vec<&str> = lockfile_commands
            .iter()
            .map(|(cmd_name, c)| (cmd_name, c.package_name))
            .filter(|(_cmd_name, package_name)| *changed_package_name == *package_name)
            .map(|(cmd_name, _)| *cmd_name)
            .collect();
        for removed_command_name in removed_commands {
            let r = lockfile_commands.remove(removed_command_name).is_some();
            eprintln!(
                "second round: removing command: {}, succeded: {}",
                removed_command_name, r
            );
        }
    }
    changes
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
            integrity = ""
            hash = ""
            abi = "None"
            entry = "target.wasm"
            [modules."xyz/bar"."3.0.0"."bar_module_a"]
            name = "bar_module_a"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            source = ""
            resolved = ""
            integrity = ""
            hash = ""
            abi = "None"
            entry = "target.wasm"
            [modules."xyz/bar"."3.0.0"."bar_module_b"]
            name = "bar_module_b"
            package_name = "xyz/bar"
            package_version = "3.0.0"
            source = ""
            resolved = ""
            integrity = ""
            hash = ""
            abi = "None"
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

        let test_dep_a = Dependency {
            name: "_/test_dep_a".to_string(),
            version: "1.0.0".to_string(),
            manifest: test_dep_a_manifest,
            download_url: "dep_a_test.com".to_string(),
            is_top_level_dependency: true,
        };

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

        let test_dep_b = Dependency {
            name: "_/test_dep_b".to_string(),
            version: "2.0.0".to_string(),
            manifest: test_dep_b_manifest,
            download_url: "dep_b_test.com".to_string(),
            is_top_level_dependency: true,
        };

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
            integrity = ""
            hash = ""
            abi = "None"
            entry = dep_a_entry
            [modules."_/test_dep_b"."2.0.0"."test_dep_b_module"]
            name = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.0.0"
            source = "registry+test_dep_b_module"
            resolved = "dep_b_test.com"
            integrity = ""
            hash = ""
            abi = "None"
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

        let test_dep_a = Dependency {
            name: "_/test_dep_a".to_string(),
            version: "1.0.0".to_string(),
            manifest: test_dep_a_manifest,
            download_url: "dep_a_test.com".to_string(),
            is_top_level_dependency: true,
        };

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

        let test_dep_b = Dependency {
            name: "_/test_dep_b".to_string(),
            version: "2.0.0".to_string(),
            manifest: test_dep_b_manifest,
            download_url: "dep_b_test.com".to_string(),
            is_top_level_dependency: true,
        };

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

        let test_dep_b_update = Dependency {
            name: "_/test_dep_b".to_string(),
            version: "2.1.0".to_string(),
            manifest: test_dep_b_manifest_update,
            download_url: "dep_b_test.com".to_string(),
            is_top_level_dependency: true,
        };

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

        let test_dep_c = Dependency {
            name: "_/test_dep_c".to_string(),
            version: "4.0.0".to_string(),
            manifest: test_dep_c_manifest,
            download_url: "dep_c_test.com".to_string(),
            is_top_level_dependency: true,
        };

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
            integrity = ""
            hash = ""
            abi = "None"
            entry = dep_a_entry
            [modules."_/test_dep_b"."2.0.0"."test_dep_b_module"]
            name = "test_dep_b_module"
            package_name = "_/test_dep_b"
            package_version = "2.0.0"
            source = "registry+test_dep_b_module"
            resolved = "dep_b_test.com"
            integrity = ""
            hash = ""
            abi = "None"
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
            integrity = ""
            hash = ""
            abi = "None"
            entry = dep_b_entry
            [modules."_/test_dep_c"."4.0.0"."test_dep_c_module"]
            name = "test_dep_c_module"
            package_name = "_/test_dep_c"
            package_version = "4.0.0"
            source = "registry+test_dep_c_module"
            resolved = "dep_c_test.com"
            integrity = ""
            hash = ""
            abi = "None"
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

        let actual_lockfile_string = toml::to_string(&actual_lockfile).unwrap();
        eprintln!("{}", actual_lockfile_string);
        assert_eq!(expected_lockfile, actual_lockfile);
    }
}
