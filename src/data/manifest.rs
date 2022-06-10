//! The Manifest file is where the core metadata of a wapm package lives
pub use wapm_toml::{
    MANIFEST_FILE_NAME, 
    PACKAGES_DIR_NAME,
    Package,
    Command,
    CommandV1,
    CommandV2,
    Module,
    Manifest,
    ManifestError,
    ValidationError,
};
