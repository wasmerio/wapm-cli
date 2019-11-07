//! logic to init a directory for use with wapm

use crate::abi::Abi;
use crate::data::manifest::MANIFEST_FILE_NAME;
use crate::data::manifest::{Command, Manifest, Module, Package};
use crate::util;

use dialoguer::{Confirmation, Input, Select};
use semver::Version;
use std::{
    any::Any,
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const WASI_LAST_VERSION: &str = "0.0.0-unstable";


pub fn ask(prompt: &str, default: Option<String>) -> Result<Option<String>, std::io::Error> {
    let value = Input::<String>::new()
        .with_prompt(prompt)
        .default(default.unwrap_or_default())
        .interact()?;
    if value.is_empty() {
        return Ok(None);
    }
    return Ok(Some(value));
}

pub fn ask_until_valid<F, VR, Err>(
    prompt: &str,
    default: Option<String>,
    validator: F,
) -> Result<VR, std::io::Error>
where
    F: Fn(&str) -> Result<VR, Err>,
    Err: std::fmt::Display,
    VR: Any,
{
    loop {
        let input = ask(prompt, default.clone())?;
        let validated = validator(&input.unwrap_or_default());
        match validated {
            Err(e) => {
                println!("{}", e);
            }
            Ok(v) => {
                return Ok(v);
            }
        }
    }
}

pub fn validate_wasm_source(source: &str) -> Result<PathBuf, String> {
    if source == "none" || source.ends_with(".wasm") {
        return Ok(PathBuf::from(source));
    }
    return Err("The module source path must have a .wasm extension".to_owned());
}

pub fn validate_commands(command_names: &str) -> Result<Vec<String>, util::NameError> {
    command_names
        .split_whitespace()
        .map(util::validate_name)
        .collect()
}

pub fn init(dir: PathBuf, force_yes: bool) -> Result<(), failure::Error> {
    let manifest_location = {
        let mut dir = dir.clone();
        dir.push(MANIFEST_FILE_NAME);
        dir
    };
    let mut manifest = if manifest_location.exists() {
        Manifest::find_in_directory(dir)?
    } else {
        Manifest {
            base_directory_path: dir.clone(),
            fs: None,
            package: Package {
                name: dir
                    .clone()
                    .as_path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                description: "".to_owned(),
                version: Version::parse("1.0.0").unwrap(),
                repository: None,
                // author: None,
                license: Some("ISC".to_owned()),
                license_file: None,
                homepage: None,
                wasmer_extra_flags: None,
                readme: None,
                disable_command_rename: false,
            },
            dependencies: None,
            module: Some(vec![Module {
                name: "entry".to_owned(),
                source: "entry.wasm".into(),
                abi: Abi::default(),
                interfaces: None,
            }]),
            command: None,
        }
    };

    if !force_yes {
        println!(
            "This utility will walk you through creating a wapm.toml file.
It only covers the most common items, and tries to guess sensible defaults.

Use `wapm add <pkg>` afterwards to add a package and
save it as a dependency in the wapm.toml file.

Press ^C at any time to quit."
        );
        manifest.package.name = ask_until_valid(
            "Package name",
            Some(manifest.package.name),
            util::validate_name,
        )?;
        manifest.package.version = ask_until_valid(
            "Version",
            Some(manifest.package.version.to_string()),
            Version::parse,
        )?;
        manifest.package.description =
            ask("Description", Some(manifest.package.description))?.unwrap_or_default();
        manifest.package.repository = ask("Repository", manifest.package.repository)?;
        // author = ask("Author", &author)?;
        manifest.package.license = Some(ask_until_valid(
            "License",
            manifest.package.license,
            util::validate_license,
        )?);
        // Let's reset the modules
        let mut all_modules: Vec<Module> = vec![];
        let mut all_commands: Vec<Command> = vec![];
        let manifest_modules = manifest.module.unwrap_or_default();
        loop {
            let current_index = all_modules.len();
            println!("Enter the data for the Module ({})", current_index + 1);
            let mut module = {
                // We take the data from the current manifest modules
                if manifest_modules.len() > current_index {
                    manifest_modules[current_index].clone()
                } else {
                    Module {
                        name: "".to_owned(),
                        source: PathBuf::from("none"),
                        abi: Abi::default(),
                        interfaces: None,
                    }
                }
            };
            module.source = ask_until_valid(
                " - Source (path)",
                Some(module.source.to_string_lossy().to_string()),
                validate_wasm_source,
            )?;
            if module.source.to_string_lossy() == "none" {
                break;
            }
            // Let's try to guess the name based on the file path
            let default_module_name = Path::new(&module.source)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_string();
            module.name = ask_until_valid(
                " - Name",
                Some(default_module_name.clone()),
                util::validate_name,
            )?;
            let default_module_abi = match module.abi {
                Abi::None => 0,
                Abi::Wasi => 1,
                Abi::Emscripten => 2,
            };
            let (abi, interfaces): (Abi, Option<HashMap<String, String>>) = match Select::new()
                .with_prompt(" - ABI")
                .item("None")
                .item("WASI")
                .item("Emscripten")
                .default(default_module_abi)
                .interact()?
            {
                1 => (
                    Abi::Wasi,
                    Some(
                        [("wasi".to_owned(), WASI_LAST_VERSION.to_owned())]
                            .iter()
                            .cloned()
                            .collect(),
                    ),
                ),
                2 => (Abi::Emscripten, None),
                0 | _ => (Abi::None, None),
            };
            module.abi = abi;
            module.interfaces = interfaces;
            // We ask for commands if it has an Abi
            if !module.abi.is_none() {
                let module_command_strings = ask_until_valid(
                    " - Commmands (space separated)",
                    Some(default_module_name.clone()),
                    validate_commands,
                )?;
                if !module_command_strings.is_empty() {
                    let module_commands =
                        module_command_strings
                            .into_iter()
                            .map(|command_string| Command {
                                name: command_string,
                                module: module.name.clone(),
                                main_args: None,
                                package: None,
                            });
                    all_commands.extend(module_commands);
                }
            }
            all_modules.push(module);
        }
        manifest.module = if all_modules.is_empty() {
            None
        } else {
            Some(all_modules)
        };
        manifest.command = if all_commands.is_empty() {
            None
        } else {
            Some(all_commands)
        };
    }

    let print_text = if force_yes {
        "Wrote to"
    } else {
        "About to write to"
    };

    println!(
        "\n{} {}:\n\n{}\n",
        print_text,
        manifest.manifest_path().canonicalize()?.to_string_lossy(),
        manifest.to_string()?
    );

    if force_yes
        || Confirmation::new()
            .with_text("Is this OK? (yes)")
            .default(true)
            .interact()?
    {
        manifest.save()?;
        #[allow(unused_must_use)]
        {
            init_gitignore(manifest.base_directory_path);
        }
    } else {
        println!("Aborted.")
    }
    Ok(())
}

pub fn init_gitignore(mut dir: PathBuf) -> Result<(), failure::Error> {
    let gitignore = {
        dir.push(".gitignore");
        dir
    };

    let mut f = fs::OpenOptions::new()
        .create(false)
        .read(true)
        .append(true)
        .open(gitignore)?;
    let mut gitignore_str = String::new();
    f.read_to_string(&mut gitignore_str)?;

    // TODO: this doesn't understand gitignores at all, it just checks for an entry
    // use crate that can check if a directory is ignored or not
    for line in gitignore_str.lines() {
        if line.contains("wapm_packages") {
            return Ok(());
        }
    }

    f.write_all(b"\nwapm_packages")?;
    Ok(())
}
