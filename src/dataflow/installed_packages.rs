use crate::data::manifest::Manifest;
use crate::database;
use crate::dataflow::manifest_packages::ManifestResult;
use crate::dataflow::resolved_packages::ResolvedPackages;
use crate::dataflow::WapmPackageKey;
use crate::graphql::VERSION;
use crate::keys;
use crate::proxy;
use crate::util::{
    self, create_package_dir, fully_qualified_package_display_name, get_package_namespace_and_name,
};
use flate2::read::GzDecoder;
use reqwest::blocking::ClientBuilder;
use std::fs::{self, OpenOptions};
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
    #[fail(display = "Failed during network connection: {}", _0)]
    IoConnectionError(String),
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
    pub fn install<Installer: Install<'a>>(
        directory: &Path,
        resolve_packages: ResolvedPackages<'a>,
        force_insecure_install: bool,
    ) -> Result<Self, Error> {
        let packages_result: Result<Vec<(WapmPackageKey, PathBuf, String)>, Error> =
            resolve_packages
                .packages
                .into_iter()
                .map(|(key, (download_url, signature))| {
                    info!("Installing {}@{}", key.name, key.version);
                    Installer::install_package(
                        &directory,
                        key,
                        &download_url,
                        signature,
                        force_insecure_install,
                    )
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
    fn install_package(
        directory: &Path,
        key: WapmPackageKey<'a>,
        download_url: &str,
        signature: Option<keys::WapmPackageSignature>,
        force_insecure_install: bool,
    ) -> Result<(WapmPackageKey<'a>, PathBuf, String), Error>;
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

#[derive(Debug, Clone)]
struct PackageSignatureVerificationData {
    insecure_install: bool,
    key_to_verify_package_with: Option<(String, String)>,
    signature_to_use: Option<String>,
}

fn verify_integrity_of_package(
    namespace: &str,
    fully_qualified_package_name: String,
    signature: Option<keys::WapmPackageSignature>,
) -> Result<PackageSignatureVerificationData, Error> {
    let mut keys_db = database::open_db().map_err(|e| {
        Error::KeyManagementError(fully_qualified_package_name.clone(), e.to_string())
    })?;
    // get the latest key for the given namespace first. If the server claims that the owner
    // is someone else, then we'll search the local database for a key from that user
    // this is required for how globally namespaced packages work and also allows transfer
    // of ownership
    let mut latest_public_key = keys::get_latest_public_key_for_user(&keys_db, &namespace)
        .map_err(|e| {
            Error::KeyManagementError(fully_qualified_package_name.clone(), e.to_string())
        })?;

    let import_public_key = |mut_db_handle, pk_id, pkv, associated_user| {
        keys::import_public_key(mut_db_handle, pk_id, pkv, associated_user).map_err(|e| {
            Error::KeyManagementError(
                fully_qualified_package_name.clone(),
                format!("could not add public key: {}", e),
            )
        })
    };

    let mut insecure_install = false;
    let mut key_to_verify_package_with = None;
    let mut signature_to_use = None;

    if let Some(keys::WapmPackageSignature {
        public_key_id,
        public_key,
        signature_data,
        owner,
        ..
    }) = signature
    {
        // Cases 1-X:
        // get key for owner as identified by the server
        latest_public_key = if owner != namespace {
            keys::get_latest_public_key_for_user(&keys_db, &owner).map_err(|e| {
                Error::KeyManagementError(fully_qualified_package_name.clone(), e.to_string())
            })?
        } else {
            latest_public_key
        };
        debug!(
            "Latest public key for user {} during install: {:?}",
            &namespace, latest_public_key
        );

        if let Some(latest_local_key) = latest_public_key {
            // Case 1-1: server has key and client has key
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
                    import_public_key(&mut keys_db, &public_key_id, &public_key, owner)?;
                    key_to_verify_package_with = Some((public_key_id, public_key));
                    signature_to_use = Some(signature_data);
                } else {
                    return Err(Error::InstallAborted(format!(
                        "Mismatching key on package {} was not trusted by user",
                        &fully_qualified_package_name
                    )));
                }
            }
        } else {
            // Case 1-0: server has key and client does not have key
            // prompt and store
            let user_trusts_new_key = util::prompt_user_for_yes(&format!(
                "New public key encountered for user {}: {} {} while installing {}.
Would you like to trust this key?",
                &owner, &public_key_id, &public_key, &fully_qualified_package_name
            ))
            .expect("Could not read input from user");
            if user_trusts_new_key {
                import_public_key(&mut keys_db, &public_key_id, &public_key, owner)?;
                key_to_verify_package_with = Some((public_key_id, public_key));
                signature_to_use = Some(signature_data);
            } else {
                return Err(Error::InstallAborted(format!(
                    "User did not trust key from registry for package {}",
                    &fully_qualified_package_name
                )));
            }
        }
    } else {
        // Cases 0-X:
        // server does not have key
        if let Some(latest_local_key) = latest_public_key {
            // Case 0-1: server does not have key and client has key
            // server error or scary things happening
            warn!(
                    "The server does not have a public key for {} for the package {} and the package is not signed but a public key for {} is known locally ({}).\nThis could mean that the wapm registry has been compromised, that the package was created before the publisher started signing their packages, or that the publisher decided not to sign this package.",
                    &namespace, &fully_qualified_package_name, &namespace, &latest_local_key.public_key_id
                );

            let user_wants_to_do_insecure_install = util::prompt_user_for_yes(
                "Would you like to proceed with an unverified installation?",
            )
            .expect("Could not read input from user");

            if user_wants_to_do_insecure_install {
                insecure_install = true;
            } else {
                return Err(Error::InstallAborted(format!(
                    "User did not trust unsigned package {}",
                    &fully_qualified_package_name
                )));
            }
        } else {
            // Case 0-0: server does not have key and client does not have key
            // silently proceed to insecure install for now
            insecure_install = true;
        }
    }
    Ok(PackageSignatureVerificationData {
        insecure_install,
        key_to_verify_package_with,
        signature_to_use,
    })
}

/// This impl will install packages from a wapm registry.
impl<'a> Install<'a> for RegistryInstaller {
    fn install_package(
        directory: &Path,
        key: WapmPackageKey<'a>,
        download_url: &str,
        signature: Option<keys::WapmPackageSignature>,
        force_insecure_install: bool,
    ) -> Result<(WapmPackageKey<'a>, PathBuf, String), Error> {
        let (namespace, pkg_name) = get_package_namespace_and_name(&key.name)
            .map_err(|e| Error::FailedToParsePackageName(key.to_string(), e.to_string()))?;
        let fully_qualified_package_name: String =
            fully_qualified_package_display_name(pkg_name, &key.version);
        let package_dir = create_package_dir(&directory, namespace, &fully_qualified_package_name)
            .map_err(|err| Error::IoErrorCreatingDirectory(key.to_string(), err.to_string()))?;
        let client = {
            let builder = ClientBuilder::new().gzip(false);
            let builder = if let Some(proxy) = proxy::maybe_set_up_proxy()
                .map_err(|e| Error::IoConnectionError(format!("{}", e)))?
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
            whoami::os().to_lowercase(),
        );
        let mut response = client
            .get(download_url)
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

        // step to perform after package is decompressed: may be a no-op or may
        // execute side effects such as logging to the user.
        let mut key_sign_end_step: Box<dyn FnMut(&mut fs::File) -> Result<(), Error>> =
            if !force_insecure_install {
                let PackageSignatureVerificationData {
                    insecure_install,
                    key_to_verify_package_with,
                    signature_to_use,
                } = verify_integrity_of_package(
                    namespace,
                    fully_qualified_package_name.clone(),
                    signature,
                )?;

                if insecure_install {
                    Box::new(|_dest| Ok(()))
                } else {
                    Box::new(move |mut dest| {
                        let (pk_id, pkv) = key_to_verify_package_with
                            .clone()
                            .expect("Critical internal logic error");
                        let signature_to_use = signature_to_use
                            .clone()
                            .expect("Critical internal logic error");
                        verify_signature_on_package(&pkv, &signature_to_use, &mut dest).map_err(
                            |e| {
                                Error::FailedToValidateSignature(
                                    fully_qualified_package_name.clone(),
                                    pk_id,
                                    e.to_string(),
                                )
                            },
                        )?;
                        info!(
                            "Signature of package {} verified!",
                            &fully_qualified_package_name
                        );
                        Ok(())
                    })
                }
            } else {
                Box::new(|_dest| Ok(()))
            };

        let temp_dir = tempfile::TempDir::new()
            .map_err(|e| Error::DownloadError(key.to_string(), e.to_string()))?;
        fs::create_dir("wapm_package_install")
            .map_err(|e| Error::IoErrorCreatingDirectory(key.to_string(), e.to_string()))?;
        let temp_tar_gz_path = temp_dir
            .path()
            .join("wapm_package_install")
            .join("package.tar.gz");
        let mut dest = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_tar_gz_path)
            .map_err(|e| Error::IoCopyError(key.to_string(), e.to_string()))?;
        io::copy(&mut response, &mut dest)
            .map_err(|e| Error::DownloadError(key.to_string(), e.to_string()))?;

        key_sign_end_step(&mut dest)?;

        Self::decompress_and_extract_archive(dest, &package_dir, &key)
            .map_err(|e| Error::DecompressionError(key.to_string(), e.to_string()))?;
        Ok((key, package_dir, download_url.to_string()))
    }
}

/// Verifies the signature of a downloaded package archive
fn verify_signature_on_package(
    pkv: &str,
    signature_to_use: &str,
    dest: &mut fs::File,
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
