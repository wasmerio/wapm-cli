/// Represents the version of this header schema.
#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum HeaderVersion {
    Version1 = 1,
}

/// Represents the compression type of the file data. Only Zstd or no-compression is supported.
#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum CompressionType {
    #[allow(dead_code)]
    NONE = 0,
    ZSTD = 1,
}

/// Represents the type of archive. The only supported archive is the Tar format.
#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum ArchiveType {
    TAR = 0,
}

/// A serializing function of creating the header buffer from parts.
pub fn header_to_bytes(
    version: HeaderVersion,
    compression_type: CompressionType,
    archive_type: ArchiveType,
) -> [u8; 4] {
    [version as _, compression_type as _, archive_type as _, 0]
}

#[cfg(test)]
mod test {
    use crate::commands::bundle::header::{
        header_to_bytes, ArchiveType, CompressionType, HeaderVersion,
    };

    #[test]
    fn does_it_work() {
        let bytes = header_to_bytes(
            HeaderVersion::Version1,
            CompressionType::NONE,
            ArchiveType::TAR,
        );
        assert_eq!(bytes, [1, 0, 0, 0]);
        let bytes = header_to_bytes(
            HeaderVersion::Version1,
            CompressionType::ZSTD,
            ArchiveType::TAR,
        );
        assert_eq!(bytes, [1, 1, 0, 0]);
    }
}
