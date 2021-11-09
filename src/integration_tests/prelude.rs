//! The functions needed to write integration tests

use crate::commands::*;
use crate::data::manifest::{Manifest, ManifestError};
use crate::util::create_temp_dir;

/// Runs `wapm config set registry.url https://registry.wapm.dev`
pub fn set_registry_to_dev() -> anyhow::Result<()> {
    config(ConfigOpt::set(
        "registry.url".to_string(),
        "https://registry.wapm.dev".to_string(),
    ))
}

pub fn set_test_dir_to_new_temp_dir() -> tempfile::TempDir {
    let new_dir = create_temp_dir().expect("Could not create temp dir");
    let new_cur_dir = new_dir.join("integration_test");
    std::fs::create_dir(&new_cur_dir).expect("Could not create subdir");
    std::env::set_current_dir(new_cur_dir)
        .expect("Could not set current directory to temporary directory");
    new_dir
}

/// Runs `wapm init`
pub fn init_manifest() -> anyhow::Result<()> {
    init(InitOpt::new(true))
}

/// Runs `wapm add`
pub fn add_dependencies(deps: &[&str]) -> anyhow::Result<()> {
    add(AddOpt::new(deps.iter().map(|s| s.to_string()).collect()))
}

/// Runs `wapm remove`
pub fn remove_dependencies(deps: &[&str]) -> anyhow::Result<()> {
    remove(RemoveOpt::new(deps.iter().map(|s| s.to_string()).collect()))
}

/// Gets the thread local Manifest
pub fn get_manifest() -> Result<Manifest, ManifestError> {
    Manifest::find_in_directory("this isn't used in the test impl right now")
}
