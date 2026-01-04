//! WebP metadata stripping.
//!
//! WebP is a RIFF-based container format with the following structure:
//! - RIFF header (4 bytes): "RIFF"
//! - File size (4 bytes, little-endian)
//! - WebP identifier (4 bytes): "WEBP"
//! - Chunks
//!
//! Each chunk has:
//! - FourCC identifier (4 bytes)
//! - Chunk size (4 bytes, little-endian)
//! - Chunk data (padded to even length)
//!
//! Metadata chunks stripped:
//! - EXIF: EXIF metadata
//! - XMP : XMP metadata
//!
//! Chunks preserved:
//! - VP8 : Lossy image data
//! - VP8L: Lossless image data
//! - VP8X: Extended file format header
//! - ALPH: Alpha channel data
//! - ANIM: Animation parameters
//! - ANMF: Animation frame data
//! - ICCP: ICC profile (considered essential for color accuracy)

use crate::error::{Error, Result};
use std::path::Path;

/// RIFF header.
const RIFF: &[u8; 4] = b"RIFF";

/// WebP identifier.
const WEBP: &[u8; 4] = b"WEBP";

/// Chunk types that contain metadata and should be stripped.
const METADATA_CHUNKS: &[&[u8; 4]] = &[
    b"EXIF", // EXIF metadata.
    b"XMP ", // XMP metadata (note: padded with space).
];

/// Check if a chunk is a metadata chunk that should be stripped.
fn is_metadata_chunk(fourcc: &[u8; 4]) -> bool {
    METADATA_CHUNKS.iter().any(|&m| m == fourcc)
}

/// A WebP chunk.
#[derive(Debug)]
struct Chunk<'a> {
    fourcc: [u8; 4],
    data: &'a [u8],
}

impl<'a> Chunk<'a> {
    /// Get the padded size (chunks are padded to even length).
    fn padded_size(&self) -> usize {
        (self.data.len() + 1) & !1
    }

    /// Write the chunk to output.
    fn write_to(&self, output: &mut Vec<u8>) {
        output.extend_from_slice(&self.fourcc);
        output.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        output.extend_from_slice(self.data);
        // Pad to even length.
        if self.data.len() % 2 != 0 {
            output.push(0);
        }
    }
}

/// Parse chunks from WebP data.
fn parse_chunks<'a>(data: &'a [u8], path: &Path) -> Result<Vec<Chunk<'a>>> {
    let mut chunks = Vec::new();
    let mut pos = 12; // Skip RIFF header + size + WEBP.

    while pos < data.len() {
        // Read FourCC.
        if pos + 4 > data.len() {
            break; // End of file, might be padding.
        }
        let fourcc: [u8; 4] = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        pos += 4;

        // Read size.
        if pos + 4 > data.len() {
            return Err(Error::invalid_image(path, "Truncated chunk size"));
        }
        let size = u32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        // Read data.
        if pos + size > data.len() {
            return Err(Error::invalid_image(path, "Truncated chunk data"));
        }
        let chunk_data = &data[pos..pos + size];

        chunks.push(Chunk {
            fourcc,
            data: chunk_data,
        });

        // Skip to next chunk (padded to even).
        pos += (size + 1) & !1;
    }

    Ok(chunks)
}

/// Update VP8X flags to remove EXIF and XMP flags.
fn update_vp8x_flags(vp8x_data: &[u8]) -> Vec<u8> {
    if vp8x_data.len() < 4 {
        return vp8x_data.to_vec();
    }

    let mut result = vp8x_data.to_vec();

    // VP8X flags are in the first byte.
    // Bit 3 (0x08): XMP metadata present.
    // Bit 5 (0x20): EXIF metadata present.
    result[0] &= !(0x08 | 0x20);

    result
}

/// Strip metadata from WebP data.
pub fn strip(data: &[u8], path: &Path) -> Result<Vec<u8>> {
    // Validate minimum size.
    if data.len() < 12 {
        return Err(Error::invalid_image(path, "File too small to be a valid WebP"));
    }

    // Validate RIFF header.
    if !data.starts_with(RIFF) {
        return Err(Error::invalid_image(path, "Invalid RIFF header"));
    }

    // Validate WebP identifier.
    if &data[8..12] != WEBP {
        return Err(Error::invalid_image(path, "Invalid WebP identifier"));
    }

    // Parse chunks.
    let chunks = parse_chunks(data, path)?;

    // Validate we have at least one image chunk.
    let has_image = chunks.iter().any(|c| {
        &c.fourcc == b"VP8 " || &c.fourcc == b"VP8L"
    });

    if !has_image && !chunks.iter().any(|c| &c.fourcc == b"VP8X") {
        return Err(Error::invalid_image(path, "Missing image data chunk"));
    }

    // Build output, filtering out metadata chunks.
    let mut output = Vec::with_capacity(data.len());

    // Write RIFF header (we'll update size later).
    output.extend_from_slice(RIFF);
    output.extend_from_slice(&[0, 0, 0, 0]); // Placeholder for size.
    output.extend_from_slice(WEBP);

    // Write non-metadata chunks.
    for chunk in &chunks {
        if is_metadata_chunk(&chunk.fourcc) {
            continue;
        }

        // Update VP8X flags if needed.
        if &chunk.fourcc == b"VP8X" {
            let updated_data = update_vp8x_flags(chunk.data);
            let updated_chunk = Chunk {
                fourcc: chunk.fourcc,
                data: &updated_data,
            };
            updated_chunk.write_to(&mut output);
        } else {
            chunk.write_to(&mut output);
        }
    }

    // Update file size in RIFF header.
    let file_size = (output.len() - 8) as u32;
    output[4..8].copy_from_slice(&file_size.to_le_bytes());

    Ok(output)
}

