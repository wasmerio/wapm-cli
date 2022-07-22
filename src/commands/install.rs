//! Code pertaining to the `install` subcommand

use crate::dataflow::{
    resolved_packages::{get_packages_query, GetPackagesQuery},
    WapmDistribution,
};
use crate::graphql::execute_query;

use graphql_client::*;
use wapm_resolve_url::get_pirita_url_of_package;

use crate::config::Config;
use crate::dataflow;
use crate::util;
use std::borrow::Cow;
use std::path::Path;
#[cfg(feature = "pirita_file")]
use std::path::PathBuf;
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
            let installed_packages = get_packages_with_versions(&options.packages)?;

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

fn get_packages_with_versions(package_args: &[String]) -> anyhow::Result<Vec<WapmDistribution>> {

    use wapm_resolve_url::get_tar_gz_url_of_package;
    use url::Url;

    let config = Config::from_file()?;
    let registry_url = Url::parse(&config.registry.get_graphql_url())?;

    let mut result = vec![];
    for name in package_args {

        let name_with_version: Vec<&str> = name.split("@").collect();

        let package_name = match &name_with_version[..] {
            [package_name, _] => Some(package_name),
            [package_name] => Some(package_name),
            _ => None,
        }
        .ok_or(InstallError::InvalidPackageIdentifier { name: name.clone() })?;

        let package_version = match &name_with_version[..] {
            [_, version] => Some(version.clone()),
            _ => None,
        };

        let (targz_url, version) = get_tar_gz_url_of_package(&registry_url, &package_name, package_version)
        .ok_or(InstallError::PackageNotFound {
            name: name.to_string(),
        })?;

        let pirita_url = get_pirita_url_of_package(&registry_url, &package_name, Some(&version));

        let package_to_download = WapmDistribution {
            name: package_name.to_string(),
            version: version.to_string(),
            download_url: format!("{targz_url}"),
            pirita_download_url: pirita_url.map(|(u, _)| format!("{u}")),
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
                    download_pirita(
                        &p.name,
                        &p.version,
                        &pirita_url,
                        false,
                        &install_directory,
                        options.nocache || options.force_yes,
                    )
                    .await?;
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

    if nocache || (
       target_file_path.exists() &&
       target_file_path.metadata()?.len() == total_size &&
       Confirm::new()
        .with_prompt(format!("The package {key:?} seems to already have been downloaded. Download again? (no)"))
        .default(false)
        .interact()?
    ) {

        let temp_dir =
            create_temp_dir()
            .map_err(|e| Error::DownloadError(key.to_string(), e.to_string()))?;

        let tmp_dir_path: &std::path::Path = temp_dir.as_ref();

        std::fs::create_dir_all(tmp_dir_path.join("wapm_package_install"))
            .map_err(|e| Error::IoErrorCreatingDirectory(key.to_string(), e.to_string()))?;

        let temp_tar_gz_path = tmp_dir_path
            .join("wapm_package_install")
            .join("package.pirita");

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
            if !pirita::PiritaFile::check_is_pirita_file(&first_chunk) && !autoconvert {
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

        std::fs::rename(&temp_tar_gz_path, &target_file_path)?;

        if !pirita::PiritaFile::load_mmap(temp_tar_gz_path.clone()).is_none() {
            if !autoconvert {
                std::fs::remove_file(&target_file_path)?;
                return Err(anyhow!("Error: remote package is not a PiritaFile"));
            }

            // autoconvert .tar.gz => .pirita after download
            let _ = pirita::convert_targz_to_pirita(
                &temp_tar_gz_path, 
                &target_file_path,
                None,
                &pirita::TransformManifestFunctions {
                    get_atoms_wapm_toml: wapm_toml::get_wapm_atom_file_paths,
                    get_dependencies: wapm_toml::get_dependencies,
                    get_package_annotations: wapm_toml::get_package_annotations,
                    get_modules: wapm_toml::get_modules,
                    get_commands: wapm_toml::get_commands,
                    get_manifest_file_names: wapm_toml::get_manifest_file_names,
                    get_metadata_paths: wapm_toml::get_metadata_paths,
                    get_wapm_manifest_file_name: wapm_toml::get_wapm_manifest_file_name,
                },
            );
        }
    }

    let parsed_file = pirita::PiritaFile::load_mmap(target_file_path.clone()).ok_or(anyhow!(
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
