use crate::commands::bundle::header::CompressionType;

/// A general way to talk about compression algorithms. This allows the bundler to use different
/// kinds of compression when storing assets in the wasm.
pub trait Compress {
    fn compress(uncompressed_data: Vec<u8>) -> Result<Vec<u8>, failure::Error>;
    fn compression_type() -> CompressionType;
}

static ZSTD_COMPRESSION_LEVEL: i32 = 3;

/// [zstd compression](https://facebook.github.io/zstd/)
/// Construction is disallowed.
pub struct ZStdCompression {
    _private: (),
}

impl Compress for ZStdCompression {
    fn compress(uncompressed_data: Vec<u8>) -> Result<Vec<u8>, failure::Error> {
        zstd::stream::encode_all(&uncompressed_data[..], ZSTD_COMPRESSION_LEVEL)
            .map_err(|e| e.into())
    }

    fn compression_type() -> CompressionType {
        CompressionType::ZSTD
    }
}

/// A non-compression Compression! Useful for unit tests.
/// Construction is disallowed.
#[cfg(test)]
pub struct NoCompression {
    _private: (),
}

#[cfg(test)]
impl Compress for NoCompression {
    fn compress(uncompressed_data: Vec<u8>) -> Result<Vec<u8>, failure::Error> {
        Ok(uncompressed_data)
    }

    fn compression_type() -> CompressionType {
        CompressionType::NONE
    }
}
