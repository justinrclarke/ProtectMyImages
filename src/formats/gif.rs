//! GIF metadata stripping.
//!
//! GIF files have the following structure:
//! - Header: "GIF87a" or "GIF89a" (6 bytes)
//! - Logical Screen Descriptor (7 bytes)
//! - Global Color Table (optional)
//! - Data blocks (extensions and images)
//! - Trailer (0x3B)
//!
//! Extension blocks we strip:
//! - Comment Extension (0x21 0xFE)
//! - Application Extension (0x21 0xFF) - except NETSCAPE2.0 for animations
//!
//! Blocks we preserve:
//! - Graphics Control Extension (0x21 0xF9) - needed for animation timing
//! - Plain Text Extension (0x21 0x01) - rarely used, but part of image
//! - Image Descriptor and data (0x2C)

use crate::error::{Error, Result};
use std::path::Path;

/// GIF header signatures.
const GIF87A: &[u8; 6] = b"GIF87a";
const GIF89A: &[u8; 6] = b"GIF89a";

/// Block types.
mod blocks {
    pub const EXTENSION: u8 = 0x21;
    pub const IMAGE: u8 = 0x2C;
    pub const TRAILER: u8 = 0x3B;
}

/// Extension types.
mod extensions {
    pub const GRAPHICS_CONTROL: u8 = 0xF9;
    pub const COMMENT: u8 = 0xFE;
    pub const PLAIN_TEXT: u8 = 0x01;
    pub const APPLICATION: u8 = 0xFF;
}

/// NETSCAPE2.0 application identifier (for animation looping).
const NETSCAPE_ID: &[u8] = b"NETSCAPE2.0";

/// Skip sub-blocks until block terminator (0x00).
fn skip_sub_blocks(data: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        if pos >= data.len() {
            return None;
        }
        let block_size = data[pos] as usize;
        pos += 1;
        if block_size == 0 {
            return Some(pos);
        }
        pos += block_size;
        if pos > data.len() {
            return None;
        }
    }
}

/// Read sub-blocks and return them along with the new position.
fn read_sub_blocks(data: &[u8], mut pos: usize) -> Option<(Vec<u8>, usize)> {
    let mut blocks = Vec::new();

    loop {
        if pos >= data.len() {
            return None;
        }
        let block_size = data[pos] as usize;
        blocks.push(data[pos]);
        pos += 1;

        if block_size == 0 {
            return Some((blocks, pos));
        }

        if pos + block_size > data.len() {
            return None;
        }

        blocks.extend_from_slice(&data[pos..pos + block_size]);
        pos += block_size;
    }
}

/// Check if an application extension is NETSCAPE2.0 (animation loop).
fn is_netscape_extension(data: &[u8], pos: usize) -> bool {
    // Application extension starts with 0x0B (11 bytes) identifier.
    if pos + 12 > data.len() {
        return false;
    }
    if data[pos] != 0x0B {
        return false;
    }
    &data[pos + 1..pos + 12] == NETSCAPE_ID
}

