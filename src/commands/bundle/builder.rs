use crate::commands::bundle::assets::Assets;
use crate::commands::bundle::compress::ZStdCompression;
use crate::commands::bundle::options::BundleOpt;
use crate::manifest::Target;
use crate::manifest::{get_absolute_manifest_path, Manifest, Source};
use std::fs;
use std::path::PathBuf;

/// A builder for generating Wasm. This structure pulls in data from the CLI and the Manifest.
/// The builder will emit a wasm module as a file.
pub struct Builder {
    source: Option<Source>,
    target: Option<Target>,
    assets: Assets,
}

impl Builder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Builder {
            source: None,
            target: None,
            assets: Assets::new(),
        }
    }

    /// Add an asset to the builder from command-line arguments. The source, target, and assets are added from the manifest.
    pub fn add_cli_args(mut self, cli_options: BundleOpt) -> Result<Self, failure::Error> {
        let manifest_path_buf = get_absolute_manifest_path(cli_options.manifest_file_path)?;
        let base_manifest_path = manifest_path_buf.parent().unwrap();
        let contents = fs::read_to_string(&manifest_path_buf)?;
        let manifest: Manifest = toml::from_str(contents.as_str())?;
        // add assets from command line arguments
        self.assets
            .add_asset_from_pattern(&base_manifest_path, cli_options.assets)?;

        self.source = manifest.source_absolute_path().ok();
        self.target = manifest.target_absolute_path().ok();

        // add assets from manifest if they exist
        if let Some(table) = manifest.fs {
            for pair in table.iter() {
                let local_path = PathBuf::from(pair.0.as_str());
                // assume there is a virtual path_string for now
                let virtual_path_string = pair.1.as_str().unwrap();
                self.assets.add_asset(&local_path, virtual_path_string)?;
            }
        }

        Ok(self)
    }

    /// Eat this builder and emit a Wasm module as a file. This will fail if the required target
    /// and/or source are missing.
    pub fn bundle_and_publish(self) -> Result<(), failure::Error> {
        // fail early if missing required target and source
        let source = self.source.ok_or(BuilderError::MissingSource)?;
        let target = self.target.ok_or(BuilderError::MissingTarget)?;

        // create a walrus module from the source file
        let mut module = walrus::Module::from_file(source)?;

        // insert a custom section with assets if we have one using zstd compression
        if let Some(custom_section) = self.assets.into_custom_section::<ZStdCompression>() {
            module.custom.push(custom_section);
        }

        // publish the wasm module
        module.emit_wasm_file(target)
    }
}

#[derive(Debug, Fail)]
pub enum BuilderError {
    #[fail(display = "Missing target.")]
    MissingTarget,
    #[fail(display = "Missing source.")]
    MissingSource,
}