/// Create a minimal valid WebP for testing.
#[cfg(test)]
pub fn create_minimal_webp() -> Vec<u8> {
    // This creates a minimal 1x1 lossy WebP.
    vec![
        // RIFF header.
        b'R', b'I', b'F', b'F',
        0x1A, 0x00, 0x00, 0x00, // File size: 26 bytes.
        b'W', b'E', b'B', b'P',
        // VP8 chunk.
        b'V', b'P', b'8', b' ',
        0x0E, 0x00, 0x00, 0x00, // Chunk size: 14 bytes.
        // Minimal VP8 bitstream (1x1 pixel).
        0x30, 0x01, 0x00, 0x9D, 0x01, 0x2A,
        0x01, 0x00, 0x01, 0x00, 0x00, 0x34,
        0x25, 0x9F,
    ]
}

/// Create a WebP with EXIF metadata for testing.
#[cfg(test)]
pub fn create_webp_with_exif() -> Vec<u8> {
    let mut data = Vec::new();

    // RIFF header (size will be updated).
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&[0, 0, 0, 0]); // Placeholder.
    data.extend_from_slice(b"WEBP");

    // VP8X chunk (extended format).
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes()); // Chunk size.
    data.push(0x28); // Flags: EXIF (0x20) + XMP (0x08) present.
    data.extend_from_slice(&[0, 0, 0]); // Reserved.
    data.extend_from_slice(&[0, 0, 0]); // Width - 1.
    data.extend_from_slice(&[0, 0, 0]); // Height - 1.

    // VP8 chunk.
    data.extend_from_slice(b"VP8 ");
    let vp8_data = [
        0x30, 0x01, 0x00, 0x9D, 0x01, 0x2A,
        0x01, 0x00, 0x01, 0x00, 0x00, 0x34,
        0x25, 0x9F,
    ];
    data.extend_from_slice(&(vp8_data.len() as u32).to_le_bytes());
    data.extend_from_slice(&vp8_data);

    // EXIF chunk (should be stripped).
    data.extend_from_slice(b"EXIF");
    let exif_data = b"Exif\x00\x00Test EXIF";
    data.extend_from_slice(&(exif_data.len() as u32).to_le_bytes());
    data.extend_from_slice(exif_data);
    if exif_data.len() % 2 != 0 {
        data.push(0);
    }

    // Update file size.
    let file_size = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_size.to_le_bytes());

    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.webp")
    }

    #[test]
    fn test_strip_minimal_webp() {
        let data = create_minimal_webp();
        let result = strip(&data, &test_path()).unwrap();

        // Should start with RIFF header.
        assert!(result.starts_with(b"RIFF"));
        // Should have WEBP identifier.
        assert_eq!(&result[8..12], b"WEBP");
    }

    #[test]
    fn test_strip_webp_with_exif() {
        let data = create_webp_with_exif();
        let result = strip(&data, &test_path()).unwrap();

        // Should be smaller (EXIF removed).
        assert!(result.len() < data.len());

        // Should not contain EXIF chunk.
        let chunks = parse_chunks(&result, &test_path()).unwrap();
        let has_exif = chunks.iter().any(|c| &c.fourcc == b"EXIF");
        assert!(!has_exif, "EXIF chunk should be removed");

        // VP8X flags should be updated.
        let vp8x = chunks.iter().find(|c| &c.fourcc == b"VP8X");
        if let Some(vp8x) = vp8x {
            let flags = vp8x.data[0];
            assert_eq!(flags & 0x28, 0, "EXIF and XMP flags should be cleared");
        }
    }

    #[test]
    fn test_invalid_too_small() {
        let data = b"RIFF";
        let result = strip(data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_riff_header() {
        let data = [0x00; 20];
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_webp_identifier() {
        let mut data = create_minimal_webp();
        data[8..12].copy_from_slice(b"XXXX");
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_is_metadata_chunk() {
        assert!(is_metadata_chunk(b"EXIF"));
        assert!(is_metadata_chunk(b"XMP "));

        assert!(!is_metadata_chunk(b"VP8 "));
        assert!(!is_metadata_chunk(b"VP8L"));
        assert!(!is_metadata_chunk(b"VP8X"));
        assert!(!is_metadata_chunk(b"ALPH"));
        assert!(!is_metadata_chunk(b"ICCP"));
    }

    #[test]
    fn test_update_vp8x_flags() {
        let original = [0x28, 0x00, 0x00, 0x00]; // EXIF + XMP flags.
        let updated = update_vp8x_flags(&original);
        assert_eq!(updated[0] & 0x28, 0);
    }

    #[test]
    fn test_chunk_padding() {
        let chunk = Chunk {
            fourcc: *b"TEST",
            data: &[0x01, 0x02, 0x03], // Odd length.
        };
        assert_eq!(chunk.padded_size(), 4); // Padded to even.

        let mut output = Vec::new();
        chunk.write_to(&mut output);
        // 4 (fourcc) + 4 (size) + 3 (data) + 1 (padding) = 12.
        assert_eq!(output.len(), 12);
    }
}
