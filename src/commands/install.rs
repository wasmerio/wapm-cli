//! Code pertaining to the `install` subcommand

use crate::graphql::execute_query;

use graphql_client::*;

use crate::config::Config;
use crate::dataflow;
use crate::util;
use std::{borrow::Cow, path::PathBuf};
use std::{path::Path, str::FromStr};
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
    /// Add the JavaScript bindings using "yarn add".
    #[structopt(long, group = "bindings", conflicts_with = "global")]
    yarn: bool,
    /// Add the JavaScript bindings using "npm install".
    #[structopt(long, group = "bindings", conflicts_with = "global")]
    npm: bool,
    /// Add the JavaScript package using yarn.
    #[structopt(long, group = "bindings", conflicts_with = "global")]
    python: bool,
    #[structopt(long, requires = "bindings")]
    module: Option<String>,
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

    match Bindings::from_options(&options) {
        Some(language) => install_bindings(
            language,
            &options.packages,
            options.module.as_deref(),
            current_directory,
        ),
        None => wapm_install(options, current_directory),
    }
}

fn install_bindings(
    language: Bindings,
    packages: &[String],
    module: Option<&str>,
    current_directory: PathBuf,
) -> Result<(), anyhow::Error> {
    let package = match packages {
        [p] => p.as_str(),
        [] => anyhow::bail!("No package provided"),
        [..] => anyhow::bail!("Bindings can only be installed for one package at a time"),
    };

    todo!()
}

fn wapm_install(options: InstallOpt, current_directory: PathBuf) -> Result<(), anyhow::Error> {
    match (options.global, options.packages.is_empty()) {
        (global_flag::GLOBAL_INSTALL, package_args::NO_PACKAGES) => {
            // install all global packages - unacceptable use case
            Err(InstallError::MustSupplyPackagesWithGlobalFlag.into())
        }
        (global_flag::LOCAL_INSTALL, package_args::NO_PACKAGES) => {
            local_install_from_lockfile(&current_directory)
        }
        (_, package_args::SOME_PACKAGES) => {
            install_packages(&options.packages, options.global, current_directory)
        }
    }
}

fn install_packages(
    package_names: &[String],
    global: bool,
    current_directory: PathBuf,
) -> Result<(), anyhow::Error> {
    let mut packages = vec![];
    for name in package_names {
        packages.push(VersionedPackage::from_str(name)?);
    }

    let installed_packages: Vec<(&str, &str)> = packages
        .iter()
        .map(|pkg| (pkg.name.as_str(), pkg.version.as_str()))
        .collect();

    // the install directory will determine which wapm.lock we are updating. For now, we
    // look in the local directory, or the global install directory
    let install_directory: Cow<Path> = match global {
        true => {
            let folder = Config::get_globals_directory()?;
            Cow::Owned(folder)
        }
        false => Cow::Borrowed(&current_directory),
    };

    std::fs::create_dir_all(install_directory.clone())
        .map_err(|err| InstallError::CannotCreateInstallDirectory(err))?;
    let changes_applied = dataflow::update(installed_packages.clone(), vec![], install_directory)
        .map_err(|err| InstallError::CannotRegenLockFile(err))?;

    if changes_applied {
        if global {
            println!("Global package installed successfully!");
        } else {
            println!("Package installed successfully to wapm_packages!");
        }
    } else {
        println!("No packages to install");
    }

    Ok(())
}

#[derive(Debug)]
struct VersionedPackage {
    name: String,
    version: String,
}

impl FromStr for VersionedPackage {
    type Err = anyhow::Error;

    fn from_str(package_specifier: &str) -> Result<Self, Self::Err> {
        let name_and_version: Vec<_> = package_specifier.split('@').collect();

        match name_and_version.as_slice() {
            [name, version] => Ok(VersionedPackage {
                name: name.to_string(),
                version: version.to_string(),
            }),
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
                Ok(VersionedPackage {
                    name: package_name,
                    version: package_version,
                })
            }
            _ => Err(InstallError::InvalidPackageIdentifier {
                name: package_specifier.to_string(),
            }
            .into()),
        }
    }
}

fn local_install_from_lockfile(current_directory: &Path) -> Result<(), anyhow::Error> {
    let added_packages = vec![];
    dataflow::update(added_packages, vec![], current_directory)
        .map_err(|err| InstallError::FailureInstallingPackages(err))?;
    println!("Packages installed to wapm_packages!");
    Ok(())
}

#[derive(Debug)]
enum Bindings {
    Npm,
    Yarn,
    Python,
}

impl Bindings {
    fn from_options(options: &InstallOpt) -> Option<Self> {
        let InstallOpt {
            yarn, npm, python, ..
        } = options;

        match (yarn, npm, python) {
            (true, false, false) => Some(Bindings::Yarn),
            (false, true, false) => Some(Bindings::Npm),
            (false, false, true) => Some(Bindings::Python),
            (false, false, false) => None,
            _ => unreachable!("Already rejected by clap"),
        }
    }
}
