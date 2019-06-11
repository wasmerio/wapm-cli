use crate::data::manifest::Manifest;
use crate::database;
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
use std::io::{Seek, SeekFrom};
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
    #[fail(display = "Install aborted: {}", _0)]
    InstallAborted(String),
    #[fail(
        display = "There was an error storing keys for package \"{}\" during installation: {}",
        _0, _1
    )]
    KeyManagementError(String, String),
    #[fail(display = "Failed to validate package {} with key {}: {}", _0, _1, _2)]
    FailedToValidateSignature(String, String, String),
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
                .map(|(key, (download_url, signature))| {
                    info!("Installing {}@{}", key.name, key.version);
                    Installer::install_package(&directory, key, &download_url, signature)
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
        signature: Option<keys::WapmPackageSignature>,
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
        signature: Option<keys::WapmPackageSignature>,
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

        let mut keys_db = database::open_db().map_err(|e| {
            Error::KeyManagementError(fully_qualified_package_name.clone(), e.to_string())
        })?;
        let latest_public_key = keys::get_latest_public_key_for_user(&keys_db, &namespace)
            .map_err(|e| {
                Error::KeyManagementError(fully_qualified_package_name.clone(), e.to_string())
            })?;
        let mut import_public_key = |pk_id, pkv, associated_user| {
            keys::import_public_key(&mut keys_db, pk_id, pkv, associated_user).map_err(|e| {
                Error::KeyManagementError(
                    fully_qualified_package_name.clone(),
                    format!("could not add public key: {}", e),
                )
            })
        };

        let mut insecure_install = false;
        let mut key_to_verify_package_with = None;
        let mut signature_to_use = None;

        match (signature, latest_public_key) {
            // server has key and client has key
            (
                Some(keys::WapmPackageSignature {
                    public_key_id,
                    public_key,
                    signature_data,
                    owner,
                    ..
                }),
                Some(latest_local_key),
            ) => {
                // verify or prompt and store
                if public_key_id == latest_local_key.public_key_id
                    && public_key == latest_local_key.public_key_value
                {
                    // keys match
                    trace!("Public key from server matches latest key locally");
                    key_to_verify_package_with = Some((
                        latest_local_key.public_key_id,
                        latest_local_key.public_key_value,
                    ));

                    signature_to_use = Some(signature_data);
                } else {
                    // mismatch, prompt user
                    let user_trusts_new_key =
                        util::prompt_user_for_yes(&format!(
                            "The keys {:?} and {:?} do not match. Do you want to trust the new key ({:?} {:?})?",
                            &latest_local_key.public_key_id, &public_key_id, &public_key_id, &public_key
                        )).expect("Could not read input from user");

                    if user_trusts_new_key {
                        import_public_key(&public_key_id, &public_key, owner)?;
                        key_to_verify_package_with = Some((public_key_id, public_key));
                        signature_to_use = Some(signature_data);
                    } else {
                        return Err(Error::InstallAborted(format!(
                            "Mismatching key on package {} was not trusted by user",
                            &fully_qualified_package_name
                        )));
                    }
                }
            }
            // server has key and client does not have key
            (
                Some(keys::WapmPackageSignature {
                    public_key_id,
                    public_key,
                    signature_data,
                    owner,
                    ..
                }),
                None,
            ) => {
                // prompt and store
                let user_trusts_new_key = util::prompt_user_for_yes(&format!(
                    "New public key encountered: {} {} while installing {}.
Would you like to trust this key?",
                    &public_key_id, &public_key, &fully_qualified_package_name
                ))
                .expect("Could not read input from user");
                if user_trusts_new_key {
                    import_public_key(&public_key_id, &public_key, owner)?;
                    key_to_verify_package_with = Some((public_key_id, public_key));
                    signature_to_use = Some(signature_data);
                } else {
                    return Err(Error::InstallAborted(format!(
                        "User did not trust key from registry for package {}",
                        &fully_qualified_package_name
                    )));
                }
            }
            // server does not have key and client has key
            (None, Some(latest_local_key)) => {
                // server error or scary things happening
                warn!(
                    "The server does not have a public key for {} for the package {}. This could mean that the wapm registry has been compromised.  Continuning with local public key {}",
                    &namespace, &fully_qualified_package_name, &latest_local_key.public_key_id
                );

                key_to_verify_package_with = Some((
                    latest_local_key.public_key_id,
                    latest_local_key.public_key_value,
                ));
            }
            // server does not have key and client does not have key
            (None, None) => {
                // silently proceed to insecure install for now
                insecure_install = true;
            }
        }

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

        if !insecure_install {
            let (pk_id, pkv) = key_to_verify_package_with.expect("Critical internal logic error");
            let signature_to_use = signature_to_use.expect("Critical internal logic error");
            verify_signature_on_package(&pkv, &signature_to_use, &mut dest).map_err(|e| {
                Error::FailedToValidateSignature(
                    fully_qualified_package_name.clone(),
                    pk_id,
                    e.to_string(),
                )
            })?;
            info!(
                "Signature of package {} verified!",
                &fully_qualified_package_name
            );
        }

        Self::decompress_and_extract_archive(dest, &package_dir, &key)
            .map_err(|e| Error::DecompressionError(key.to_string(), e.to_string()))?;
        Ok((key, package_dir, download_url.as_ref().to_string()))
    }
}

/// Verifies the signature of a downloaded package archive
fn verify_signature_on_package(
    pkv: &str,
    signature_to_use: &str,
    dest: &mut std::fs::File,
) -> Result<(), failure::Error> {
    dest.seek(SeekFrom::Start(0))?;
    // TODO: refactor to remove extra bit of info here
    let public_key = minisign::PublicKey::from_base64(&pkv)
        .map_err(|e| format_err!("Invalid key: {}", e.to_string()))?;
    let sig_box = minisign::SignatureBox::from_string(&signature_to_use)
        .map_err(|e| format_err!("Error with downloaded signature: {}", e.to_string()))?;

    minisign::verify(&public_key, &sig_box, dest, true, false)
        .map_err(|e| format_err!("Could not validate signature: {}", e.to_string()))
}
