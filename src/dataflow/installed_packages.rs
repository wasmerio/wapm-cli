use crate::data::manifest::Manifest;
use crate::dataflow::manifest_packages::ManifestResult;
use crate::dataflow::resolved_packages::ResolvedPackages;
use crate::dataflow::WapmPackageKey;
use crate::graphql::VERSION;
use crate::keys;
use crate::util::{
    self, create_package_dir, fully_qualified_package_display_name, get_package_namespace_and_name,
};
use flate2::read::GzDecoder;
use reqwest::ClientBuilder;
use std::fs::OpenOptions;
use std::io;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use tar::Archive;

#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(
        display = "There was a problem opening the manifest for installed package \"{}\". {}",
        _0, _1
    )]
    InstalledDependencyIsMissingManifest(String, String),
    #[fail(
        display = "There was a problem decompressing the package data for \"{}\". {}",
        _0, _1
    )]
    DecompressionError(String, String),
    #[fail(
        display = "There was a problem parsing the package name for \"{}\". {}",
        _0, _1
    )]
    FailedToParsePackageName(String, String),
    #[fail(
        display = "There was an IO error creating the wapm_packages directory for package \"{}\". {}",
        _0, _1
    )]
    IoErrorCreatingDirectory(String, String),
    #[fail(
        display = "There was an IO error copying package data for package \"{}\". {}",
        _0, _1
    )]
    IoCopyError(String, String),
    #[fail(
        display = "Error downloading package data for package \"{}\". {}",
        _0, _1
    )]
    DownloadError(String, String),
}

/// A structure containing installed packages. Currently contains the key, the deserialized
/// manifest, and the download url.
#[derive(Clone, Debug)]
pub struct InstalledPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, Manifest, String)>,
}

