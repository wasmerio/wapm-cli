//! Global data definitions used for testing

use std::cell::RefCell;
use std::thread_local;

thread_local! {
    /// The string is the contents of the manifest, the Option is whether or not the manifest exists.
    /// Used to mock reading and writing the manifest to the file system.
    // for now we just have one manifest, a more complex implementation may be useful later
    pub static RAW_MANIFEST_DATA: RefCell<Option<String>> = RefCell::new(None);

    /// The string is the contents of the manifest, the Option is whether or not the manifest exists.
    /// Used to mock reading and writing the manifest to the file system.
    pub static RAW_CONFIG_DATA: RefCell<Option<String>> = RefCell::new(None);
}
