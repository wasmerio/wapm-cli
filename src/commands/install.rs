//! Code pertaining to the `install` subcommand

use crate::{
    commands::install::get_package_query::GetPackageQueryPackageLastVersion,
    dataflow::bindings::Language, graphql::execute_query,
};

use anyhow::Context;
use graphql_client::*;
use wapm_resolve_url::get_pirita_url_of_package;

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
    pub(crate) packages: Vec<String>,
    /// Install the package(s) globally
    #[structopt(short = "g", long = "global")]
    pub(crate) global: bool,
    /// If packages already exist, the CLI will throw a prompt whether you'd like to
    /// re-download the package. This flag disables the prompt and will re-download
    /// the file even if it already exists.
    #[structopt(long = "nocache")]
    pub(crate) nocache: bool,
    /// Agree to all prompts. Useful for non-interactive uses. (WARNING: this may cause undesired behavior)
    #[structopt(long = "force-yes", short = "y")]
    pub(crate) force_yes: bool,
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
    #[cfg(feature = "pirita_file")]
    #[error(
        "Could not find PiritaFile download url for package {0}@{1}",
        name,
        version
    )]
    NoPiritaFileForPackage { name: String, version: String },
}

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
    #[cfg(feature = "pirita_file")]
    if std::env::var("USE_PIRITA").ok() == Some("1".to_string()) {
        return install_pirita(&options);
    }
    let current_directory = crate::config::Config::get_current_dir()?;
    let _value = util::set_wapm_should_accept_all_prompts(options.force_yes);
    debug_assert!(
        _value.is_some(),
        "this function should only be called once!"
    );

    match Target::from_options(&options) {
        Some(language) => install_bindings(
            language,
            &options.packages,
            options.module.as_deref(),
            options.dev,
            current_directory,
        ),
        None => wapm_install(options, current_directory),
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

    if changes_applied {
        if global {
            println!("Global package installed successfully!");
        } else {
            println!("Package installed successfully to wapm_packages!");
        }
    } else {
        println!("No packages to install (package already installed?)");
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
        .map_err(|err| InstallError::FailureInstallingPackages(err))?;
    println!("Packages installed to wapm_packages!");
    Ok(())
}

#[derive(Debug)]
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

fn get_packages_with_versions(package_args: &[String]) -> anyhow::Result<Vec<WapmDistribution>> {

    use wapm_resolve_url::get_tar_gz_url_of_package;
    use url::Url;

    let config = Config::from_file()?;
    let registry_url = Url::parse(&config.registry.get_graphql_url())?;

    let mut result = vec![];
    for name in package_args {

        let name_with_version: Vec<&str> = name.split("@").collect();

        let mut package_name = match &name_with_version[..] {
            [package_name, _] => Some(package_name.to_string()),
            [package_name] => Some(package_name.to_string()),
            _ => None,
        }
        .ok_or(InstallError::InvalidPackageIdentifier { name: name.clone() })?;

        let mut package_version = match &name_with_version[..] {
            [_, version] => Some(version.to_string()),
            _ => None,
        };

        use crate::commands::execute::{WaxGetCommandQuery, wax_get_command_query};
        let get_wax_package_name = |name: String| {
            let q = WaxGetCommandQuery::build_query(wax_get_command_query::Variables {
                command: name,
            });
            debug!("Querying server for package info");
            let response: Result<wax_get_command_query::ResponseData, _> = execute_query(&q);
            match response {
                Ok(o) => Some((
                    o.command.as_ref()?.package_version.package.name.to_string(), 
                    o.command.as_ref()?.package_version.version.to_string(),
                )),
                Err(_) => None,
            }
        };

        let pv = package_version.clone();
        let pv = pv.as_ref().map(|s| s.as_str());
        let targz_info = match get_tar_gz_url_of_package(&registry_url, &package_name, pv) {
            Some(s) => s,
            None => {
                if let Some((wax_package_name, wax_package_version)) = get_wax_package_name(package_name) {
                    package_name = wax_package_name.clone();
                    package_version = Some(wax_package_version.clone());
                    get_tar_gz_url_of_package(&registry_url, &wax_package_name, Some(&wax_package_version))
                    .ok_or(InstallError::PackageNotFound {
                        name: name.to_string(),
                    })?
                } else {
                    return Err(InstallError::PackageNotFound {
                        name: name.to_string(),
                    }.into());
                }
            }
        };

        let pirita_info = get_pirita_url_of_package(&registry_url, &package_name, Some(&targz_info.resolved_version));

        let package_to_download = WapmDistribution {
            name: targz_info.resolved_name.to_string(),
            version: targz_info.resolved_version.to_string(),
            download_url: format!("{}", targz_info.url),
            pirita_download_url: pirita_info.map(|i| format!("{}", i.url)),
            is_last_version: package_version.is_none(),
        };

        result.push(package_to_download.clone());
    }

    Ok(result)
}

/// Run the install command with --pirita flags
#[cfg(feature = "pirita_file")]
pub fn install_pirita(options: &InstallOpt) -> anyhow::Result<()> {

    let current_directory = crate::config::Config::get_current_dir()?;
    let _value = util::set_wapm_should_accept_all_prompts(options.force_yes);
    debug_assert!(
        _value.is_some(),
        "this function should only be called once!"
    );

    let installed_packages = get_packages_with_versions(&options.packages);
    let installed_packages = installed_packages?;
    let install_directory = Path::new(&current_directory);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        for p in installed_packages {
            
            match p.pirita_download_url.as_ref() {
                Some(pirita_url) => {
                    println!("downloading {pirita_url}");
                    if let Err(_) = download_pirita(
                        &p.name,
                        &p.version,
                        &pirita_url,
                        false,
                        &install_directory,
                        options.nocache || options.force_yes,
                    )
                    .await {
                        println!("downloading with autoconvert!");
                        download_pirita(
                            &p.name,
                            &p.version,
                            &p.download_url,
                            true, // autoconvert .tar.gz -> .pirita
                            &install_directory,
                            options.nocache || options.force_yes,
                        )
                        .await?;
                    }
                },
                None => {
                    download_pirita(
                        &p.name,
                        &p.version,
                        &p.download_url,
                        true, // autoconvert .tar.gz -> .pirita
                        &install_directory,
                        options.nocache || options.force_yes,
                    )
                    .await?;
                }
            }
        }
        Ok(())
    })
}

