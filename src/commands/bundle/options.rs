use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "bundle", about = "Bundle a package with assets.")]
pub struct BundleOpt {
    /// path to the manifest file (wasmer.toml) for the wasm bundle
    #[structopt(short = "m", long = "manifest-path", parse(from_os_str))]
    pub manifest_file_path: Option<PathBuf>,
    /// Assets to be embedded into the wasm bundle
    #[structopt(short = "a", long = "assets", raw(multiple = "true"))]
    pub assets: Vec<String>,
}
