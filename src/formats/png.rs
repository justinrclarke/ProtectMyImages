//! PNG metadata stripping.
//!
//! PNG files consist of a signature followed by chunks. Each chunk has:
//! - 4 bytes: length (big-endian)
//! - 4 bytes: chunk type (ASCII)
//! - N bytes: data
//! - 4 bytes: CRC32
//!
//! Metadata chunks stripped:
//! - tEXt: Uncompressed text
//! - zTXt: Compressed text
//! - iTXt: International text
//! - eXIf: EXIF data
//! - tIME: Last modification time
//! - pHYs: Physical dimensions (optional, can be metadata)
//!
//! Critical chunks preserved:
//! - IHDR: Image header
//! - PLTE: Palette
//! - IDAT: Image data
//! - IEND: Image end
//! - All other ancillary chunks not in the strip list

use crate::error::{Error, Result};
use std::path::Path;

/// PNG signature bytes.
const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Chunk types that contain metadata and should be stripped.
const METADATA_CHUNKS: &[&[u8; 4]] = &[
    b"tEXt", // Uncompressed text.
    b"zTXt", // Compressed text.
    b"iTXt", // International text.
    b"eXIf", // EXIF data.
    b"tIME", // Modification time.
];

/// Check if a chunk type is a metadata chunk that should be stripped.
fn is_metadata_chunk(chunk_type: &[u8; 4]) -> bool {
    METADATA_CHUNKS.iter().any(|&m| m == chunk_type)
}

/// Calculate CRC32 for PNG chunk validation/creation.
/// Uses hardware-accelerated implementation when available.
fn crc32(data: &[u8]) -> u32 {
    crate::simd::crc32::compute(data)
}

/// A PNG chunk.
#[derive(Debug)]
struct Chunk<'a> {
    chunk_type: [u8; 4],
    data: &'a [u8],
}

impl<'a> Chunk<'a> {
    /// Calculate the CRC for this chunk.
    fn calculate_crc(&self) -> u32 {
        let mut crc_data = Vec::with_capacity(4 + self.data.len());
        crc_data.extend_from_slice(&self.chunk_type);
        crc_data.extend_from_slice(self.data);
        crc32(&crc_data)
    }

    /// Write the chunk to a buffer.
    fn write_to(&self, output: &mut Vec<u8>) {
        // Length.
        output.extend_from_slice(&(self.data.len() as u32).to_be_bytes());
        // Type.
        output.extend_from_slice(&self.chunk_type);
        // Data.
        output.extend_from_slice(self.data);
        // CRC.
        output.extend_from_slice(&self.calculate_crc().to_be_bytes());
    }
}

/// Parse chunks from PNG data.
fn parse_chunks<'a>(data: &'a [u8], path: &Path) -> Result<Vec<Chunk<'a>>> {
    let mut chunks = Vec::new();
    let mut pos = PNG_SIGNATURE.len();

    while pos < data.len() {
        // Read length.
        if pos + 4 > data.len() {
            return Err(Error::invalid_image(path, "Truncated chunk length"));
        }
        let length = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        // Read chunk type.
        if pos + 4 > data.len() {
            return Err(Error::invalid_image(path, "Truncated chunk type"));
        }
        let chunk_type: [u8; 4] = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        pos += 4;

        // Read data.
        if pos + length > data.len() {
            return Err(Error::invalid_image(path, "Truncated chunk data"));
        }
        let chunk_data = &data[pos..pos + length];
        pos += length;

        // Read CRC (we'll recalculate it anyway).
        if pos + 4 > data.len() {
            return Err(Error::invalid_image(path, "Truncated chunk CRC"));
        }
        pos += 4;

        chunks.push(Chunk {
            chunk_type,
            data: chunk_data,
        });

        // Stop at IEND.
        if &chunk_type == b"IEND" {
            break;
        }
    }

    Ok(chunks)
}

/// Strip metadata from PNG data.
pub fn strip(data: &[u8], path: &Path) -> Result<Vec<u8>> {
    // Validate signature.
    if data.len() < PNG_SIGNATURE.len() {
        return Err(Error::invalid_image(path, "File too small to be a valid PNG"));
    }

    if !data.starts_with(&PNG_SIGNATURE) {
        return Err(Error::invalid_image(path, "Invalid PNG signature"));
    }

    // Parse chunks.
    let chunks = parse_chunks(data, path)?;

    // Validate we have required chunks.
    let has_ihdr = chunks.iter().any(|c| &c.chunk_type == b"IHDR");
    let has_iend = chunks.iter().any(|c| &c.chunk_type == b"IEND");

    if !has_ihdr {
        return Err(Error::invalid_image(path, "Missing IHDR chunk"));
    }
    if !has_iend {
        return Err(Error::invalid_image(path, "Missing IEND chunk"));
    }

    // Build output, filtering out metadata chunks.
    let mut output = Vec::with_capacity(data.len());

    // Write signature.
    output.extend_from_slice(&PNG_SIGNATURE);

    // Write non-metadata chunks.
    for chunk in chunks {
        if !is_metadata_chunk(&chunk.chunk_type) {
            chunk.write_to(&mut output);
        }
    }

    Ok(output)
}

