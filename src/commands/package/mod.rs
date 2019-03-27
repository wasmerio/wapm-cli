mod assets;
mod compress;
mod header;
mod options;

use std::path::PathBuf;
pub use crate::commands::package::options::PackageOpt;
use crate::commands::package::assets::Assets;
use crate::manifest::{get_absolute_manifest_path, Manifest};
use crate::commands::package::compress::ZStdCompression;

pub fn package(package_options: PackageOpt) -> Result<(), failure::Error> {
    // add cli args
    let manifest_path_buf = get_absolute_manifest_path(package_options.manifest_file_path)?;
    let manifest: Manifest = Manifest::new_from_path(Some(manifest_path_buf.clone()))?;

    // fail early if missing required target and source
    let source = manifest.source_absolute_path().map_err(|_| BundleError::MissingSource)?;
    let target = manifest.target_absolute_path().map_err(|_| BundleError::MissingTarget)?;

    // add assets from CLI pattern
    let base_manifest_path = manifest_path_buf.parent().unwrap();
    let mut assets = Assets::new();
    assets
        .add_asset_from_pattern(&base_manifest_path, package_options.assets)?;
    // add assets from manifest if they exist
    if let Some(table) = manifest.fs {
        for pair in table.iter() {
            let local_path = PathBuf::from(pair.0.as_str());
            // assume there is a virtual path_string for now
            let virtual_path_string = pair.1.as_str().unwrap();
            assets.add_asset(&local_path, virtual_path_string)?;
        }
    }

    // create a walrus module from the source file
    let mut module = walrus::Module::from_file(source)?;

    // insert a custom section with assets if we have one using zstd compression
    if let Some(custom_section) = assets.into_custom_section::<ZStdCompression>() {
        module.custom.push(custom_section);
    }

    // publish the wasm module
    module.emit_wasm_file(target)
}

#[derive(Debug, Fail)]
pub enum BundleError {
    #[fail(display = "Missing target.")]
    MissingTarget,
    #[fail(display = "Missing source.")]
    MissingSource,
}
