use crate::commands::package::compress::Compress;
use crate::commands::package::header::{header_to_bytes, ArchiveType, HeaderVersion};
use regex::Regex;
use std::path::Path;
use std::path::PathBuf;
use tar::Builder;
use thiserror::Error;

/// The section name that is to be used when constructing `CustomSection`.
static ASSETS_CUSTOM_SECTION_NAME: &str = "wasmer:fs";

/// Assets represent files that are to be embedded into a Wasm module as Custom Sections. The file
/// data is stored in a [Tar](https://www.gnu.org/software/tar/) archive and is lazily constructed.
/// The API supports adding from string patterns (as on the CLI) or with distinct paths (like in the
/// manifest).
pub struct Assets(Option<tar::Builder<Vec<u8>>>);

impl Assets {
    /// Construct a new `Assets`. The archive will be constructed when adding assets.
    pub fn new() -> Self {
        Assets(None)
    }
    /// Add an asset with strings of the file paths. The `virtual_file_path` will be the path of the
    /// file in the archive and the path of the file when mounted by WebAssembly runtimes.
    pub fn add_asset(
        &mut self,
        local_path: &Path,
        virtual_file_path: &str,
    ) -> anyhow::Result<()> {
        if local_path.is_file() {
            let ar = self.0.get_or_insert(Builder::new(vec![]));
            ar.append_path_with_name(local_path, virtual_file_path)
                .map_err(|e| e.into())
        } else if local_path.is_dir() {
            let virtual_path_buf = PathBuf::from(virtual_file_path);
            let ar = self.0.get_or_insert(Builder::new(vec![]));
            ar.append_dir_all(virtual_path_buf, local_path)
                .map_err(|e| e.into())
        } else {
            use path_slash::PathExt;
            let path_string = local_path
                .to_slash()
                .unwrap_or(local_path.display().to_string());
            Err(AssetsError::InvalidAsset(path_string).into())
        }
    }
    /// Adds an asset with a string in the format `local_path:virtual_path`. The `virtual_file_path`
    /// will be the path of the file in the archive and the path of the file when mounted by
    /// WebAssembly runtimes.
    pub fn add_asset_from_pattern(
        &mut self,
        base_path: &Path,
        cli_arg_patterns: Vec<String>,
    ) -> anyhow::Result<()> {
        // a lazy regex that matches the command line args e.g C:/foo.txt:/foo.txt
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r"(?P<local_path>.*:?.*):(?P<virtual_path>.*)").unwrap();
        }
        for pattern in cli_arg_patterns.iter() {
            let caps = RE.captures(pattern).unwrap();
            let local_path = &caps["local_path"];
            let virtual_path = &caps["virtual_path"];
            let local_path = base_path.join(PathBuf::from(local_path));
            self.add_asset(local_path.as_path(), virtual_path)?
        }
        Ok(())
    }

    /// Eat this `Assets` structure and produce a `CustomSection`. Will be `None` if no assets were
    /// added. Will compress the data.
    pub fn into_custom_section<Compressor: Compress>(self) -> Option<walrus::CustomSection> {
        self.0.map(|ar| {
            let data = ar.into_inner().unwrap();
            // create default
            let header_bytes = header_to_bytes(
                HeaderVersion::Version1,
                Compressor::compression_type(),
                ArchiveType::TAR,
            );
            // compress the data
            let compressed_data = Compressor::compress(data.clone()).unwrap();
            // join the header and the compressed data
            let header_and_compressed_data_bytes =
                [&header_bytes[..], &compressed_data[..]].concat();
            walrus::CustomSection {
                name: ASSETS_CUSTOM_SECTION_NAME.to_string(),
                value: header_and_compressed_data_bytes,
            }
        })
    }
}

#[derive(Debug, Error)]
pub enum AssetsError {
    #[error("Path is not directory or file: \"{0}\"")]
    InvalidAsset(String),
}

#[cfg(test)]
mod test {
    use crate::commands::package::assets::Assets;
    use crate::commands::package::assets::ASSETS_CUSTOM_SECTION_NAME;
    use crate::commands::package::compress::NoCompression;
    use crate::commands::package::header::{ArchiveType, CompressionType, HeaderVersion};
    use std::fs;
    use std::fs::File;
    use std::io::{Read, Write};
    use std::path::PathBuf;
    use walrus::CustomSection;

