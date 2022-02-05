//! Code pertaining to the `install` subcommand

use crate::graphql::execute_query;

use graphql_client::*;

use crate::config::Config;
use crate::dataflow;
use crate::util;
use std::borrow::Cow;
use std::path::Path;
use structopt::StructOpt;
use thiserror::Error;

/// Options for the `install` subcommand
#[derive(StructOpt, Debug)]
pub struct InstallOpt {
    packages: Vec<String>,
    /// Install the package(s) globally
    #[structopt(short = "g", long = "global")]
    global: bool,
    /// Agree to all prompts. Useful for non-interactive uses. (WARNING: this may cause undesired behavior)
    #[structopt(long = "force-yes", short = "y")]
    force_yes: bool,
}

#[derive(Debug, Error)]
enum InstallError {
    #[error("Package not found in the registry: {name}")]
    PackageNotFound { name: String },

    #[error("No package versions available for package {name}")]
    NoVersionsAvailable { name: String },

    #[error("Failed to install packages. {0}")]
    CannotRegenLockFile(dataflow::Error),

    #[error("Failed to create the install directory. {0}")]
    CannotCreateInstallDirectory(std::io::Error),

    #[error("Failed to install packages in manifest. {0}")]
    FailureInstallingPackages(dataflow::Error),

    #[error(
        "Failed to install package because package identifier {0} is invalid, expected <name>@<version> or <name>",
        name
    )]
    InvalidPackageIdentifier { name: String },
    #[error("Must supply package names to install command when using --global/-g flag.")]
    MustSupplyPackagesWithGlobalFlag,
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_package.graphql",
    response_derives = "Debug"
)]
struct GetPackageQuery;

mod global_flag {
    pub const GLOBAL_INSTALL: bool = true;
    pub const LOCAL_INSTALL: bool = false;
}

mod package_args {
    /// Command run with no package arguments, it will install packages from the manifest
    pub const NO_PACKAGES: bool = true;
    pub const SOME_PACKAGES: bool = false;
}

/// Run the install command
pub fn install(options: InstallOpt) -> anyhow::Result<()> {
    let current_directory = crate::config::Config::get_current_dir()?;
    let _value = util::set_wapm_should_accept_all_prompts(options.force_yes);
    debug_assert!(
        _value.is_some(),
        "this function should only be called once!"
    );

    match (options.global, options.packages.is_empty()) {
        (global_flag::GLOBAL_INSTALL, package_args::NO_PACKAGES) => {
            // install all global packages - unacceptable use case
            return Err(InstallError::MustSupplyPackagesWithGlobalFlag.into());
        }
        (global_flag::LOCAL_INSTALL, package_args::NO_PACKAGES) => {
            // install all packages locally
            let added_packages = vec![];
            dataflow::update(added_packages, vec![], &current_directory)
                .map_err(|err| InstallError::FailureInstallingPackages(err))?;
            println!("Packages installed to wapm_packages!");
        }
        (_, package_args::SOME_PACKAGES) => {
            let mut packages = vec![];
            for name in options.packages {
                let name_with_version: Vec<&str> = name.split("@").collect();

                match &name_with_version[..] {
                    [package_name, package_version] => {
                        packages.push((package_name.to_string(), package_version.to_string()));
                    }
                    [name] => {
                        let q = GetPackageQuery::build_query(get_package_query::Variables {
                            name: name.to_string(),
                        });
                        let response: get_package_query::ResponseData = execute_query(&q)?;
                        let package = response.package.ok_or(InstallError::PackageNotFound {
                            name: name.to_string(),
                        })?;
                        let last_version =
                            package
                                .last_version
                                .ok_or(InstallError::NoVersionsAvailable {
                                    name: name.to_string(),
                                })?;
                        let package_name = package.name.clone();
                        let package_version = last_version.version.clone();
                        packages.push((package_name, package_version));
                    }
                    _ => {
                        return Err(
                            InstallError::InvalidPackageIdentifier { name: name.clone() }.into(),
                        );
                    }
                }
            }

            let installed_packages: Vec<(&str, &str)> = packages
                .iter()
                .map(|(s1, s2)| (s1.as_str(), s2.as_str()))
                .collect();

            // the install directory will determine which wapm.lock we are updating. For now, we
            // look in the local directory, or the global install directory
            let install_directory: Cow<Path> = match options.global {
                true => {
                    let folder = Config::get_globals_directory()?;
                    Cow::Owned(folder)
                }
                false => Cow::Borrowed(&current_directory),
            };
            std::fs::create_dir_all(install_directory.clone())
                .map_err(|err| InstallError::CannotCreateInstallDirectory(err))?;

            let changes_applied =
                dataflow::update(installed_packages.clone(), vec![], install_directory)
                    .map_err(|err| InstallError::CannotRegenLockFile(err))?;

            if changes_applied {
                if options.global {
                    println!("Global package installed successfully!");
                } else {
                    println!("Package installed successfully to wapm_packages!");
                }
            } else {
                println!("No packages to install")
            }
        }
    }
    Ok(())
}
