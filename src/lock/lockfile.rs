use crate::dependency_resolver::PackageRegistryLike;
use crate::lock::lockfile_command::LockfileCommand;
use crate::lock::lockfile_module::LockfileModule;
use crate::lock::{LOCKFILE_HEADER, LOCKFILE_NAME};
use crate::manifest::Manifest;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

//type ModuleMap<'a> = BTreeMap<&'a str, BTreeMap<&'a str, BTreeMap<&'a str, LockfileModule>>>;
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
    pub fn read_lockfile_string<P: AsRef<Path>>(directory: P) -> Result<String, failure::Error> {
        let lockfile_path = directory.as_ref().join(LOCKFILE_NAME);
        let mut lockfile_file = File::open(lockfile_path)?;
        let mut lockfile_string = String::new();
        lockfile_file.read_to_string(&mut lockfile_string)?;
        Ok(lockfile_string)
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
        let dependencies = dependency_resolver.get_all_dependencies(&manifest.package.name, &manifest.package.version, unresolved_dependencies)?;
        for dependency in dependencies.iter() {
            let package_name = dependency.manifest.package.name.as_str();
            let package_version = dependency.manifest.package.version.as_str();
            let lockfile_modules_vec = LockfileModule::from_dependency(*dependency)?;
            for lockfile_module in lockfile_modules_vec.into_iter() {
                let module_name = lockfile_module.name.clone();
                let version_map = lockfile_modules.entry(package_name).or_insert(BTreeMap::new());
                let module_map = version_map.entry(package_version).or_insert(BTreeMap::new());
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
                let lockfile_command = LockfileCommand::from_command(pkg_name, pkg_version, command);
                lockfile_commands.insert(&command.name, lockfile_command);
            }
        }

        let new_lockfile = Lockfile {
            modules: lockfile_modules,
            commands: lockfile_commands,
        };
        Ok(new_lockfile)
    }

    pub fn new_from_manifest_and_lockfile<D: PackageRegistryLike>(manifest: &'a Manifest, existing_lockfile: &'a Lockfile, resolver: &'a mut D) -> Result<Self, failure::Error> {
        // TODO re-implement for multiple modules
        Self::new_from_manifest(manifest, resolver)
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

    pub fn get_module(&self, package_name: &str, package_version: &str, module_name: &str) -> Result<&LockfileModule, failure::Error> {
        let version_map = self.modules.get(package_name).ok_or::<failure::Error>(LockfileError::ModuleNotFound(module_name.to_string()).into())?;
        let module_map = version_map.get(package_version).ok_or::<failure::Error>(LockfileError::ModuleNotFound(module_name.to_string()).into())?;
        let module = module_map.get(module_name).ok_or::<failure::Error>(LockfileError::ModuleNotFound(module_name.to_string()).into())?;
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

// /// This helper function resolves differences between the lockfile and the manifest file. All changes
// /// that have not been reflected in the lockfile are returned as a vec of package names and versions.
// /// The packages that had no changes are returned as references to the the lockfile modules.
//fn resolve_changes<'b>(
//    manifest: &'b Manifest,
//    lockfile_modules: &BTreeMap<String, LockfileModule>,
//) -> Result<(Vec<(&'b str, &'b str)>, BTreeMap<String, LockfileModule>), failure::Error> {
//    let (changes, not_changed) = match manifest.dependencies {
//        Some(ref dependencies) => {
//            let mut changes = vec![];
//            let mut not_changed = BTreeMap::new();
//            let dependencies = extract_dependencies(dependencies)?;
//            for (name, version) in dependencies.iter() {
//                let key = format!("{} {}", name, version);
//                match lockfile_modules.get(&key) {
//                    Some(lockfile_module) => {
//                        not_changed.insert(key, lockfile_module.clone());
//                    }
//                    None => changes.push((*name, *version)),
//                }
//            }
//            (changes, not_changed)
//        }
//        None => (vec![], BTreeMap::new()),
//    };
//    Ok((changes, not_changed))
//}

// /// dependencies that are unchanged remain in the BTreeMap. A vec of string refs are returned which
// /// are the package name and the version which is different or new. The strings live as long as the
// /// dependencies vec.
//fn better_resolve_changes<'dependencies>(
//    dependencies: &'dependencies Vec<(&'dependencies str, &'dependencies str)>,
//    lockfile_modules: &mut ModuleMap<'dependencies>,
//) -> Vec<(&'dependencies str, &'dependencies str)> {
//    let mut changes = vec![];
//    for (name, version) in dependencies {
//        match lockfile_modules.get(*name) {
//            Some(ref modules) if !modules.contains_key(*version) => {
//                lockfile_modules.remove(*name);
//                changes.push((*name, *version));
//            }
//            None => {
//                changes.push((*name, *version));
//            }
//            Some(_) => {}
//        }
//    }
//    changes
//}

//fn get_lockfile_data_from_dependency<'a>(
//    dependency: &'a Dependency,
//    lockfile_modules: &'a mut ModuleMap<'a>,
//    lockfile_commands: &'a mut CommandMap<'a>,
//) {
//    let manifest = &dependency.manifest;
//    let package_name = &manifest.package.name;
//    let package_version = &manifest.package.version;
//    let download_url = dependency.download_url.as_str();
//    match manifest.module {
//        Some(ref modules) => {
//            for module in modules {
//                let name = &dependency.name;
//                let mut version_map = match lockfile_modules.get(dependency.manifest.package.name.as_str()) {
//                    Some(version_map) => {
//                        version_map
//                    },
//                    None => {
//                        let version_map = BTreeMap::new();
//                        lockfile_modules.insert(&dependency.manifest.package.name, version_map);
//                        lockfile_modules.get(dependency.manifest.package.name.as_str()).unwrap()
//                    }
//                };
//                let mut module_map = match version_map.get(dependency.manifest.package.version.as_str()) {
//                    Some(module_map) => module_map,
//                    None => {
//                        let module_map = BTreeMap::new();
//                        version_map.insert(dependency.manifest.package.version.as_str(), module_map);
//                        version_map.get(dependency.manifest.package.version.as_str()).unwrap()
//                    }
//                };
//                let lockfile_module =
//                    LockfileModule::from_module(package_name, package_version, module, download_url);
//                module_map.insert(module.name.as_str(), lockfile_module);
//            }
//            // if there is a module, then get the commands if any exist
//            match manifest.command {
//                Some(ref commands) => {
//                    for command in commands {
//                        let lockfile_command = LockfileCommand::from_command(&dependency.manifest.package.name,&dependency.manifest.package.version, command);
//                        lockfile_commands.insert(&command.name, lockfile_command);
//                    }
//                }
//                None => {}
//            }
//        }
//        None => {}
//    }
//}

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

//#[cfg(test)]
//mod get_lockfile_data_from_manifest_tests {
//    use crate::dependency_resolver::Dependency;
//    use crate::lock::lockfile::get_lockfile_data_from_dependency;
//    use crate::manifest::Manifest;
//    use std::collections::BTreeMap;
//
//    #[test]
//    fn fill_lockfile_data() {
//        let mut lockfile_modules = BTreeMap::new();
//        let mut lockfile_commands = BTreeMap::new();
//        let foo_toml: toml::Value = toml! {
//            [package]
//            name = "ns/foo"
//            description = "foo in the ns namespace"
//            version = "1.0.0"
//            [[module]]
//            name = "foo"
//            source = "foo.wasm"
//            [[command]]
//            module = "foo"
//            name = "do_foo_stuff"
//            [[command]]
//            module = "foo"
//            name = "do_other_stuff"
//        };
//        let foo_manifest: Manifest = foo_toml.try_into().unwrap();
//        let dependency = Dependency {
//            name: "foo".to_string(),
//            manifest: foo_manifest,
//            download_url: "".to_string(),
//        };
//        get_lockfile_data_from_dependency(
//            &dependency,
//            &mut lockfile_modules,
//            &mut lockfile_commands,
//        );
//        assert_eq!(1, lockfile_modules.len());
//        assert_eq!(2, lockfile_commands.len());
//    }
//}

//#[cfg(test)]
//mod resolve_changes_tests {
//    use crate::lock::lockfile::{resolve_changes, Lockfile};
//    use crate::manifest::Manifest;
//
//    #[test]
//    fn lock_file_exists_and_one_unchanged_dependency() {
//        let wapm_toml = toml! {
//            [package]
//            name = "ns/test"
//            version = "1.0.0"
//            description = "test package"
//            [[module]]
//            name = "test"
//            source = "target.wasm"
//            description = "description"
//            [dependencies]
//            "abc/foo" = "1.0.0"
//            "xyz/bar" = "2.0.1"
//        };
//        let manifest: Manifest = wapm_toml.try_into().unwrap();
//        let wapm_lock_toml = toml! {
//            [modules."abc/foo 1.0.0 foo"]
//            name = "foo"
//            version = "1.0.0"
//            source = ""
//            resolved = ""
//            integrity = ""
//            hash = ""
//            abi = "None"
//            entry = "target.wasm"
//            [modules."xyz/bar 3.0.0"]
//            name = "bar"
//            version = "3.0.0" // THIS CHANGED!
//            source = ""
//            resolved = ""
//            integrity = ""
//            hash = ""
//            abi = "None"
//            entry = "target.wasm"
//            [commands]
//        };
//        let lockfile: Lockfile = wapm_lock_toml.try_into().unwrap();
//        let lockfile_modules = lockfile.modules;
//        let (changes, not_changed) = resolve_changes(&manifest, &lockfile_modules).unwrap();
//        assert_eq!(1, changes.len()); // one dependency was upgraded
//        assert_eq!(1, not_changed.len()); // one dependency did not change, reuse the lockfile module
//    }
//}
//
//#[cfg(test)]
//mod test {
//    use crate::dependency_resolver::{Dependency, TestResolver};
//    use crate::lock::lockfile::Lockfile;
//    use crate::lock::LOCKFILE_NAME;
//    use crate::manifest::{Manifest, MANIFEST_FILE_NAME};
//    use std::collections::BTreeMap;
//    use std::fs::File;
//    use std::io::Write;
//
//    #[test]
//    fn create_from_manifest() {
//        let tmp_dir = tempdir::TempDir::new("create_from_manifest").unwrap();
//        let wapm_toml = toml! {
//            [package]
//            description = "description"
//            version = "1.0.0"
//            [[module]]
//            name = "test"
//            source = "test.wasm"
//        };
//        let manifest_path = tmp_dir.path().join(MANIFEST_FILE_NAME);
//        let mut file = File::create(&manifest_path).unwrap();
//        let toml_string = toml::to_string(&wapm_toml).unwrap();
//        file.write_all(toml_string.as_bytes()).unwrap();
//        let manifest = Manifest::open(manifest_path).unwrap();
//        let resolver = TestResolver(BTreeMap::new());
//        let lockfile = Lockfile::new_from_manifest(&manifest, &resolver).unwrap();
//        assert_eq!(0, lockfile.commands.len());
//        assert_eq!(0, lockfile.modules.len());
//    }
//    #[test]
//    fn create_from_manifest_and_existing_lockfile_with_dependencies_and_commands() {
//        let wapm_toml = toml! {
//            [package]
//            name = "test"
//            version = "1.0.0"
//            description = "description"
//            [[module]]
//            name = "test"
//            source = "test.wasm"
//            [dependencies]
//            foo = "1.0.2"
//            bar = "3.0.0"
//        };
//        let manifest: Manifest = wapm_toml.try_into().unwrap();
//
//        // setup resolver
//        let mut map = BTreeMap::new();
//        // FOO package v 1.0.0
//        let foo_toml: toml::Value = toml! {
//            [package]
//            name = "foo"
//            version = "1.0.0"
//            description = ""
//            [[module]]
//            name = "foo"
//            source = "foo.wasm"
//            [[command]]
//            name = "do_foo_stuff"
//        };
//        let foo_manifest: Manifest = foo_toml.try_into().unwrap();
//        let foo_dependency = Dependency {
//            name: "foo".to_string(),
//            manifest: foo_manifest,
//            download_url: "".to_string(),
//        };
//        // FOO package v 1.0.2
//        map.insert(("foo".to_string(), "1.0.2".to_string()), foo_dependency);
//        let newer_foo_toml: toml::Value = toml! {
//            [package]
//            name = "foo"
//            version = "1.0.2"
//            description = ""
//            [[module]]
//            name = "foo"
//            source = "foo.wasm"
//            [[command]]
//            name = "do_more_foo_stuff" // COMMAND REMOVED AND ADDED
//        };
//        let newer_foo_manifest: Manifest = newer_foo_toml.try_into().unwrap();
//        let newer_foo_dependency = Dependency {
//            name: "foo".to_string(),
//            manifest: newer_foo_manifest,
//            download_url: "".to_string(),
//        };
//        map.insert(
//            ("foo".to_string(), "1.0.2".to_string()),
//            newer_foo_dependency,
//        );
//        // BAR package v 2.0.1
//        let bar_toml: toml::Value = toml! {
//            [package]
//
//            [[module]]
//            name = "bar"
//            version = "2.0.1"
//            module = "bar.wasm"
//            description = ""
//        };
//        let bar_manifest: Manifest = bar_toml.try_into().unwrap();
//        let bar_dependency = Dependency {
//            name: "foo".to_string(),
//            manifest: bar_manifest,
//            download_url: "".to_string(),
//        };
//        map.insert(("bar".to_string(), "2.0.1".to_string()), bar_dependency);
//        // BAR package v 3.0.0
//        let bar_newer_toml: toml::Value = toml! {
//            [package]
//            name = "bar"
//            version = "3.0.0"
//            description = ""
//            [[module]]
//            name = "bar"
//            module = "bar.wasm"
//            [[command]]
//            name = "do_bar_stuff" // ADDED COMMAND
//        };
//        let bar_newer_manifest: Manifest = bar_newer_toml.try_into().unwrap();
//        let bar_newer_dependency = Dependency {
//            name: "foo".to_string(),
//            manifest: bar_newer_manifest,
//            download_url: "".to_string(),
//        };
//        map.insert(
//            ("bar".to_string(), "3.0.0".to_string()),
//            bar_newer_dependency,
//        );
//        let test_resolver = TestResolver(map);
//
//        // existing lockfile
//        let wapm_lock_toml = toml! {
//            [modules."abc/foo"."1.0.0"."foo_module"]
//            name = "foo_module"
//            version = "1.0.0"
//            source = "registry+foo"
//            resolved = ""
//            integrity = ""
//            hash = ""
//            abi = "None"
//            entry = "foo.wasm"
//
//            [modules."xyz/bar"."2.0.1"."bar_module"]
//            name = "bar_module"
//            version = "2.0.1"
//            source = "registry+bar"
//            resolved = ""
//            integrity = ""
//            hash = ""
//            abi = "None"
//            entry = "bar.wasm"
//
//            [commands.do_foo_stuff]
//            module = "foo_module"
//            package_name = "abc/foo"
//            package_version = "1.0.0"
//        };
//
//        let existing_lockfile: Lockfile = wapm_lock_toml.try_into().unwrap();
//
//        let lockfile =
//            Lockfile::new_from_manifest_and_lockfile(&manifest, existing_lockfile, &test_resolver)
//                .unwrap();
//
//        // existing lockfile
//        let expected_lock_toml = toml! {
//            [modules."abc/foo"."1.0.2"."foo_module"]
//            name = "foo_module"
//            version = "1.0.2"
//            source = "registry+foo"
//            resolved = ""
//            integrity = ""
//            hash = ""
//            abi = "None"
//            entry = "foo.wasm"
//
//            [modules."xyz/bar"."3.0.0"."bar_module"]
//            name = "bar_module"
//            version = "3.0.0"
//            source = "registry+bar"
//            resolved = ""
//            integrity = ""
//            hash = ""
//            abi = "None"
//            entry = "bar.wasm"
//
//            [commands.do_more_foo_stuff]
//            package_name = "abc/foo"
//            package_version = "1.0.2"
//            module = "foo_module"
//
//            [commands.do_bar_stuff]
//            package_name = "xyz/bar"
//            package_version = "1.0.2"
//            module = "bar_module"
//        };
//
//        let expected_lockfile: Lockfile = expected_lock_toml.try_into().unwrap();
//
//        assert_eq!(expected_lockfile, lockfile);
//    }
//}