/// Strip metadata from GIF data.
pub fn strip(data: &[u8], path: &Path) -> Result<Vec<u8>> {
    // Validate minimum size.
    if data.len() < 13 {
        return Err(Error::invalid_image(
            path,
            "File too small to be a valid GIF",
        ));
    }

    // Validate header.
    if !data.starts_with(GIF87A) && !data.starts_with(GIF89A) {
        return Err(Error::invalid_image(path, "Invalid GIF header"));
    }

    let mut output = Vec::with_capacity(data.len());

    // Copy header (6 bytes).
    output.extend_from_slice(&data[0..6]);
    let mut pos = 6;

    // Copy Logical Screen Descriptor (7 bytes).
    if pos + 7 > data.len() {
        return Err(Error::invalid_image(
            path,
            "Truncated Logical Screen Descriptor",
        ));
    }
    output.extend_from_slice(&data[pos..pos + 7]);

    // Check for Global Color Table.
    let packed = data[pos + 4];
    let has_gct = (packed & 0x80) != 0;
    let gct_size = if has_gct {
        3 * (1 << ((packed & 0x07) + 1))
    } else {
        0
    };
    pos += 7;

    // Copy Global Color Table if present.
    if has_gct {
        if pos + gct_size > data.len() {
            return Err(Error::invalid_image(path, "Truncated Global Color Table"));
        }
        output.extend_from_slice(&data[pos..pos + gct_size]);
        pos += gct_size;
    }

    // Process blocks.
    while pos < data.len() {
        let block_type = data[pos];

        match block_type {
            blocks::EXTENSION => {
                if pos + 2 > data.len() {
                    return Err(Error::invalid_image(path, "Truncated extension block"));
                }
                let ext_type = data[pos + 1];

                match ext_type {
                    extensions::COMMENT => {
                        // Skip comment extension.
                        pos += 2;
                        pos = skip_sub_blocks(data, pos).ok_or_else(|| {
                            Error::invalid_image(path, "Truncated comment extension")
                        })?;
                    }
                    extensions::APPLICATION => {
                        // Check if it's NETSCAPE2.0 (animation).
                        if is_netscape_extension(data, pos + 2) {
                            // Keep NETSCAPE extension for animations.
                            output.extend_from_slice(&data[pos..pos + 2]);
                            pos += 2;
                            let (blocks, new_pos) =
                                read_sub_blocks(data, pos).ok_or_else(|| {
                                    Error::invalid_image(path, "Truncated application extension")
                                })?;
                            output.extend_from_slice(&blocks);
                            pos = new_pos;
                        } else {
                            // Skip other application extensions (XMP, etc.).
                            pos += 2;
                            pos = skip_sub_blocks(data, pos).ok_or_else(|| {
                                Error::invalid_image(path, "Truncated application extension")
                            })?;
                        }
                    }
                    extensions::GRAPHICS_CONTROL => {
                        // Keep graphics control extension.
                        output.extend_from_slice(&data[pos..pos + 2]);
                        pos += 2;
                        let (blocks, new_pos) = read_sub_blocks(data, pos).ok_or_else(|| {
                            Error::invalid_image(path, "Truncated graphics control extension")
                        })?;
                        output.extend_from_slice(&blocks);
                        pos = new_pos;
                    }
                    extensions::PLAIN_TEXT => {
                        // Keep plain text extension.
                        output.extend_from_slice(&data[pos..pos + 2]);
                        pos += 2;
                        let (blocks, new_pos) = read_sub_blocks(data, pos).ok_or_else(|| {
                            Error::invalid_image(path, "Truncated plain text extension")
                        })?;
                        output.extend_from_slice(&blocks);
                        pos = new_pos;
                    }
                    _ => {
                        // Unknown extension - skip it.
                        pos += 2;
                        pos = skip_sub_blocks(data, pos).ok_or_else(|| {
                            Error::invalid_image(path, "Truncated unknown extension")
                        })?;
                    }
                }
            }
            blocks::IMAGE => {
                // Copy image descriptor (10 bytes).
                if pos + 10 > data.len() {
                    return Err(Error::invalid_image(path, "Truncated image descriptor"));
                }
                output.extend_from_slice(&data[pos..pos + 10]);

                // Check for Local Color Table.
                let img_packed = data[pos + 9];
                let has_lct = (img_packed & 0x80) != 0;
                let lct_size = if has_lct {
                    3 * (1 << ((img_packed & 0x07) + 1))
                } else {
                    0
                };
                pos += 10;

                // Copy Local Color Table if present.
                if has_lct {
                    if pos + lct_size > data.len() {
                        return Err(Error::invalid_image(path, "Truncated Local Color Table"));
                    }
                    output.extend_from_slice(&data[pos..pos + lct_size]);
                    pos += lct_size;
                }

                // Copy LZW minimum code size.
                if pos >= data.len() {
                    return Err(Error::invalid_image(path, "Missing LZW minimum code size"));
                }
                output.push(data[pos]);
                pos += 1;

                // Copy image data sub-blocks.
                let (blocks, new_pos) = read_sub_blocks(data, pos)
                    .ok_or_else(|| Error::invalid_image(path, "Truncated image data"))?;
                output.extend_from_slice(&blocks);
                pos = new_pos;
            }
            blocks::TRAILER => {
                // End of GIF.
                output.push(blocks::TRAILER);
                break;
            }
            _ => {
                // Unknown block type - skip byte and continue.
                pos += 1;
            }
        }
    }

    // Ensure trailer is present.
    if !output.ends_with(&[blocks::TRAILER]) {
        output.push(blocks::TRAILER);
    }

    Ok(output)
}