#[cfg(feature = "pirita_file")]
async fn download_pirita(
    name: &str,
    version: &str,
    download_url: &str,
    autoconvert: bool,
    directory: &Path,
    nocache: bool,
) -> Result<(String, PathBuf, String), anyhow::Error> {
    use crate::dataflow::installed_packages::Error;
    use crate::graphql::VERSION;
    #[cfg(not(target_os = "wasi"))]
    use crate::proxy;
    use crate::util::{
        create_package_dir, create_temp_dir, fully_qualified_package_display_name,
        get_package_namespace_and_name, whoami_distro,
    };
    use dialoguer::Confirm;
    use indicatif::{ProgressBar, ProgressStyle};
    use reqwest::{header, ClientBuilder};
    use std::fs::OpenOptions;
    use std::io::Write;

    let version = semver::Version::parse(version)
        .map_err(|e| anyhow!("Invalid version for package {name:?}: {version:?}: {e}"))?;

    let key = format!("{name}@{version}");
    let (namespace, pkg_name) = get_package_namespace_and_name(name)
        .map_err(|e| Error::FailedToParsePackageName(name.to_string(), e.to_string()))?;

    let fully_qualified_package_name: String =
        fully_qualified_package_display_name(pkg_name, &version);
    let package_dir = create_package_dir(&directory, namespace, &fully_qualified_package_name)
        .map_err(|err| Error::IoErrorCreatingDirectory(key.to_string(), err.to_string()))?;
    let target_file_path = package_dir.join("package.pirita");

    let client = {
        let builder = ClientBuilder::new().gzip(true);
        #[cfg(not(target_os = "wasi"))]
        let builder = if let Some(proxy) =
            proxy::maybe_set_up_proxy().map_err(|e| Error::IoConnectionError(format!("{}", e)))?
        {
            builder.proxy(proxy)
        } else {
            builder
        };

        builder.build().unwrap()
    };
    let user_agent = format!(
        "wapm/{} {} {}",
        VERSION,
        whoami::platform(),
        whoami_distro(),
    );

    let mut response = client
        .get(download_url)
        .header(header::USER_AGENT, user_agent)
        .send()
        .await
        .map_err(|e| {
            let error_message = e.to_string();
            #[cfg(feature = "telemetry")]
            {
                let e = e.into();
                sentry::integrations::anyhow::capture_anyhow(&e);
            }
            Error::DownloadError(key.to_string(), error_message)
        })?;

    let total_size: u64 = response
        .headers()
        .get("Content-Length")
        .and_then(|c| c.to_str().ok()?.parse().ok())
        .unwrap_or(u64::MAX);

    let temp_dir =
    create_temp_dir()
    .map_err(|e| Error::DownloadError(key.to_string(), e.to_string()))?;

    let tmp_dir_path: &std::path::Path = temp_dir.as_ref();

    std::fs::create_dir_all(tmp_dir_path.join("wapm_package_install"))
        .map_err(|e| Error::IoErrorCreatingDirectory(key.to_string(), e.to_string()))?;

    let temp_tar_gz_path = tmp_dir_path
        .join("wapm_package_install")
        .join("package.pirita");

    if nocache || autoconvert || (
       target_file_path.exists() &&
       target_file_path.metadata()?.len() == total_size &&
       Confirm::new()
        .with_prompt(format!("The package {key:?} seems to already have been downloaded. Download again? (no)"))
        .default(false)
        .interact()?
    ) {

        let mut dest = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_tar_gz_path)
            .map_err(|e| Error::IoCopyError(key.to_string(), e.to_string()))?;

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .progress_chars("#>-")
        );

        let mut downloaded = 0_u64;

        if let Some(first_chunk) = response.chunk().await? {
            let new = (downloaded + first_chunk.len() as u64).min(total_size);
            downloaded = new;
            if !autoconvert && !pirita::Pirita::check_is_pirita_file(&first_chunk) {
                pb.finish_and_clear();
                return Err(anyhow!("Error: remote package is not a PiritaFile"));
            }
            dest.write_all(&first_chunk)?;
            pb.set_position(new);
        }

        while let Some(chunk) = response.chunk().await? {
            let new = (downloaded + chunk.len() as u64).min(total_size);
            downloaded = new;
            dest.write_all(&chunk)?;
            pb.set_position(new);
        }

        pb.finish_and_clear();
        println!("downloaded: {download_url} to {}", temp_tar_gz_path.display());

        std::fs::copy(&temp_tar_gz_path, &target_file_path)?;
    }

    println!("file downloaded: {autoconvert}, {}", pirita::Pirita::load_mmap(temp_tar_gz_path.clone()).is_none());

    if autoconvert && pirita::Pirita::load_mmap(temp_tar_gz_path.clone()).is_none() {

        std::fs::remove_file(&target_file_path)?;

        // autoconvert .tar.gz => .pirita after download
        let e = pirita::convert_targz_to_pirita(
            &temp_tar_gz_path, 
            &target_file_path,
            None,
            &pirita::TransformManifestFunctions {
                get_atoms_wapm_toml: wapm_toml::get_wapm_atom_file_paths,
                get_dependencies: wapm_toml::get_dependencies,
                get_package_annotations: wapm_toml::get_package_annotations,
                get_modules: wapm_toml::get_modules,
                get_commands: wapm_toml::get_commands,
                get_bindings: wapm_toml::get_bindings,
                get_manifest_file_names: wapm_toml::get_manifest_file_names,
                get_metadata_paths: wapm_toml::get_metadata_paths,
                get_wapm_manifest_file_name: wapm_toml::get_wapm_manifest_file_name,
            },
        );

        if let Err(e) = e {
            println!("{e}");
        }
    }

    let parsed_file = pirita::Pirita::load_mmap(target_file_path.clone()).ok_or(anyhow!(
        "Could not parse {key:?} ({target_file_path:?}): not a PiritaFile"
    ))?;

    std::fs::create_dir_all(directory.join("wapm_packages").join(".bin"))?;

    for (command_name, _) in parsed_file.get_manifest().commands.iter() {
        let command =
            format!("wasmer run {target_file_path:?} --invoke {command_name:?}");
        let command_path = directory
            .join("wapm_packages")
            .join(".bin")
            .join(&command_name);
        std::fs::write(&command_path, command.as_bytes())?;
    }

    Ok((key, package_dir, download_url.to_string()))
}
