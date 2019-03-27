use std::path::PathBuf;
use structopt::StructOpt;

/// This command produces a wasm package from a manifest file (wapm.toml). By default, this command
/// looks in the current directory for the manifest file. One may also pass the path to the file
/// with a flag.
#[derive(Debug, StructOpt)]
#[structopt(name = "package", about = "Bundle a package with assets.")]
pub struct PackageOpt {
    /// Path to the manifest file (wasmer.toml) for the wasm package.
    #[structopt(short = "m", long = "manifest-path", parse(from_os_str))]
    pub manifest_file_path: Option<PathBuf>,
    /// Assets to be bundled in the wasm package. This is a comma delimited list of patterns
    /// e.g. `foo.txt:foo.txt,bar.txt:other/place/bar.txt`.
    #[structopt(short = "a", long = "assets", raw(multiple = "true"))]
    pub assets: Vec<String>,
}
