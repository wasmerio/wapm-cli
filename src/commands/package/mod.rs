mod assets;
mod compress;
mod header;
mod options;

use crate::commands::package::assets::Assets;
use crate::commands::package::compress::ZStdCompression;
pub use crate::commands::package::options::PackageOpt;
use crate::manifest::Manifest;
use std::env;
use std::path::PathBuf;
use thiserror::Error;

pub fn package(package_options: PackageOpt) -> anyhow::Result<()> {
    let (manifest, base_path) = match package_options.manifest_file_path {
        Some(manifest_path) => {
            let manifest = Manifest::open(&manifest_path)?;
            let base_path = manifest_path.parent().unwrap().to_path_buf();
            (manifest, base_path)
        }
        None => {
            let manifest = Manifest::find_in_current_directory()?;
            let base_path = crate::config::Config::get_current_dir()?;
            (manifest, base_path)
        }
    };

    let wapm_module = manifest.module.as_ref().ok_or(PackageError::NoModule)?;

    // fail early if missing required source
    let source = manifest
        .source_path()
        .map_err(|_| PackageError::MissingSource)?;

    // add assets from CLI pattern
    //    let base_manifest_path = manifest_path_buf.parent().unwrap();
    let mut assets = Assets::new();
    assets.add_asset_from_pattern(&base_path, package_options.assets)?;
    // add assets from manifest if they exist
    if let Some(table) = &wapm_module.fs {
        for pair in table.iter() {
            let local_path = PathBuf::from(pair.0.as_str());
            // assume there is a virtual path_string for now
            let virtual_path_string = pair.1.as_str().unwrap();
            let local_path = base_path.join(local_path);
            assets.add_asset(&local_path, virtual_path_string)?;
        }
    }

    // create a walrus module from the source file
    let mut module = walrus::Module::from_file(source)?;

    // insert a custom section with assets if we have one using zstd compression
    if let Some(custom_section) = assets.into_custom_section::<ZStdCompression>() {
        module.custom.push(custom_section);
    }

    // because this possibly does not exist yet, simply join to the base path if it is relative
    let module_path = manifest.module_path()?;
    let module_path = if module_path.is_relative() {
        base_path.join(module_path)
    } else {
        module_path
    };

    // publish the wasm module
    module.emit_wasm_file(module_path)
}

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("Missing source.")]
    MissingSource,
    #[error("Cannot package without a module.")]
    NoModule,
}
