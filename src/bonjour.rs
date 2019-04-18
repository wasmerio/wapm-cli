use crate::lock::lockfile::LockfileError;
use crate::lock::Lockfile;
use crate::manifest::{Manifest, ManifestError, MANIFEST_FILE_NAME, Package};
use std::collections::btree_map::BTreeMap;
use std::path::{Path, PathBuf};
use std::{env, fs};
use toml::Value;

#[derive(Debug, Fail)]
pub enum BonjourError {
    #[fail(display = "e")]
    E,
}

#[derive(Clone, Debug)]
struct SharedData {
    pub directory: PathBuf,
}

impl SharedData {
    pub fn new(directory: PathBuf) -> Self {
        SharedData { directory }
    }
}

#[derive(Clone, Debug)]
struct ManifestData {
    pub value: Option<Manifest>,
}

impl ManifestData {
    pub fn new(m: Manifest) -> Self {
        ManifestData { value: Some(m) }
    }
}

enum ManifestValue {
    Manifest(Manifest),
    NoManifest,
    ManifestError(ManifestError),
}

#[derive(Clone, Debug)]
enum PackageData {
    ManifestDependency {
        package_name: String,
        package_version: String,
    },
    ManifestPackage {
        package: Package,
    },
}

#[derive(Clone, Debug)]
struct ProjectData {
    pub package_data: Vec<PackageData>,
}

impl ProjectData {
    pub fn new_from_manifest_data(manifest_data: ManifestData) -> Result<Self, BonjourError> {
        match manifest_data.value {
            Some(Manifest {
                package,
                dependencies: Some(dependencies),
                ..
            }) => {
                let package_data = dependencies
                    .into_iter()
                    .map(|(name, value)| match value {
                        Value::String(version) => Ok(PackageData::ManifestDependency {
                            package_name: name,
                            package_version: version,
                        }),
                        _ => Err(BonjourError::E),
                    })
                    .chain(vec![Ok(PackageData::ManifestPackage {
                        package
                    })].into_iter())
                    .collect::<Result<Vec<PackageData>, BonjourError>>()?;
                Ok(ProjectData { package_data })
            }
            _ => Ok(ProjectData {
                package_data: vec![],
            }),
        }
    }

    pub fn new_from_lockfile_data(lockfile_data: LockfileData) -> Result<Self, BonjourError> { Err(BonjourError::E)}
}

struct LockfileData<'a> {
    pub value: Option<Lockfile<'a>>,
    pub source: Option<String>,
}

impl<'a> LockfileData<'a> {
    pub fn new() -> Self {
        LockfileData {
            value: None,
            source: None,
        }
    }
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

fn manifest_source(source: &str) -> Result<ManifestData, BonjourError> {
    toml::from_str::<Manifest>(source)
        .map(ManifestData::new)
        .map_err(|_| BonjourError::E)
}

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

fn create_project_data<'a>(
    manifest_data: ManifestData,
    lockfile_data: LockfileData<'a>,
) -> Result<ProjectData, BonjourError> {
    Err(BonjourError::E)
}

//fn generate_lockfile(project_data: ProjectData) -> Result<LockfileData, BonjourError> {
//    unimplemented!()
//    Ok(LockfileData::new(project_data.shared_data))
//}

fn generate_manifest(project_data: ProjectData) -> Result<ManifestData, BonjourError> {
    unimplemented!()
    //    Ok(ManifestData::new(project_data.shared_data))
}

fn persist_manifest(manifest_data: ManifestData) -> Result<(), BonjourError> {
    match manifest_data.value {
        Some(manifest) => {
            manifest.save();
        }
        _ => {}
    }
    Ok(())
}

//fn persist_lockfile(lockfile_data: LockfileData) -> Result<(), BonjourError> {
//    match lockfile_data.value {
//        Some(lockfile) => {
//            lockfile.save(lockfile_data.shared_data.directory);
//        }
//        _ => {}
//    }
//    Ok(())
//}

pub enum PackageId {
    //    LocalPackage {
    //        directory: &'a Path,
    //    },
    WapmRegistryPackage { name: String, version: String },
    //    GitUrl {
    //        url: &'a str,
    //    },
    //    TestPackage {
    //        name: &'a str,
    //        version: &'a str,
    //    }
}

pub fn update() -> Result<(), BonjourError> {
    let directory = env::current_dir().map_err(|_| BonjourError::E)?;
    let manifest_string = open_manifest_file(&directory)?;
    //    manifest_data
    let manifest_data = manifest_source(&manifest_string)?;

    //    let lockfile_data = lockfile_source(shared_data.clone())?;
    //    let project_data = create_project_data(manifest_data, lockfile_data)?;
    //    let manifest_data = generate_manifest(project_data.clone())?;
    //    let lockfile_data = generate_lockfile(project_data)?;
    //    persist_manifest(manifest_data)?;
    //    persist_lockfile(lockfile_data)?;
    Ok(())
}

struct PackageModel {
    package: PackageId,
}

struct ModuleModel {
    package: PackageId,
    module_name: String,
    download_url: String,
}

struct Model {
    key: usize,
}
