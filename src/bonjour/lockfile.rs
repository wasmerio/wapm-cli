use crate::bonjour::{BonjourError, PackageData, PackageId};
use crate::lock::{Lockfile, LockfileCommand};
use std::collections::btree_map::BTreeMap;

#[derive(Debug)]
pub enum LockfileResult<'a> {
    Lockfile(Lockfile<'a>),
    NoLockfile,
    LockfileError(BonjourError),
}

impl<'a> LockfileResult<'a> {
    pub fn from_optional_source(source: &'a Option<String>) -> LockfileResult {
        source
            .as_ref()
            .map(|source| match toml::from_str::<Lockfile>(source) {
                Ok(l) => LockfileResult::Lockfile(l),
                Err(e) => LockfileResult::LockfileError(BonjourError::LockfileTomlParseError(
                    e.to_string(),
                )),
            })
            .unwrap_or(LockfileResult::NoLockfile)
    }
}

impl<'a> Default for LockfileResult<'a> {
    fn default() -> Self {
        LockfileResult::NoLockfile
    }
}

pub struct LockfileData<'a> {
    pub package_data: BTreeMap<PackageId<'a>, PackageData<'a>>,
}

impl<'a> LockfileData<'a> {
    pub fn new_from_result(result: LockfileResult<'a>) -> Result<Option<Self>, BonjourError> {
        match result {
            LockfileResult::Lockfile(l) => Ok(Some(Self::new_from_lockfile(l))),
            LockfileResult::NoLockfile => Ok(None),
            LockfileResult::LockfileError(e) => return Err(e),
        }
    }

    fn new_from_lockfile(lockfile: Lockfile<'a>) -> LockfileData {
        let (raw_lockfile_modules, raw_lockfile_commands) = (lockfile.modules, lockfile.commands);

        let mut lockfile_commands_map: BTreeMap<PackageId, Vec<LockfileCommand>> = BTreeMap::new();
        for (_name, command) in raw_lockfile_commands {
            let command: LockfileCommand<'a> = command;
            let id = PackageId::new_registry_package(command.package_name, command.package_version);
            let command_vec = lockfile_commands_map.entry(id).or_default();
            command_vec.push(command);
        }

        let package_data: BTreeMap<PackageId, PackageData> = raw_lockfile_modules
            .into_iter()
            .map(|(pkg_name, pkg_versions)| {
                pkg_versions
                    .into_iter()
                    .map(|(pkg_version, modules)| {
                        let id =
                            PackageId::new_registry_package(pkg_name.clone(), pkg_version.clone());
                        println!("id: {:?}", id);
                        let lockfile_modules = modules
                            .into_iter()
                            .map(|(_module_name, module)| module)
                            .collect::<Vec<_>>();
                        let lockfile_commands = lockfile_commands_map.remove(&id).unwrap_or(vec![]);
                        let package_data = PackageData::LockfilePackage {
                            modules: lockfile_modules,
                            commands: lockfile_commands,
                        };
                        (id, package_data)
                    })
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<BTreeMap<_, _>>();

        Self { package_data }
    }
}
