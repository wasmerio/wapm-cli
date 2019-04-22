use std::cmp::Ordering;
use std::collections::btree_set::BTreeSet;
use std::path::{Path, PathBuf};

//use crate::bonjour::differences::{PackageDataDifferences, AddedPackages, RetainedPackages, MergedPackageData};
use crate::bonjour::lockfile::{LockfileData, LockfileResult, LockfileSource};
use crate::bonjour::manifest::{ManifestData, ManifestResult, ManifestSource};
use crate::bonjour::remote::RemotePackageData;
use crate::cfg_toml::lock::lockfile_command::LockfileCommand;
use crate::cfg_toml::lock::lockfile_module::LockfileModule;
use crate::dependency_resolver::{Dependency, PackageRegistry, PackageRegistryLike};
use graphql_client::*;
use std::collections::btree_map::BTreeMap;

pub mod differences;
pub mod lockfile;
pub mod manifest;
pub mod remote;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.graphql",
    query_path = "graphql/queries/get_packages.graphql",
    response_derives = "Debug"
)]
struct GetPackagesQuery;

#[derive(Clone, Debug, Fail)]
pub enum BonjourError {
    #[fail(display = "Could not parse manifest because {}.", _0)]
    ManifestTomlParseError(String),
    #[fail(display = "Could not parse lockfile because {}.", _0)]
    LockfileTomlParseError(String),
    #[fail(display = "Dependency version must be a string. Package name: {}.", _0)]
    DependencyVersionMustBeString(String),
    #[fail(display = "Could not install added packages. {}.", _0)]
    InstallError(String),
    #[fail(display = "Could not save lockfile. {}.", _0)]
    LockfileSaveError(String),
}

struct Token<'a> {
    raw: Cow<'a, str>
}

impl<'a> Token<'a> {
    pub fn new<S>(raw: S) -> Token<'a>
        where S: Into<Cow<'a, str>>
    {
        Token { raw: raw.into() }
    }
}


#[derive(Clone, Debug, Eq, Hash, PartialOrd, PartialEq)]
pub struct WapmPackageKey<'a> {
    pub name: Cow<'a, str>,
    pub version: Cow<'a, str>,
}

#[derive(Clone, Debug, Eq, Hash, PartialOrd, PartialEq)]
pub enum PackageKey<'a> {
    LocalPackage { directory: &'a Path },
    WapmPackage(WapmPackageKey<'a>),
    //    GitUrl { url: &'a str, },
}

impl<'a> PackageKey<'a> {
    fn new_registry_package<S>(name: S, version: S) -> Self where S: Into<Cow<'a, str>> {
        PackageKey::WapmPackage(WapmPackageKey { name: name.into(), version: version.into() })
    }
}

//impl<'a> Ord for PackageKey<'a> {
//    fn cmp(&self, other: &PackageKey<'a>) -> Ordering {
//        match (self, other) {
//            (
//                PackageKey::WapmPackage(WapmPackageKey { name, version }),
//                PackageKey::WapmPackage(WapmPackageKey {
//                    name: other_name,
//                    version: other_version,
//                }),
//            ) => {
//                let name_cmp = name.cmp(other_name);
//                let version_cmp = version.cmp(other_version);
//                match (name_cmp, version_cmp) {
//                    (Ordering::Equal, _) => version_cmp,
//                    _ => name_cmp,
//                }
//            }
//            (
//                PackageKey::LocalPackage { directory },
//                PackageKey::LocalPackage {
//                    directory: other_directory,
//                },
//            ) => directory.cmp(other_directory),
//            (PackageKey::LocalPackage { .. }, _) => Ordering::Less,
//            (PackageKey::WapmPackage { .. }, _) => Ordering::Greater,
//        }
//    }
//}

#[derive(Clone, Debug)]
pub struct LockfilePackage<'a> {
    pub modules: Vec<LockfileModule<'a>>,
    pub commands: Vec<LockfileCommand<'a>>,
}

#[derive(Clone, Debug)]
pub struct PkgData<'a> {
    modules: Vec<LockfileModule<'a>>,
    commands: Vec<LockfileCommand<'a>>,
    download_url: &'a str,
}
#[derive(Clone, Debug)]
pub enum PackageData<'a> {
    LockfilePackage {
        modules: Vec<LockfileModule<'a>>,
        commands: Vec<LockfileCommand<'a>>,
    },
    RemotePackage {
        download_url: &'a str,
    },
    //    ResolvedManifestDependencyPackage(Dependency),
    ManifestPackage,
}

impl<'a> PackageData<'a> {
    fn install(self) -> Result<(), BonjourError> {
        unimplemented!()
        //        match self {
        //            PackageData::LockfilePackage { .. } => {
        //                Ok(())
        //            },
        //            PackageData::RemotePackage { download_url } => {
        //                Ok(())
        //            },
        //        }
    }
}

