//! The Manifest file is where the core metadata of a wapm package lives
pub use wapm_toml::{
    Command, CommandV1, CommandV2, Manifest, ManifestError, Module, Package, ValidationError,
    MANIFEST_FILE_NAME, PACKAGES_DIR_NAME,
};
