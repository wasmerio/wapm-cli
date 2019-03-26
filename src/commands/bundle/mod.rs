mod assets;
mod builder;
mod compress;
mod header;
mod options;

pub use self::options::BundleOpt;

pub fn bundle(_bundle_options: BundleOpt) -> Result<(), failure::Error> {
    Ok(())
}