fn install_added_dependencies<'a>(
    added_set: BTreeSet<PackageKey<'a>>,
    registry: &'a mut PackageRegistry,
) -> Result<Vec<&'a Dependency>, BonjourError> {
    // get added wapm registry packages
    let added_package_ids: Vec<(Cow<'a, &str>, Cow<'a, &str>)> = added_set
        .iter()
        .cloned()
        .filter_map(|id| match id {
            PackageKey::WapmPackage(WapmPackageKey { name, version }) => Some((name, version)),
            _ => None,
        })
        .collect();

    // sync and install missing dependencies
    registry
        .get_all_dependencies(added_package_ids)
        .map_err(|e| BonjourError::InstallError(e.to_string()))
}

use crate::bonjour::PackageKey::WapmPackage;
use crate::cfg_toml::manifest::Manifest;
use crate::graphql::execute_query;
use crate::util::{
    create_package_dir, fully_qualified_package_display_name, get_package_namespace_and_name,
};
use flate2::read::GzDecoder;
use std::collections::btree_map::Entry;
use std::collections::hash_set::HashSet;
use std::fs::OpenOptions;
use std::io;
use std::io::SeekFrom;
use tar::Archive;
use walrus::ir::Expr::BrTable;
use std::collections::hash_map::HashMap;
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct ResolvedManifestPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, String)>,
}

impl<'a> ResolvedManifestPackages<'a> {
    pub fn new(manifest_data: ChangedManifestPackages<'a>) -> Result<Self, BonjourError> {
        let wapm_pkgs = manifest_data
            .packages
            .into_iter()
            .filter_map(|k| match k {
                PackageKey::WapmPackage(k) => Some(k),
                _ => None,
            })
            .collect();
        let packages: Vec<_> = Self::sync_packages(wapm_pkgs)
            .map_err(|e| BonjourError::InstallError(e.to_string()))?;
        Ok(Self { packages })
    }

    fn get_response(added_pkgs: Vec<WapmPackageKey<'a>>) -> get_packages_query::ResponseData {
        let mut set: HashSet<WapmPackageKey<'a>> = added_pkgs.into_iter().collect();
        let names = set.iter().map(|k| k.name.to_string()).collect();
        let q = GetPackagesQuery::build_query(get_packages_query::Variables { names });
        execute_query(&q).unwrap()
    }