/// Create a minimal valid PNG for testing.
#[cfg(test)]
pub fn create_minimal_png() -> Vec<u8> {
    let mut data = Vec::new();

    // Signature.
    data.extend_from_slice(&PNG_SIGNATURE);

    // IHDR chunk (1x1 pixel, 8-bit grayscale).
    let ihdr_data = [
        0x00, 0x00, 0x00, 0x01, // Width: 1
        0x00, 0x00, 0x00, 0x01, // Height: 1
        0x08,                   // Bit depth: 8
        0x00,                   // Color type: grayscale
        0x00,                   // Compression: deflate
        0x00,                   // Filter: adaptive
        0x00,                   // Interlace: none
    ];
    let ihdr = Chunk {
        chunk_type: *b"IHDR",
        data: &ihdr_data,
    };
    ihdr.write_to(&mut data);

    // IDAT chunk (minimal compressed data for 1x1 grayscale).
    // This is zlib-compressed: filter byte (0) + pixel value (0).
    let idat_data = [
        0x78, 0x9C, // zlib header
        0x62, 0x60, 0x00, 0x00, // compressed data
        0x00, 0x02, 0x00, 0x01, // adler32
    ];
    let idat = Chunk {
        chunk_type: *b"IDAT",
        data: &idat_data,
    };
    idat.write_to(&mut data);

    // IEND chunk.
    let iend = Chunk {
        chunk_type: *b"IEND",
        data: &[],
    };
    iend.write_to(&mut data);

    data
}

/// Create a PNG with text metadata for testing.
#[cfg(test)]
pub fn create_png_with_metadata() -> Vec<u8> {
    let mut data = Vec::new();

    // Signature.
    data.extend_from_slice(&PNG_SIGNATURE);

    // IHDR chunk.
    let ihdr_data = [
        0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x01,
        0x08, 0x00, 0x00, 0x00, 0x00,
    ];
    let ihdr = Chunk {
        chunk_type: *b"IHDR",
        data: &ihdr_data,
    };
    ihdr.write_to(&mut data);

    // tEXt chunk (should be stripped).
    let text_data = b"Comment\x00This is a test comment";
    let text = Chunk {
        chunk_type: *b"tEXt",
        data: text_data,
    };
    text.write_to(&mut data);

    // tIME chunk (should be stripped).
    let time_data = [
        0x07, 0xE8, // Year: 2024
        0x06,       // Month: June
        0x15,       // Day: 21
        0x0C,       // Hour: 12
        0x00,       // Minute: 0
        0x00,       // Second: 0
    ];
    let time = Chunk {
        chunk_type: *b"tIME",
        data: &time_data,
    };
    time.write_to(&mut data);

    // IDAT chunk.
    let idat_data = [0x78, 0x9C, 0x62, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01];
    let idat = Chunk {
        chunk_type: *b"IDAT",
        data: &idat_data,
    };
    idat.write_to(&mut data);

    // IEND chunk.
    let iend = Chunk {
        chunk_type: *b"IEND",
        data: &[],
    };
    iend.write_to(&mut data);

    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.png")
    }

    #[test]
    fn test_crc32() {
        // Test with known values.
        let data = b"IEND";
        let crc = crc32(data);
        assert_eq!(crc, 0xAE426082);
    }

    #[test]
    fn test_is_metadata_chunk() {
        assert!(is_metadata_chunk(b"tEXt"));
        assert!(is_metadata_chunk(b"zTXt"));
        assert!(is_metadata_chunk(b"iTXt"));
        assert!(is_metadata_chunk(b"eXIf"));
        assert!(is_metadata_chunk(b"tIME"));

        assert!(!is_metadata_chunk(b"IHDR"));
        assert!(!is_metadata_chunk(b"IDAT"));
        assert!(!is_metadata_chunk(b"IEND"));
        assert!(!is_metadata_chunk(b"PLTE"));
    }

    #[test]
    fn test_strip_minimal_png() {
        let data = create_minimal_png();
        let result = strip(&data, &test_path()).unwrap();

        // Should still have signature.
        assert!(result.starts_with(&PNG_SIGNATURE));

        // Should have IHDR, IDAT, IEND.
        let chunks = parse_chunks(&result, &test_path()).unwrap();
        let types: Vec<_> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(types.contains(&&*b"IHDR"));
        assert!(types.contains(&&*b"IDAT"));
        assert!(types.contains(&&*b"IEND"));
    }

    #[test]
    fn test_strip_png_with_metadata() {
        let data = create_png_with_metadata();
        let result = strip(&data, &test_path()).unwrap();

        // Should be smaller.
        assert!(result.len() < data.len());

        // Should not contain tEXt or tIME chunks.
        let chunks = parse_chunks(&result, &test_path()).unwrap();
        let types: Vec<_> = chunks.iter().map(|c| &c.chunk_type).collect();
        assert!(!types.contains(&&*b"tEXt"));
        assert!(!types.contains(&&*b"tIME"));

        // Should still have essential chunks.
        assert!(types.contains(&&*b"IHDR"));
        assert!(types.contains(&&*b"IDAT"));
        assert!(types.contains(&&*b"IEND"));
    }

    #[test]
    fn test_invalid_too_small() {
        let data = [0x89, 0x50];
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_signature() {
        let data = [0x00; 20];
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_chunk_write() {
        let chunk = Chunk {
            chunk_type: *b"tEST",
            data: &[0x01, 0x02, 0x03],
        };
        let mut output = Vec::new();
        chunk.write_to(&mut output);

        // Should be: length (4) + type (4) + data (3) + crc (4) = 15 bytes.
        assert_eq!(output.len(), 15);

        // Check length.
        let len = u32::from_be_bytes([output[0], output[1], output[2], output[3]]);
        assert_eq!(len, 3);

        // Check type.
        assert_eq!(&output[4..8], b"tEST");

        // Check data.
        assert_eq!(&output[8..11], &[0x01, 0x02, 0x03]);
    }
}