    // helper for unwrapping custom section blob
    // may evolve if we add compression
    fn assert_custom_section_data(
        custom_section: &CustomSection,
        expected_file_path: &str,
        expected_file_contents: &str,
        expected_archive_type: ArchiveType,
        expected_compression_type: CompressionType,
    ) {
        let data = &custom_section.value;

        // assert the header
        let header_data = &data[..4];
        let header = header_from_bytes(header_data);
        assert!(header.is_some(), "Failed to read header.");
        let (_, actual_compression_type, actual_archive_type) = header.unwrap();
        assert_eq!(
            expected_compression_type, actual_compression_type,
            "Compression types were different."
        );
        assert_eq!(
            expected_archive_type, actual_archive_type,
            "Archive types were different."
        );

        // assert the payload
        let payload_data = &data[4..];
        let mut ar = tar::Archive::new(&payload_data[..]);
        let mut contains_file = false;
        for file in ar.entries().unwrap() {
            let mut file = file.unwrap();
            let actual_file_path = file.header().path().unwrap();
            let actual_file_path_str = actual_file_path.to_str().unwrap();
            if actual_file_path_str == expected_file_path {
                contains_file = true;
                let mut actual_file_contents = String::new();
                file.read_to_string(&mut actual_file_contents).unwrap();
                assert_eq!(
                    expected_file_contents,
                    actual_file_contents.as_str(),
                    "file contents do not match"
                );
            }
        }

        if !contains_file {
            panic!("file path not found in custom section");
        }
    }

    #[test]
    fn empty_custom_section() {
        let assets = Assets::new();
        let custom_section = assets.into_custom_section::<NoCompression>();
        assert!(
            custom_section.is_none(),
            "Custom section was non-empty for empty assets"
        );
    }

    #[test]
    fn single_asset_in_root() {
        let tmp_dir = tempdir::TempDir::new("single_asset_in_root").unwrap();
        let file_path = tmp_dir.path().join("foo.txt");
        let mut tmp_file = File::create(file_path.clone()).unwrap();
        writeln!(tmp_file, "foo foo foo").unwrap();

        // an pattern "path/to/tmp/dir/foo.txt:./foo.txt"
        let file_path_display = file_path.display().to_string();
        let cli_arg_pattern = format!("{}:foo.txt", file_path_display);
        let mut cli_arg_patterns = vec![];
        cli_arg_patterns.push(cli_arg_pattern);

        let mut assets = Assets::new();
        let root = PathBuf::from(".");

        let add_result = assets.add_asset_from_pattern(&root, cli_arg_patterns);
        assert!(add_result.is_ok(), "Adding asset failed.");
        let custom_section = assets.into_custom_section::<NoCompression>();

        assert!(
            custom_section.is_some(),
            "Custom section was none when given an asset."
        );
        let custom_section = custom_section.unwrap();
        let custom_section_name = custom_section.name.clone();
        let custom_section_data = custom_section.value.clone();

        // assert the custom section name
        assert_eq!(
            custom_section_name.as_str(),
            ASSETS_CUSTOM_SECTION_NAME,
            "Incorrect custom section name"
        );
        // assert the custom section data
        assert!(!custom_section_data.is_empty(), "Custom section is empty");
        assert_custom_section_data(
            &custom_section,
            "foo.txt",
            "foo foo foo\n",
            ArchiveType::TAR,
            CompressionType::NONE,
        );
    }

    #[test]
    fn single_asset_in_sub_directory() {
        let tmp_dir = tempdir::TempDir::new("single_asset_in_root").unwrap();
        let file_path = tmp_dir.path().join("foo.txt");
        let mut tmp_file = File::create(file_path.clone()).unwrap();
        writeln!(tmp_file, "foo foo foo").unwrap();

        let file_path_display = file_path.display().to_string();
        let cli_arg_pattern = format!("{}:the/sub/dir/foo.txt", file_path_display);
        let mut cli_arg_patterns = vec![];
        cli_arg_patterns.push(cli_arg_pattern);

        let mut assets = Assets::new();
        let root = PathBuf::from(".");

        let add_result = assets.add_asset_from_pattern(&root, cli_arg_patterns);
        assert!(add_result.is_ok(), "Adding asset failed.");
        let custom_section = assets.into_custom_section::<NoCompression>();

        assert!(
            custom_section.is_some(),
            "Custom section was none when given an asset."
        );
        let custom_section = custom_section.unwrap();
        let custom_section_name = custom_section.name.clone();
        let custom_section_data = custom_section.value.clone();

        // assert the custom section name
        assert_eq!(
            custom_section_name.as_str(),
            ASSETS_CUSTOM_SECTION_NAME,
            "Incorrect custom section name"
        );
        // assert the custom section data
        assert!(!custom_section_data.is_empty(), "Custom section is empty");
        assert_custom_section_data(
            &custom_section,
            "the/sub/dir/foo.txt",
            "foo foo foo\n",
            ArchiveType::TAR,
            CompressionType::NONE,
        );
    }

