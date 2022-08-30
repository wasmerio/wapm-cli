//! Code pertaining to the `install` subcommand

use crate::{
    commands::install::get_package_query::GetPackageQueryPackageLastVersion,
    dataflow::bindings::Language, graphql::execute_query,
};

use anyhow::Context;
use graphql_client::*;

use crate::config::Config;
use crate::dataflow;
use crate::util;
use std::{
    borrow::Cow,
    convert::TryInto,
    path::PathBuf,
    process::{Command, Stdio},
};
use std::{convert::TryFrom, path::Path};
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
    #[structopt(long, groups = &["bindings", "js"], conflicts_with = "global")]
    yarn: bool,
    /// Add the JavaScript bindings using "npm install".
    #[structopt(long, groups = &["bindings", "js"], conflicts_with = "global")]
    npm: bool,
    /// Add the package as a dev dependency (JavaScript only)
    #[structopt(long, requires = "js")]
    dev: bool,
    /// Add the Python bindings using "pip install".
    #[structopt(long, group = "bindings", conflicts_with = "global")]
    pip: bool,
    /// The module to install bindings for (useful if a package contains more
    /// than one)
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
    println!("wapm install: start: {options:#?}");
    let current_directory = crate::config::Config::get_current_dir()?;
    let _value = util::set_wapm_should_accept_all_prompts(options.force_yes);
    debug_assert!(
        _value.is_some(),
        "this function should only be called once!"
    );

    match Target::from_options(&options) {
        Some(language) => {
            let result = install_bindings(
                language.clone(),
                &options.packages,
                options.module.as_deref(),
                options.dev,
                current_directory,
            );
            println!("install bindings (language = {language:?}): {result:?}");
            result
        },
        None => {
            let result = wapm_install(options, current_directory);
            println!("install bindings (language = None): {result:?}");
            result
        }
    }
}

fn install_bindings(
    target: Target,
    packages: &[String],
    module: Option<&str>,
    dev: bool,
    current_directory: PathBuf,
) -> Result<(), anyhow::Error> {
    let VersionedPackage { name, version } = match packages {
        [p] => p.as_str().try_into()?,
        [] => anyhow::bail!("No package provided"),
        [..] => anyhow::bail!("Bindings can only be installed for one package at a time"),
    };

    let url =
        dataflow::bindings::link_to_package_bindings(name, version, target.language(), module)?;

    let mut cmd = target.command(url.as_str(), dev);

    // Note: We explicitly want to show the command output to users so they can
    // troubleshoot any failures.
    let status = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(&current_directory)
        .status()
        .with_context(|| {
            format!(
                "Unable to start \"{}\". Is it installed?",
                cmd.get_program().to_string_lossy()
            )
        })?;

    anyhow::ensure!(status.success(), "Command failed: {:?}", cmd);

    Ok(())
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
        packages.push(parse_package_and_version(name)?);
    }

    let installed_packages: Vec<(&str, &str)> = packages
        .iter()
        .map(|(name, version)| (name.as_str(), version.as_str()))
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

    match changes_applied {
        Ok(()) => {
            if global {
                println!("Global package installed successfully!");
            } else {
                println!("Package installed successfully to wapm_packages!");
            }
        },
        Err(e) => {
            if packages.len() == 1 {
                if global {
                    println!("Failed to install package {:?} globally: {e}", packages[0]);
                } else {
                    println!("Failed to install package {:?}: {e}", packages[0]);
                }
            } else {
                if global {
                    println!("Failed to install packages {packages:?} globally: {e}");
                } else {
                    println!("Failed to install packages {packages:?}: {e}");
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct VersionedPackage<'a> {
    name: &'a str,
    version: Option<&'a str>,
}

impl<'a> TryFrom<&'a str> for VersionedPackage<'a> {
    type Error = anyhow::Error;

    fn try_from(package_specifier: &'a str) -> Result<Self, Self::Error> {
        let name_and_version: Vec<_> = package_specifier.split('@').collect();

        match *name_and_version.as_slice() {
            [name, version] => Ok(VersionedPackage {
                name,
                version: Some(version),
            }),
            [name] => Ok(VersionedPackage {
                name,
                version: None,
            }),
            _ => Err(InstallError::InvalidPackageIdentifier {
                name: package_specifier.to_string(),
            }
            .into()),
        }
    }
}

fn parse_package_and_version(package_specifier: &str) -> Result<(String, String), anyhow::Error> {
    let name_and_version: Vec<_> = package_specifier.split('@').collect();

    match name_and_version.as_slice() {
        [name, version] => Ok((name.to_string(), version.to_string())),
        [name] => {
            let q = GetPackageQuery::build_query(get_package_query::Variables {
                name: name.to_string(),
            });
            let response: get_package_query::ResponseData = execute_query(&q)?;
            let package = response.package.ok_or(InstallError::PackageNotFound {
                name: name.to_string(),
            })?;
            let GetPackageQueryPackageLastVersion { version, .. } =
                package
                    .last_version
                    .ok_or(InstallError::NoVersionsAvailable {
                        name: name.to_string(),
                    })?;

            Ok((name.to_string(), version))
        }
        _ => Err(InstallError::InvalidPackageIdentifier {
            name: package_specifier.to_string(),
        }
        .into()),
    }
}

fn local_install_from_lockfile(current_directory: &Path) -> Result<(), anyhow::Error> {
    let added_packages = vec![];
    dataflow::update(added_packages, vec![], current_directory)
        .map_err(|err| InstallError::FailureInstallingPackages(err))?
        .map_err(|e| anyhow!("Error installing package to wapm_packages: {e}"))?;
    println!("Packages installed to wapm_packages!");
    Ok(())
}

#[derive(Debug, Clone)]
enum Target {
    Npm,
    Yarn,
    Pip,
}

impl Target {
    fn from_options(options: &InstallOpt) -> Option<Self> {
        let InstallOpt { yarn, npm, pip, .. } = options;

        match (yarn, npm, pip) {
            (true, false, false) => Some(Target::Yarn),
            (false, true, false) => Some(Target::Npm),
            (false, false, true) => Some(Target::Pip),
            (false, false, false) => None,
            _ => unreachable!("Already rejected by clap"),
        }
    }

    fn language(&self) -> Language {
        match self {
            Target::Npm | Target::Yarn => Language::JavaScript,
            Target::Pip => Language::Python,
        }
    }

    fn command(&self, url: &str, dev: bool) -> Command {
        match self {
            Target::Npm => {
                let mut cmd = Command::new("npm");
                cmd.arg("install");
                if dev {
                    cmd.arg("--save-dev");
                }
                cmd.arg(url);
                cmd
            }
            Target::Yarn => {
                let mut cmd = Command::new("yarn");
                cmd.arg("add");
                if dev {
                    cmd.arg("--dev");
                }
                cmd.arg(url);
                cmd
            }
            Target::Pip => {
                let mut cmd = Command::new("pip");
                cmd.arg("install").arg(url);
                cmd
            }
        }
    }
}
