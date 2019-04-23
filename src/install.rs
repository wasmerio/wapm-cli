use crate::dependency_resolver::Dependency;
use crate::graphql::VERSION;
use crate::manifest::PACKAGES_DIR_NAME;
use flate2::read::GzDecoder;
use reqwest::Client;
use std::fs::OpenOptions;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::{fs, io};
use tar::Archive;

pub fn install_package<P: AsRef<Path>>(
    dependency: &Dependency,
    directory: P,
) -> Result<(), failure::Error> {
    let (namespace, pkg_name) = get_package_namespace_and_name(&dependency.name)?;
    let fully_qualified_package_name =
        fully_qualified_package_display_name(pkg_name, &dependency.version);
    let package_dir = create_package_dir(&directory, namespace, &fully_qualified_package_name)
        .map_err(|err| InstallError::MiscError {
            custom_text: "Could not create package directory".to_string(),
            error: format!("{}", err),
        })?;

    let client = Client::new();
    let user_agent = format!(
        "wapm/{} {} {}",
        VERSION,
        whoami::platform(),
        whoami::os().to_lowercase(),
    );
    let mut response = client
        .get(&dependency.download_url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .send()?;

    let temp_dir =
        tempdir::TempDir::new("wapm_package_install").map_err(|err| InstallError::MiscError {
            custom_text: "Failed to create temporary directory to open the package in".to_string(),
            error: format!("{}", err),
        })?;
    let temp_tar_gz_path = temp_dir.path().join("package.tar.gz");
    let mut dest = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&temp_tar_gz_path)?;
    io::copy(&mut response, &mut dest).map_err(|err| InstallError::MiscError {
        custom_text: "Could not copy response to temporary directory".to_string(),
        error: format!("{}", err),
    })?;
    decompress_and_extract_archive(dest, &package_dir)
        .map_err(|err| InstallError::CannotOpenPackageArchive(format!("{}", err)))?;
    Ok(())
}

fn create_package_dir<P: AsRef<Path>, P2: AsRef<Path>>(
    project_dir: P,
    namespace_dir: P2,
    fully_qualified_package_name: &str,
) -> Result<PathBuf, io::Error> {
    let mut package_dir = project_dir.as_ref().join(PACKAGES_DIR_NAME);
    package_dir.push(namespace_dir);
    package_dir.push(&fully_qualified_package_name);
    fs::create_dir_all(&package_dir)?;
    Ok(package_dir)
}

#[inline]
fn fully_qualified_package_display_name(package_name: &str, package_version: &str) -> String {
    format!("{}@{}", package_name, package_version)
}

#[inline]
pub fn get_package_namespace_and_name(package_name: &str) -> Result<(&str, &str), failure::Error> {
    let split: Vec<&str> = package_name.split('/').collect();
    match &split[..] {
        [namespace, name] => Ok((*namespace, *name)),
        [global_package_name] => {
            info!(
                "Interpreting unqualified global package name \"{}\" as \"_/{}\"",
                package_name, global_package_name
            );
            Ok(("_", *global_package_name))
        }
        _ => bail!("Package name is invalid"),
    }
}
/// Loads a GZipped tar in to memory, decompresses it, and unpackages the
/// content to `pkg_name`
fn decompress_and_extract_archive<P: AsRef<Path>, F: io::Seek + io::Read>(
    mut compressed_archive: F,
    pkg_name: P,
) -> Result<(), failure::Error> {
    compressed_archive.seek(SeekFrom::Start(0))?;
    let gz = GzDecoder::new(compressed_archive);
    let mut archive = Archive::new(gz);
    archive
        .unpack(&pkg_name)
        .map_err(|err| InstallError::CorruptFile {
            name: format!("{}", pkg_name.as_ref().display()),
            error: format!("{}", err),
        })?;
    Ok(())
}

#[derive(Debug, Fail)]
enum InstallError {
    #[fail(display = "Can't process package file {} because {}", name, error)]
    CorruptFile { name: String, error: String },

    #[fail(display = "{}: {}", custom_text, error)]
    MiscError { custom_text: String, error: String },

    #[fail(display = "Failed to decompress or open package: {}", _0)]
    CannotOpenPackageArchive(String),
}