    fn sync_packages(
        added_pkgs: Vec<WapmPackageKey<'a>>,
    ) -> Result<Vec<(WapmPackageKey<'a>, String)>, failure::Error> {
        let response = Self::get_response(added_pkgs.clone());
        let results: Vec<(WapmPackageKey<'a>, String)> = response
            .package
            .into_iter()
            .filter_map(|p| p)
            .filter_map(|p| {
                let versions = p.versions.unwrap_or(vec![]);
                let name = p.name;
                Some((name, versions))
            })
            .map(|(n, vs)| {
                vs.into_iter()
                    .filter_map(|o| o)
                    .map(|v| {
                        let version = v.version;
                        let download_url = v.distribution.download_url;
                        (n.clone(), version, download_url)
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .filter_map(|(n, v, d)| {
                let key = added_pkgs.iter().find(|k| {
                    match k.name.find('/') {
                        Some(_) => k.name == n && k.version == v,
                        _ => k.name == &n[2..] && k.version == v,
                    }
                });
                key.map(|k| (k.clone(), d))
            })
            .collect();
        Ok(results)
    }
}

#[derive(Clone, Debug)]
pub struct InstalledManifestPackages<'a> {
    pub packages: Vec<(WapmPackageKey<'a>, Manifest, String)>,
}

impl<'a> InstalledManifestPackages<'a> {
    pub fn install<P: AsRef<Path>>(
        directory: P,
        resolved_manifest_packages: ResolvedManifestPackages<'a>,
    ) -> Result<Self, BonjourError> {
        let packages_result: Result<Vec<(WapmPackageKey, PathBuf, String)>, BonjourError> =
            resolved_manifest_packages
                .packages
                .into_iter()
                .map(|(key, download_url)| Self::install_package(&directory, key, &download_url))
                .collect();
        let packages_result: Result<Vec<(WapmPackageKey, Manifest, String)>, BonjourError> =
            packages_result?
                .into_iter()
                .map(|(key, dir, download_url)| {
                    let m = Manifest::find_in_directory(&dir)
                        .map(|m| (key, m))
                        .map_err(|e| BonjourError::InstallError(e.to_string()));
                    let m = m.map(|(k, m)| (k, m, download_url));
                    m
                })
                .collect();
        let packages = packages_result?;
        Ok(Self { packages })
    }

    fn install_package<P: AsRef<Path>, S: AsRef<str>>(
        directory: P,
        key: WapmPackageKey<'a>,
        download_url: S,
    ) -> Result<(WapmPackageKey, PathBuf, String), BonjourError> {
        let (namespace, pkg_name) = get_package_namespace_and_name(&key.name)
            .map_err(|e| BonjourError::InstallError(e.to_string()))?;
        let fully_qualified_package_name: String =
            fully_qualified_package_display_name(pkg_name, &key.version);
        let package_dir = create_package_dir(&directory, namespace, &fully_qualified_package_name)
            .map_err(|err| {
                BonjourError::InstallError("Could not create package directory".to_string())
            })?;
        let mut response = reqwest::get(download_url.as_ref())
            .map_err(|e| BonjourError::InstallError(e.to_string()))?;
        let temp_dir = tempdir::TempDir::new("wapm_package_install").map_err(|err| {
            BonjourError::InstallError(
                "Failed to create temporary directory to open the package in".to_string(),
            )
        })?;
        let temp_tar_gz_path = temp_dir.path().join("package.tar.gz");
        let mut dest = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_tar_gz_path)
            .map_err(|e| BonjourError::InstallError(e.to_string()))?;
        io::copy(&mut response, &mut dest).map_err(|err| {
            BonjourError::InstallError("Could not copy response to temporary directory".to_string())
        })?;
        Self::decompress_and_extract_archive(dest, &package_dir)
            .map_err(|err| BonjourError::InstallError(format!("{}", err)))?;
        Ok((key, package_dir, download_url.as_ref().to_string()))
    }

    fn decompress_and_extract_archive<P: AsRef<Path>, F: io::Seek + io::Read>(
        mut compressed_archive: F,
        pkg_name: P,
    ) -> Result<(), failure::Error> {
        compressed_archive.seek(SeekFrom::Start(0))?;
        let gz = GzDecoder::new(compressed_archive);
        let mut archive = Archive::new(gz);
        archive
            .unpack(&pkg_name)
            .map_err(|err| BonjourError::InstallError(format!("{}", err)))?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ChangedManifestPackages<'a> {
    pub packages: HashSet<PackageKey<'a>>,
}

impl<'a> ChangedManifestPackages<'a> {
    pub fn prune_unchanged_dependencies(
        manifest_data: ManifestData<'a>,
        lockfile_data: &LockfileData<'a>,
    ) -> Result<Self, BonjourError> {
        let packages = match manifest_data.package_keys {
            Some(m) => {
                let lockfile_keys: HashSet<PackageKey<'a>> =
                    lockfile_data.packages.keys().cloned().collect();
                let differences: HashSet<PackageKey<'a>> =
                    m.difference(&lockfile_keys).cloned().collect();
                differences
            }
            _ => HashSet::new(),
        };
        Ok(Self { packages })
    }
}

struct Ctx<'a> {
    pub manifests: HashMap<PackageKey<'a>, String>,
}

pub fn update<P: AsRef<Path>>(
    added_packages: &Vec<(&str, &str)>,
    directory: P,
) -> Result<(), BonjourError> {
    let directory = directory.as_ref();
    // get manifest data
    let manifest_source = ManifestSource::new(&directory);
    let manifest_result = ManifestResult::from_source(&manifest_source);
    let mut manifest_data = ManifestData::new_from_result(&manifest_result)?;
    // add the extra packages
    manifest_data.add_additional_packages(added_packages);
    let manifest_data = manifest_data;
    // get lockfile data
    let lockfile_string = LockfileSource::new(&directory);
    let lockfile_result = LockfileResult::from_source(&lockfile_string);
    let lockfile_data = LockfileData::new_from_result(lockfile_result)?;

    println!("lockfile: {:?}", lockfile_data);
    println!("manifest: {:?}", manifest_data);

    let pruned_manifest_data =
        ChangedManifestPackages::prune_unchanged_dependencies(manifest_data, &lockfile_data)?;
    println!("pruned_manifest_data: {:?}", pruned_manifest_data);
    let resolved_manifest_packages = ResolvedManifestPackages::new(pruned_manifest_data)?;
    println!(
        "resolved_manifest_packages: {:?}",
        resolved_manifest_packages
    );

    let installed_manifest_packages =
        InstalledManifestPackages::install(&directory, resolved_manifest_packages)?;
    println!(
        "installed_manifest_packages: {:?}",
        installed_manifest_packages
    );

    let manifest_lockfile_data =
        LockfileData::from_installed_packages(&installed_manifest_packages);
    let final_lockfile_data = manifest_lockfile_data.merge(lockfile_data);

    //final_lockfile_data.generate_lockfile(&directory)?;
    Ok(())
}
