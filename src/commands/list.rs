//! Subcommand for inspecting installed packages and commands

use crate::config;
use crate::data::lock::lockfile::{CommandMap, ModuleMap};
use crate::dataflow::lockfile_packages::LockfileResult;
use prettytable::{format, Table};
use std::{env, fmt::Write as _};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct ListOpt {
    /// List just the globally installed packages
    #[structopt(short = "g", long = "global")]
    global: bool,

    /// List both locally and globally installed packages
    #[structopt(short = "a", long = "all")]
    all: bool,
}

pub fn list(options: ListOpt) -> Result<(), failure::Error> {
    let mut local = false;
    let mut global = false;
    match (options.global, options.all) {
        (_, true) => {
            local = true;
            global = true;
        }
        (true, false) => {
            global = true;
        }
        (false, false) => {
            local = true;
        }
    }

    let mut handle = String::new();
    if local {
        let cwd = env::current_dir()?;
        match LockfileResult::find_in_directory(cwd) {
            LockfileResult::Lockfile(lockfile) => {
                let has_modules = !lockfile.modules.is_empty();
                let has_commands = !lockfile.commands.is_empty();
                // unset local if there's nothing to list
                local = has_modules | has_commands;
                if has_modules {
                    writeln!(handle, "LOCAL PACKAGES:")?;
                    write!(handle, "{}", create_module_ascii_table(&lockfile.modules))?;
                }
                if has_modules && has_commands {
                    writeln!(handle)?;
                }
                if has_commands {
                    writeln!(handle, "LOCAL COMMANDS:")?;
                    write!(handle, "{}", create_command_ascii_table(&lockfile.commands))?;
                }
            }
            LockfileResult::NoLockfile => {
                if !global {
                    writeln!(handle, "No packages in current directory")?;
                    return Ok(());
                }
            }
            LockfileResult::LockfileError(e) => {
                return Err(format_err!(
                    "Failed to read lock file in current directory: {}",
                    e
                ));
            }
        }
    }

    if local && global {
        writeln!(handle)?;
    }

    if global {
        let global_path = config::Config::get_globals_directory()?;
        match LockfileResult::find_in_directory(global_path) {
            LockfileResult::Lockfile(lockfile) => {
                let has_modules = !lockfile.modules.is_empty();
                let has_commands = !lockfile.commands.is_empty();
                // unset global if there's nothing to list
                global = has_modules || has_commands;
                if has_modules {
                    writeln!(handle, "GLOBAL PACKAGES:")?;
                    write!(handle, "{}", create_module_ascii_table(&lockfile.modules))?;
                }
                if has_modules && has_commands {
                    writeln!(handle)?;
                }
                if has_commands {
                    writeln!(handle, "GLOBAL COMMANDS:")?;
                    write!(handle, "{}", create_command_ascii_table(&lockfile.commands))?;
                }
            }
            LockfileResult::NoLockfile => {
                if !local {
                    writeln!(handle, "No global packages")?;
                    return Ok(());
                }
            }
            LockfileResult::LockfileError(e) => {
                return Err(format_err!(
                    "Failed to read lock file in current directory: {}",
                    e
                ));
            }
        }
    }

    if handle.is_empty() {
        // we should only have an empty string if we unset both global and local
        debug_assert!(!global);
        debug_assert!(!local);
        println!("No packages found");
    } else {
        print!("{}", handle);
    }
    Ok(())
}

fn create_module_ascii_table(modules: &ModuleMap) -> String {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row!["PACKAGE", "VERSION", "MODULE", "ABI"]);
    for (package_name, version_info) in modules.iter() {
        for (version_number, module_info) in version_info.iter() {
            for (module_name, module) in module_info.iter() {
                table.add_row(row![package_name, version_number, module_name, module.abi,]);
            }
        }
    }
    format!("{}", table)
}

fn create_command_ascii_table(commands: &CommandMap) -> String {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
    table.add_row(row!["COMMAND", "PACKAGE", "VERSION"]);
    for (command_name, command) in commands.iter() {
        table.add_row(row![
            command_name,
            command.package_name,
            command.package_version,
        ]);
    }
    format!("{}", table)
}