    #[test]
    fn two_assets_in_root() {
        let tmp_dir = tempdir::TempDir::new("two_assets_in_root").unwrap();
        let root = PathBuf::from(".");

        // first file
        let foo_file_path = tmp_dir.path().join("foo.txt");
        let mut foo_tmp_file = File::create(foo_file_path.clone()).unwrap();
        writeln!(foo_tmp_file, "foo foo foo").unwrap();

        // second file
        let bar_file_path = tmp_dir.path().join("bar.txt");
        let mut bar_tmp_file = File::create(bar_file_path.clone()).unwrap();
        writeln!(bar_tmp_file, "bar bar").unwrap();

        let foo_file_path_display = foo_file_path.display().to_string();
        let bar_file_path_display = bar_file_path.display().to_string();

        let mut cli_arg_patterns = vec![];
        cli_arg_patterns.push(format!("{}:foo.txt", foo_file_path_display));
        cli_arg_patterns.push(format!("{}:bar.txt", bar_file_path_display));

        let mut assets = Assets::new();
        let add_result = assets.add_asset_from_pattern(&root, cli_arg_patterns);
        assert!(add_result.is_ok(), "Adding asset failed.");
        let custom_section = assets.into_custom_section::<NoCompression>();

        assert!(
            custom_section.is_some(),
            "Custom section was none when given an asset."
        );
        let custom_section = custom_section.unwrap();
        let custom_section_name = custom_section.name.clone();
        let custom_section_data = custom_section.value.clone();

        // assert the custom section name
        assert_eq!(
            custom_section_name.as_str(),
            ASSETS_CUSTOM_SECTION_NAME,
            "Incorrect custom section name"
        );
        // assert the custom section data
        assert!(!custom_section_data.is_empty(), "Custom section is empty");

        assert_custom_section_data(
            &custom_section,
            "foo.txt",
            "foo foo foo\n",
            ArchiveType::TAR,
            CompressionType::NONE,
        );
        assert_custom_section_data(
            &custom_section,
            "bar.txt",
            "bar bar\n",
            ArchiveType::TAR,
            CompressionType::NONE,
        );
    }

    #[test]
    fn dir_in_subdir() {
        let tmp_dir = tempdir::TempDir::new("dir_in_subdir").unwrap();
        let root = PathBuf::from(".");

        // the dir to package
        let my_dir = tmp_dir.path().join("my_dir");
        let _ = fs::create_dir(my_dir.as_path()).unwrap();

        // first file
        let foo_file_path = my_dir.clone().join("foo.txt");
        let mut foo_tmp_file = File::create(foo_file_path).unwrap();
        writeln!(foo_tmp_file, "foo foo foo").unwrap();

        // second file
        let bar_file_path = my_dir.clone().join("bar.txt");
        let mut bar_tmp_file = File::create(bar_file_path.clone()).unwrap();
        writeln!(bar_tmp_file, "bar bar").unwrap();

        let display = my_dir.display().to_string();

        let mut cli_arg_patterns = vec![];
        cli_arg_patterns.push(format!("{}:my_dir/my_nested_dir", display));

        let mut assets = Assets::new();
        let add_result = assets.add_asset_from_pattern(&root, cli_arg_patterns);
        assert!(add_result.is_ok(), "Adding asset failed.");
        let custom_section = assets.into_custom_section::<NoCompression>();

        assert!(
            custom_section.is_some(),
            "Custom section was none when given an asset."
        );
        let custom_section = custom_section.unwrap();
        let custom_section_name = custom_section.name.clone();
        let custom_section_data = custom_section.value.clone();

        // assert the custom section name
        assert_eq!(
            custom_section_name.as_str(),
            ASSETS_CUSTOM_SECTION_NAME,
            "Incorrect custom section name"
        );
        // assert the custom section data
        assert!(!custom_section_data.is_empty(), "Custom section is empty");

        assert_custom_section_data(
            &custom_section,
            "my_dir/my_nested_dir/foo.txt",
            "foo foo foo\n",
            ArchiveType::TAR,
            CompressionType::NONE,
        );
        assert_custom_section_data(
            &custom_section,
            "my_dir/my_nested_dir/bar.txt",
            "bar bar\n",
            ArchiveType::TAR,
            CompressionType::NONE,
        );
    }

    /// A helper for unpacking the header.
    pub fn header_from_bytes(
        bytes: &[u8],
    ) -> Option<(HeaderVersion, CompressionType, ArchiveType)> {
        if let Some(bytes) = bytes.get(..4) {
            let version = match bytes[0] {
                1 => HeaderVersion::Version1,
                _ => return None,
            };
            let compression_type = match bytes[1] {
                0 => CompressionType::NONE,
                1 => CompressionType::ZSTD,
                _ => return None,
            };
            let archive_type = match bytes[2] {
                0 => ArchiveType::TAR,
                _ => return None,
            };
            Some((version, compression_type, archive_type))
        } else {
            None
        }
    }
}
