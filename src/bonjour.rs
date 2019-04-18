use crate::lock::lockfile::LockfileError;
use crate::lock::{Lockfile, LOCKFILE_NAME, LockfileModule, LockfileCommand};
use crate::manifest::{Manifest, ManifestError, MANIFEST_FILE_NAME, Package};
use std::path::{Path, PathBuf};
use std::{env, fs};
use toml::Value;
use std::collections::btree_map::BTreeMap;

#[derive(Clone, Debug, Fail)]
pub enum BonjourError {
    #[fail(display = "e")]
    E,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PackageId<'a> {
    LocalPackage {
        directory: &'a Path,
    },
    WapmRegistryPackage {
        name: &'a str,
        version: &'a str,
    },
    //    GitUrl {
    //        url: &'a str,
    //    },
    //    TestPackage {
    //        name: &'a str,
    //        version: &'a str,
    //    }
}

fn lockfile_to_packages<'a>(lockfile: Lockfile<'a>) -> Result<(Vec<PackageId<'a>>, BTreeMap<PackageId<'a>, PackageData<'a>>), BonjourError> {
    let package_names = lockfile.modules.keys().cloned().collect::<Vec<&str>>();
    let package_versions = package_names.iter().cloned().map(|n| {
        lockfile.modules.get(n).unwrap().keys().map(|v| PackageId::WapmRegistryPackage { name: n.clone(), version: v, }).collect::<Vec<PackageId<'a>>>()
    });
    unimplemented!()
}

pub enum UnresolvedPackage {

}

pub enum InstalledPackage {
}

pub enum PackageData<'a> {
    LockfilePackage {
        modules: Vec<LockfileModule<'a>>,
        commands: Vec<LockfileCommand<'a>>,
    },
    ManifestDependencyPackage {

    },
    ManifestPackage {
    },
}

struct LockfilePackage<'a> {
    pub modules: Vec<LockfileModule<'a>>,
    pub commands: Vec<LockfileCommand<'a>>,
}

#[derive(Clone, Debug)]
struct ManifestData {
    pub value: Option<Manifest>,
}

impl<'a> ManifestData {
    pub fn new(m: Manifest) -> Self {
        ManifestData { value: Some(m) }
    }
    pub fn get_packages(&'a self) -> Option<Vec<PackageId<'a>>> { unimplemented!() }
}

struct LockfileData<'a> {
    pub value: Option<Lockfile<'a>>,
}

impl<'a> LockfileData<'a> {
    pub fn new(lockfile: Lockfile<'a>) -> Self {
        LockfileData {
            value: Some(lockfile),
        }
    }
    pub fn get_packages(&'a self) -> Option<(PackageId<'a>, LockfilePackage)> { unimplemented!() }
}

enum LockfileValue<'a> {
    Lockfile(Lockfile<'a>),
    NoLockfile,
    LockfileError(LockfileError),
}

fn open_manifest_file(directory: &Path) -> Result<String, BonjourError> {
    let manifest_path_buf = directory.join(MANIFEST_FILE_NAME);
    fs::read_to_string(&manifest_path_buf).map_err(|_| BonjourError::E)
}

/// Construct a ManifestData from a string source
fn manifest_source(source: &String) -> Result<ManifestData, BonjourError> {
    toml::from_str::<Manifest>(source)
        .map(ManifestData::new)
        .map_err(|_| BonjourError::E)
}

fn open_lockfile(directory: &Path) -> Result<String, BonjourError> {
    let lockfile_path_buf = directory.join(LOCKFILE_NAME);
    fs::read_to_string(&lockfile_path_buf).map_err(|_| BonjourError::E)
}

/// Construct a ManifestData from a string source
fn lockfile_source(source: &String) -> Result<LockfileData, BonjourError> {
    toml::from_str::<Lockfile>(source)
        .map(LockfileData::new)
        .map_err(|_| BonjourError::E)
}

struct PackageManager;

impl<'a> PackageManager {
    fn new() -> Self {
        PackageManager
    }
    fn add_packages_by_package_ids(&mut self, ids: Vec<PackageId>) -> Result<(), BonjourError> { unimplemented!() }
}

struct BonjourLogger;
impl BonjourLogger {
    fn log_err(error: BonjourError) {}
    fn log_info(msg: String) {}
}