/// Create a minimal valid GIF for testing.
#[cfg(test)]
pub fn create_minimal_gif() -> Vec<u8> {
    vec![
        // Header.
        b'G', b'I', b'F', b'8', b'9', b'a', // Logical Screen Descriptor.
        0x01, 0x00, // Width: 1.
        0x01, 0x00, // Height: 1.
        0x00, // Packed: no GCT.
        0x00, // Background color index.
        0x00, // Pixel aspect ratio.
        // Image Descriptor.
        0x2C, // Image separator.
        0x00, 0x00, // Left.
        0x00, 0x00, // Top.
        0x01, 0x00, // Width: 1.
        0x01, 0x00, // Height: 1.
        0x00, // Packed: no LCT.
        // Image Data.
        0x02, // LZW minimum code size.
        0x02, // Block size: 2.
        0x44, 0x01, // Compressed data.
        0x00, // Block terminator.
        // Trailer.
        0x3B,
    ]
}

/// Create a GIF with comment for testing.
#[cfg(test)]
pub fn create_gif_with_comment() -> Vec<u8> {
    vec![
        // Header.
        b'G', b'I', b'F', b'8', b'9', b'a', // Logical Screen Descriptor.
        0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        // Comment Extension (should be stripped).
        0x21, 0xFE, // Extension introducer + comment label.
        0x0D, // Block size: 13.
        b'T', b'e', b's', b't', b' ', b'c', b'o', b'm', b'm', b'e', b'n', b't', b'!',
        0x00, // Block terminator.
        // Image Descriptor.
        0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, // Image Data.
        0x02, 0x02, 0x44, 0x01, 0x00, // Trailer.
        0x3B,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.gif")
    }

    #[test]
    fn test_strip_minimal_gif() {
        let data = create_minimal_gif();
        let result = strip(&data, &test_path()).unwrap();

        // Should start with GIF header.
        assert!(result.starts_with(b"GIF89a") || result.starts_with(b"GIF87a"));
        // Should end with trailer.
        assert!(result.ends_with(&[0x3B]));
    }

    #[test]
    fn test_strip_gif_with_comment() {
        let data = create_gif_with_comment();
        let result = strip(&data, &test_path()).unwrap();

        // Should be smaller (comment removed).
        assert!(result.len() < data.len());

        // Should not contain comment extension marker.
        let has_comment = result.windows(2).any(|w| w == [0x21, 0xFE]);
        assert!(!has_comment, "Comment extension should be removed");

        // Should still have image data.
        let has_image = result.windows(1).any(|w| w == [0x2C]);
        assert!(has_image, "Image descriptor should be preserved");
    }

    #[test]
    fn test_invalid_too_small() {
        let data = b"GIF89a";
        let result = strip(data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_header() {
        let data = [0x00; 20];
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_is_netscape_extension() {
        let data = [
            0x0B, // Block size.
            b'N', b'E', b'T', b'S', b'C', b'A', b'P', b'E', b'2', b'.', b'0',
        ];
        assert!(is_netscape_extension(&data, 0));

        let not_netscape = [
            0x0B, b'X', b'M', b'P', b' ', b'D', b'a', b't', b'a', b'X', b'M', b'P',
        ];
        assert!(!is_netscape_extension(&not_netscape, 0));
    }

    #[test]
    fn test_skip_sub_blocks() {
        let data = [
            0x03, b'a', b'b', b'c', // Block of 3 bytes.
            0x02, b'd', b'e', // Block of 2 bytes.
            0x00, // Terminator.
        ];
        let end = skip_sub_blocks(&data, 0).unwrap();
        assert_eq!(end, data.len());
    }

    #[test]
    fn test_read_sub_blocks() {
        let data = [0x03, b'a', b'b', b'c', 0x00];
        let (blocks, end) = read_sub_blocks(&data, 0).unwrap();
        assert_eq!(blocks, vec![0x03, b'a', b'b', b'c', 0x00]);
        assert_eq!(end, data.len());
    }
}
