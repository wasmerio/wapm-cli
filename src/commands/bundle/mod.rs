mod assets;
mod builder;
mod compress;
mod header;
mod options;

pub use crate::commands::bundle::options::BundleOpt;
use crate::commands::bundle::builder::Builder;

pub fn bundle(bundle_options: BundleOpt) -> Result<(), failure::Error> {
    Builder::new()
        .add_cli_args(bundle_options)?
        .bundle_and_publish()
}