pub fn update() -> Result<(), BonjourError> {
    let directory = env::current_dir().map_err(|_| BonjourError::E)?;
    let manifest_string = open_manifest_file(&directory);
    let manifest_data = manifest_string.as_ref()
        .map(manifest_source)
        .map_err(BonjourError::clone)?.ok();
    let lockfile_string = open_lockfile(&directory);
    let lockfile_data = lockfile_string.as_ref()
        .map(lockfile_source)
        .map_err(BonjourError::clone)?.ok();

    let mut pkg_mgr = PackageManager::new();

    manifest_data.as_ref()
        .map(ManifestData::get_packages)
        .unwrap_or_default()
        .map(|manifest_package_ids| {
            pkg_mgr.add_packages_by_package_ids(manifest_package_ids)
        }).unwrap_or(Ok(()))?;

//    lockfile_data.as_ref()
//        .map(LockfileData::get_packages)
//        .unwrap_or_default()
//        .map(|lockfile_package_ids| {
//            pkg_mgr.add_packages_by_package_ids(manifest_package_ids)
//        }).unwrap_or(Ok(()))?;
    //    let lockfile_data = lockfile_source(shared_data.clone())?;
    //    let project_data = create_project_data(manifest_data, lockfile_data)?;
    //    let manifest_data = generate_manifest(project_data.clone())?;
    //    let lockfile_data = generate_lockfile(project_data)?;
    //    persist_manifest(manifest_data)?;
    //    persist_lockfile(lockfile_data)?;
    Ok(())
}


//#[derive(Clone, Debug)]
//enum PackageData {
//    ManifestDependency {
//        package_name: String,
//        package_version: String,
//    },
//    ManifestPackage {
//        package: Package,
//    },
////    LockfilePackage {
////        package_name: String,
////        package_version: String,
////        modules: Vec<LockfileModule>,
////        commands: Vec<LockfileCommand>,
////    },
//}
//
//#[derive(Clone, Debug)]
//struct ProjectData {
//    pub package_data: Vec<PackageData>,
//}
//
//impl ProjectData {
//    pub fn new_from_manifest_data(manifest_data: ManifestData) -> Result<Self, BonjourError> {
//        match manifest_data.value {
//            Some(Manifest {
//                     package,
//                     dependencies: Some(dependencies),
//                     ..
//                 }) => {
//                let package_data = dependencies
//                    .into_iter()
//                    .map(|(name, value)| match value {
//                        Value::String(version) => Ok(PackageData::ManifestDependency {
//                            package_name: name,
//                            package_version: version,
//                        }),
//                        _ => Err(BonjourError::E),
//                    })
//                    .chain(vec![Ok(PackageData::ManifestPackage {
//                        package
//                    })].into_iter())
//                    .collect::<Result<Vec<PackageData>, BonjourError>>()?;
//                Ok(ProjectData { package_data })
//            }
//            _ => Ok(ProjectData {
//                package_data: vec![],
//            }),
//        }
//    }
//
//    pub fn new_from_lockfile_data(lockfile_data: LockfileData) -> Result<Self, BonjourError> { Err(BonjourError::E)}
//}

//fn lockfile_source<'a, 's: 'a>(shared_data: SharedData<'s>) -> Result<LockfileData<'a>, BonjourError> {
//    let manifest_path_buf = shared_data.directory.join(MANIFEST_FILE_NAME);
//    match fs::read_to_string(&manifest_path_buf) {
//        Ok(contents) => {
//            let mut lockfile_data = LockfileData {
//                value: None,
//                source: Some(contents),
//                shared_data,
//            };
//            if let Some(ref contents) = lockfile_data.source {
//                match toml::from_str::<'a, Lockfile<'a>>(contents.as_str()) {
//                    Ok(lockfile) => {
//                        lockfile_data.value = Some(lockfile);
//                    },
//                    Err(toml_error) => {
//                        return Err(BonjourError::E);
//                    }
//                }
//            }
//            Ok(lockfile_data)
//        },
//        Err(_io_error) => {
//            Ok(LockfileData {
//                value: None,
//                source: None,
//                shared_data,
//            })
//        }
//    }
//}

//fn manifest_data_to_project_data(manifest_data: ManifestData) -> ProjectData {
//
//}

//fn create_project_data<'a>(
//    manifest_data: ManifestData,
//    lockfile_data: LockfileData<'a>,
//) -> Result<ProjectData, BonjourError> {
//    Err(BonjourError::E)
//}

//fn generate_lockfile(project_data: ProjectData) -> Result<LockfileData, BonjourError> {
//    unimplemented!()
//    Ok(LockfileData::new(project_data.shared_data))
//}

//fn generate_manifest(project_data: ProjectData) -> Result<ManifestData, BonjourError> {
//    unimplemented!()
//    //    Ok(ManifestData::new(project_data.shared_data))
//}

//fn persist_manifest(manifest_data: ManifestData) -> Result<(), BonjourError> {
//    match manifest_data.value {
//        Some(manifest) => {
//            manifest.save();
//        }
//        _ => {}
//    }
//    Ok(())
//}

//fn persist_lockfile(lockfile_data: LockfileData) -> Result<(), BonjourError> {
//    match lockfile_data.value {
//        Some(lockfile) => {
//            lockfile.save(lockfile_data.shared_data.directory);
//        }
//        _ => {}
//    }
//    Ok(())
//}