impl<'a> InstalledPackages<'a> {
    /// Will install the resolved manifest packages into the specified directory.
    pub fn install<Installer: Install<'a>, P: AsRef<Path>>(
        directory: P,
        resolve_packages: ResolvedPackages<'a>,
    ) -> Result<Self, Error> {
        let packages_result: Result<Vec<(WapmPackageKey, PathBuf, String)>, Error> =
            resolve_packages
                .packages
                .into_iter()
                .map(|(key, download_url)| {
                    info!("Installing {}@{}", key.name, key.version);
                    Installer::install_package(&directory, key, &download_url)
                })
                .collect();
        let packages_result: Result<Vec<(WapmPackageKey, Manifest, String)>, Error> =
            packages_result?
                .into_iter()
                .map(|(key, dir, download_url)| {
                    let manifest = match ManifestResult::find_in_directory(&dir) {
                        ManifestResult::ManifestError(e) => {
                            return Err(Error::InstalledDependencyIsMissingManifest(
                                key.clone().to_string(),
                                e.to_string(),
                            ));
                        }
                        ManifestResult::Manifest(m) => m,
                        ManifestResult::NoManifest => {
                            return Err(Error::InstalledDependencyIsMissingManifest(
                                key.clone().to_string(),
                                "Manifest was not found.".to_string(),
                            ));
                        }
                    };
                    Ok((key.clone(), manifest, download_url))
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
        key: &WapmPackageKey,
    ) -> Result<(), failure::Error> {
        compressed_archive.seek(SeekFrom::Start(0))?;
        let gz = GzDecoder::new(compressed_archive);
        let mut archive = Archive::new(gz);
        archive
            .unpack(&pkg_name)
            .map_err(|err| Error::DecompressionError(key.to_string(), format!("{}", err)))?;
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
            .map_err(|e| Error::FailedToParsePackageName(key.to_string(), e.to_string()))?;
        let fully_qualified_package_name: String =
            fully_qualified_package_display_name(pkg_name, &key.version);
        let package_dir = create_package_dir(&directory, namespace, &fully_qualified_package_name)
            .map_err(|err| Error::IoErrorCreatingDirectory(key.to_string(), err.to_string()))?;
        let client = ClientBuilder::new().gzip(false).build().unwrap();
        let user_agent = format!(
            "wapm/{} {} {}",
            VERSION,
            whoami::platform(),
            whoami::os().to_lowercase(),
        );
        let mut response = client
            .get(download_url.as_ref())
            .header(reqwest::header::USER_AGENT, user_agent)
            .send()
            .map_err(|e| {
                let error_message = e.to_string();
                #[cfg(feature = "telemetry")]
                {
                    let e = e.into();
                    sentry::integrations::failure::capture_error(&e);
                }
                Error::DownloadError(key.to_string(), error_message)
            })?;
        // download public key
        // all public keys on wapm.io should be signed with previous public key?
        let timestamp = 0;
        let public_keys_from_server = vec![(
            "KEY_VALUE".to_string(),
            Some("SIGNATURE".to_string()),
            timestamp,
        )];
        // sort by timestamp

        // TOOD: handle key errors
        let mut keys_db = keys::open_keys_db().unwrap();
        let latest_public_key = keys::get_latest_public_key_for_user(&keys_db, &namespace).unwrap();

        match (
            !public_keys_from_server.is_empty(),
            latest_public_key.is_some(),
        ) {
            (true, true) => {
                // verify or prompt and store
                let (pk_id, pkv) =
                    keys::normalize_public_key(public_keys_from_server[0].0.clone()).unwrap();
                // the following unwrap is safe because of the check in the match expression
                let latest_local_key = latest_public_key.unwrap();
                if pk_id == latest_local_key.public_key_tag
                    && pkv == latest_local_key.public_key_value
                {
                    // keys match
                } else {
                    // mismatch, prompt user
                    let user_trusts_new_key =
                        util::prompt_user_for_yes(&format!(
                            "The keys {:?} and {:?} do not match. Do you want to trust the new key ({:?} {:?})?",
                            &latest_local_key.public_key_tag, &pk_id, &pk_id, &pkv
                        )).unwrap();

                    if user_trusts_new_key {
                        keys::import_public_key(
                            &mut keys_db,
                            public_keys_from_server[0].0.clone(),
                            pkg_name.to_string(),
                        )
                        .expect("TODO: handle this");
                    } else {
                        // fail here
                    }
                }
            }
            (true, false) => {
                // prompt and store
                let (pk_id, pkv) =
                    // TODO: prompt user on corrupt public key?
                    keys::normalize_public_key(public_keys_from_server[0].0.clone()).unwrap();
                let user_trusts_new_key = util::prompt_user_for_yes(&format!(
                    "New public key encountered: {} {} while installing {}.
Would you like to trust this key?",
                    &pk_id, &pkv, &fully_qualified_package_name
                ))
                .unwrap();
                if user_trusts_new_key {
                    // add it to DB
                } else {
                    // otherwise fail
                }
            }
            (false, true) => {
                // server error or scary things happening
                warn!(
                    "The server does not have a public key for {} for the package {}. This could mean that the wapm registry has been compromised",
                    &namespace, &fully_qualified_package_name
                );
                let user_wants_to_continue =
                    util::prompt_user_for_yes("Would you like to continue?").unwrap();

                if user_wants_to_continue {
                    // proceed to insecure install
                } else {
                    // fail early
                }
            }
            (false, false) => {
                // insecure install
            }
        }

        // manage pubilc key database
        // verify
        let temp_dir = tempdir::TempDir::new("wapm_package_install")
            .map_err(|e| Error::DownloadError(key.to_string(), e.to_string()))?;
        let temp_tar_gz_path = temp_dir.path().join("package.tar.gz");
        let mut dest = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_tar_gz_path)
            .map_err(|e| Error::IoCopyError(key.to_string(), e.to_string()))?;
        io::copy(&mut response, &mut dest)
            .map_err(|e| Error::DownloadError(key.to_string(), e.to_string()))?;
        Self::decompress_and_extract_archive(dest, &package_dir, &key)
            .map_err(|e| Error::DecompressionError(key.to_string(), e.to_string()))?;
        Ok((key, package_dir, download_url.as_ref().to_string()))
    }
}
