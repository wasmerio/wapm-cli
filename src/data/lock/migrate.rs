#[derive(Clone, Debug, Fail)]
pub enum Error {
    #[fail(display = "Lockfile version is missing or invalid. Delete `wapm.lock`.")]
    InvalidOrMissingVersion
}

pub enum LockfileVersion {
    V1,
    V2,
}

impl LockfileVersion {
    pub fn from_lockfile_string(raw_string: &str) -> Result<Self, Error> {
        match raw_string {
            _ if raw_string.starts_with("# Lockfile v1") => Ok(LockfileVersion::V1),
            _ if raw_string.starts_with("# Lockfile v2") => Ok(LockfileVersion::V2),
            _ => Err(Error::InvalidOrMissingVersion)
        }
    }
}

fn migrate_v1_to_v2(raw_string: &str) -> String {

}