use crate::cfg_toml::manifest::Manifest;
use crate::dataflow::resolved_manifest_packages::ResolvedManifestPackages;
use crate::dataflow::{Error, WapmPackageKey};
use crate::util::{
    create_package_dir, fully_qualified_package_display_name, get_package_namespace_and_name,
};
use flate2::read::GzDecoder;
use std::fs::OpenOptions;
use std::io;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tar::Archive;

/// A structure containing installed packages. Currently contains the key, the deserialized
/// manifest, and the download url.
#[derive(Clone, Debug)]
pub struct InstalledManifestPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, Manifest, String)>,
}

impl<'a> InstalledManifestPackages<'a> {
    /// Will install the resolved manifest packages into the specified directory.
    pub fn install<Installer: Install<'a>, P: AsRef<Path>>(
        directory: P,
        resolved_manifest_packages: ResolvedManifestPackages<'a>,
    ) -> Result<Self, Error> {
        let packages_result: Result<Vec<(WapmPackageKey, PathBuf, String)>, Error> =
            resolved_manifest_packages
                .packages
                .into_iter()
                .map(|(key, download_url)| {
                    Installer::install_package(&directory, key, &download_url)
                })
                .collect();
        let packages_result: Result<Vec<(WapmPackageKey, Manifest, String)>, Error> =
            packages_result?
                .into_iter()
                .map(|(key, dir, download_url)| {
                    let m = Manifest::find_in_directory(&dir)
                        .map(|m| (key, m))
                        .map_err(|e| Error::InstallError(e.to_string()));
                    let m = m.map(|(k, m)| (k, m, download_url));
                    m
                })
                .collect();
        let packages = packages_result?;
        Ok(Self { packages })
    }
}

/// A trait for injecting an installer for installing wapm packages.
pub trait Install<'a> {
    fn install_package<P: AsRef<Path>, S: AsRef<str>>(
        directory: P,
        key: WapmPackageKey<'a>,
        download_url: S,
    ) -> Result<(WapmPackageKey, PathBuf, String), Error>;
}

pub struct RegistryInstaller;

impl RegistryInstaller {
    fn decompress_and_extract_archive<P: AsRef<Path>, F: io::Seek + io::Read>(
        mut compressed_archive: F,
        pkg_name: P,
    ) -> Result<(), failure::Error> {
        compressed_archive.seek(SeekFrom::Start(0))?;
        let gz = GzDecoder::new(compressed_archive);
        let mut archive = Archive::new(gz);
        archive
            .unpack(&pkg_name)
            .map_err(|err| Error::InstallError(format!("{}", err)))?;
        Ok(())
    }
}

/// This impl will install packages from a wapm registry.
impl<'a> Install<'a> for RegistryInstaller {
    fn install_package<P: AsRef<Path>, S: AsRef<str>>(
        directory: P,
        key: WapmPackageKey,
        download_url: S,
    ) -> Result<(WapmPackageKey, PathBuf, String), Error> {
        let (namespace, pkg_name) = get_package_namespace_and_name(&key.name)
            .map_err(|e| Error::InstallError(e.to_string()))?;
        let fully_qualified_package_name: String =
            fully_qualified_package_display_name(pkg_name, &key.version);
        let package_dir = create_package_dir(&directory, namespace, &fully_qualified_package_name)
            .map_err(|_err| {
                Error::InstallError("Could not create package directory".to_string())
            })?;
        let mut response =
            reqwest::get(download_url.as_ref()).map_err(|e| Error::InstallError(e.to_string()))?;
        let temp_dir = tempdir::TempDir::new("wapm_package_install").map_err(|_err| {
            Error::InstallError(
                "Failed to create temporary directory to open the package in".to_string(),
            )
        })?;
        let temp_tar_gz_path = temp_dir.path().join("package.tar.gz");
        let mut dest = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_tar_gz_path)
            .map_err(|e| Error::InstallError(e.to_string()))?;
        io::copy(&mut response, &mut dest).map_err(|_err| {
            Error::InstallError("Could not copy response to temporary directory".to_string())
        })?;
        Self::decompress_and_extract_archive(dest, &package_dir)
            .map_err(|err| Error::InstallError(format!("{}", err)))?;
        Ok((key, package_dir, download_url.as_ref().to_string()))
    }
}
