//! JPEG metadata stripping.
//!
//! JPEG files consist of segments marked by FF xx markers. This module
//! strips metadata segments while preserving essential image data.
//!
//! Metadata segments stripped:
//! - APP1 (FF E1): EXIF, XMP
//! - APP2-APP15 (FF E2 - FF EF): Various metadata
//! - APP13 (FF ED): IPTC/Photoshop
//! - COM (FF FE): Comments
//!
//! Segments preserved:
//! - SOI (FF D8): Start of image
//! - APP0 (FF E0): JFIF marker
//! - DQT (FF DB): Quantization tables
//! - DHT (FF C4): Huffman tables
//! - SOF (FF C0-CF): Start of frame
//! - SOS (FF DA): Start of scan (and all image data)
//! - EOI (FF D9): End of image

use crate::error::{Error, Result};
use std::path::Path;

/// JPEG marker bytes.
mod markers {
    pub const MARKER_PREFIX: u8 = 0xFF;

    // Start/End markers.
    pub const SOI: u8 = 0xD8; // Start of image.
    pub const EOI: u8 = 0xD9; // End of image.

    // Frame markers.
    pub const SOF0: u8 = 0xC0; // Baseline DCT.
    pub const SOF1: u8 = 0xC1; // Extended sequential DCT.
    pub const SOF2: u8 = 0xC2; // Progressive DCT.
    pub const SOF3: u8 = 0xC3; // Lossless.
    pub const DHT: u8 = 0xC4; // Huffman table.
    pub const SOF5: u8 = 0xC5;
    pub const SOF6: u8 = 0xC6;
    pub const SOF7: u8 = 0xC7;
    pub const SOF9: u8 = 0xC9;
    pub const SOF10: u8 = 0xCA;
    pub const SOF11: u8 = 0xCB;
    pub const DAC: u8 = 0xCC; // Arithmetic coding.
    pub const SOF13: u8 = 0xCD;
    pub const SOF14: u8 = 0xCE;
    pub const SOF15: u8 = 0xCF;

    // Restart markers.
    pub const RST0: u8 = 0xD0;
    pub const RST7: u8 = 0xD7;

    // Other markers.
    pub const DQT: u8 = 0xDB; // Quantization table.
    pub const DRI: u8 = 0xDD; // Restart interval.
    pub const SOS: u8 = 0xDA; // Start of scan.

    // Application markers (metadata).
    pub const APP0: u8 = 0xE0; // JFIF.
    pub const APP1: u8 = 0xE1; // EXIF, XMP.
    pub const APP2: u8 = 0xE2; // ICC profile, FlashPix.
    pub const APP13: u8 = 0xED; // IPTC/Photoshop.
    pub const APP14: u8 = 0xEE; // Adobe.
    pub const APP15: u8 = 0xEF;

    // Comment marker.
    pub const COM: u8 = 0xFE;
}

/// Check if a marker is a metadata marker that should be stripped.
fn is_metadata_marker(marker: u8) -> bool {
    match marker {
        // APP1 (EXIF, XMP) - always strip.
        markers::APP1 => true,
        // APP2-APP12 - strip (ICC profile, FlashPix, etc.).
        0xE2..=0xEC => true,
        // APP13 (IPTC) - strip.
        markers::APP13 => true,
        // APP15 - strip.
        markers::APP15 => true,
        // COM (comments) - strip.
        markers::COM => true,
        _ => false,
    }
}

/// Check if a marker is a standalone marker (no length field).
fn is_standalone_marker(marker: u8) -> bool {
    match marker {
        markers::SOI | markers::EOI => true,
        m if (markers::RST0..=markers::RST7).contains(&m) => true,
        0x00 | 0xFF => true, // Padding/stuffing bytes.
        _ => false,
    }
}

/// Strip metadata from JPEG data.
pub fn strip(data: &[u8], path: &Path) -> Result<Vec<u8>> {
    // Validate minimum size and SOI marker.
    if data.len() < 4 {
        return Err(Error::invalid_image(path, "File too small to be a valid JPEG"));
    }

    if data[0] != markers::MARKER_PREFIX || data[1] != markers::SOI {
        return Err(Error::invalid_image(path, "Missing JPEG SOI marker"));
    }

    let mut output = Vec::with_capacity(data.len());

    // Write SOI marker.
    output.extend_from_slice(&[markers::MARKER_PREFIX, markers::SOI]);
    let mut pos = 2;

    // Process segments.
    while pos < data.len() {
        // Find next marker.
        if data[pos] != markers::MARKER_PREFIX {
            return Err(Error::invalid_image(
                path,
                format!("Expected marker at position {}", pos),
            ));
        }

        // Skip padding bytes (multiple 0xFF).
        while pos < data.len() && data[pos] == markers::MARKER_PREFIX {
            pos += 1;
        }

        if pos >= data.len() {
            break;
        }

        let marker = data[pos];
        pos += 1;

        // Handle end of image.
        if marker == markers::EOI {
            output.extend_from_slice(&[markers::MARKER_PREFIX, markers::EOI]);
            break;
        }

        // Handle start of scan - copy rest of file.
        if marker == markers::SOS {
            // Copy SOS marker and everything until EOI.
            output.extend_from_slice(&[markers::MARKER_PREFIX, markers::SOS]);

            // Read SOS header length.
            if pos + 2 > data.len() {
                return Err(Error::invalid_image(path, "Truncated SOS segment"));
            }

            let length = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
            if pos + length > data.len() {
                return Err(Error::invalid_image(path, "SOS segment extends beyond file"));
            }

            // Copy SOS header.
            output.extend_from_slice(&data[pos..pos + length]);
            pos += length;

            // Copy entropy-coded data until EOI.
            while pos < data.len() {
                if data[pos] == markers::MARKER_PREFIX && pos + 1 < data.len() {
                    let next = data[pos + 1];

                    // 0xFF00 is an escaped 0xFF in the data stream.
                    if next == 0x00 {
                        output.extend_from_slice(&[markers::MARKER_PREFIX, 0x00]);
                        pos += 2;
                        continue;
                    }

                    // Restart markers are embedded in the data stream.
                    if (markers::RST0..=markers::RST7).contains(&next) {
                        output.extend_from_slice(&[markers::MARKER_PREFIX, next]);
                        pos += 2;
                        continue;
                    }

                    // EOI marker.
                    if next == markers::EOI {
                        output.extend_from_slice(&[markers::MARKER_PREFIX, markers::EOI]);
                        pos += 2;
                        break;
                    }

                    // Another marker - might be multiple scans.
                    break;
                }

                output.push(data[pos]);
                pos += 1;
            }

            continue;
        }

        // Handle standalone markers.
        if is_standalone_marker(marker) {
            // Skip padding, don't write anything.
            continue;
        }

        // Read segment length (includes length field itself).
        if pos + 2 > data.len() {
            return Err(Error::invalid_image(path, "Truncated segment header"));
        }

        let length = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
        if length < 2 {
            return Err(Error::invalid_image(path, "Invalid segment length"));
        }

        if pos + length > data.len() {
            return Err(Error::invalid_image(path, "Segment extends beyond file"));
        }

        // Copy or skip segment based on marker type.
        if is_metadata_marker(marker) {
            // Skip metadata segment.
            pos += length;
        } else {
            // Copy segment.
            output.extend_from_slice(&[markers::MARKER_PREFIX, marker]);
            output.extend_from_slice(&data[pos..pos + length]);
            pos += length;
        }
    }

    Ok(output)
}

/// Create a minimal valid JPEG for testing.
#[cfg(test)]
pub fn create_minimal_jpeg() -> Vec<u8> {
    // This creates a minimal 1x1 pixel JPEG.
    vec![
        // SOI.
        0xFF, 0xD8,
        // APP0 (JFIF).
        0xFF, 0xE0, 0x00, 0x10,
        b'J', b'F', b'I', b'F', 0x00,
        0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
        // DQT.
        0xFF, 0xDB, 0x00, 0x43, 0x00,
        0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07,
        0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14,
        0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12, 0x13,
        0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A,
        0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22,
        0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C,
        0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39,
        0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32,
        // SOF0.
        0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00,
        // DHT.
        0xFF, 0xC4, 0x00, 0x1F, 0x00,
        0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0A, 0x0B,
        // DHT.
        0xFF, 0xC4, 0x00, 0xB5, 0x10,
        0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03,
        0x05, 0x05, 0x04, 0x04, 0x00, 0x00, 0x01, 0x7D,
        0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12,
        0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
        0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08,
        0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
        0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16,
        0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
        0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
        0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
        0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
        0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
        0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
        0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
        0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
        0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
        0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6,
        0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
        0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4,
        0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
        0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA,
        0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
        0xF9, 0xFA,
        // SOS.
        0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00,
        // Minimal scan data.
        0xFB, 0xD3, 0x28, 0xA2, 0x80, 0x0F,
        // EOI.
        0xFF, 0xD9,
    ]
}

/// Create a JPEG with EXIF metadata for testing.
#[cfg(test)]
pub fn create_jpeg_with_exif() -> Vec<u8> {
    let mut data = Vec::new();

    // SOI.
    data.extend_from_slice(&[0xFF, 0xD8]);

    // APP0 (JFIF).
    data.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
    data.extend_from_slice(b"JFIF\x00");
    data.extend_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);

    // APP1 (EXIF) - this should be stripped.
    let exif_data = b"Exif\x00\x00Test EXIF data that should be removed";
    let exif_len = (exif_data.len() + 2) as u16;
    data.extend_from_slice(&[0xFF, 0xE1]);
    data.extend_from_slice(&exif_len.to_be_bytes());
    data.extend_from_slice(exif_data);

    // COM (comment) - this should be stripped.
    let comment = b"Test comment to remove";
    let comment_len = (comment.len() + 2) as u16;
    data.extend_from_slice(&[0xFF, 0xFE]);
    data.extend_from_slice(&comment_len.to_be_bytes());
    data.extend_from_slice(comment);

    // Append the rest of a minimal JPEG (from DQT onwards).
    let minimal = create_minimal_jpeg();
    let dqt_pos = minimal.windows(2).position(|w| w == [0xFF, 0xDB]).unwrap();
    data.extend_from_slice(&minimal[dqt_pos..]);

    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.jpg")
    }

    #[test]
    fn test_strip_minimal_jpeg() {
        let data = create_minimal_jpeg();
        let result = strip(&data, &test_path()).unwrap();

        // Should still be valid.
        assert!(result.starts_with(&[0xFF, 0xD8]));
        assert!(result.ends_with(&[0xFF, 0xD9]));
    }

    #[test]
    fn test_strip_jpeg_with_exif() {
        let data = create_jpeg_with_exif();
        let result = strip(&data, &test_path()).unwrap();

        // Should be smaller (metadata removed).
        assert!(result.len() < data.len());

        // Should not contain EXIF marker.
        let has_exif = result.windows(2).any(|w| w == [0xFF, 0xE1]);
        assert!(!has_exif, "EXIF marker should be removed");

        // Should not contain COM marker.
        let has_com = result.windows(2).any(|w| w == [0xFF, 0xFE]);
        assert!(!has_com, "COM marker should be removed");

        // Should still have APP0 (JFIF).
        let has_app0 = result.windows(2).any(|w| w == [0xFF, 0xE0]);
        assert!(has_app0, "APP0 marker should be preserved");
    }

    #[test]
    fn test_invalid_too_small() {
        let data = [0xFF, 0xD8];
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_no_soi() {
        let data = [0x00, 0x00, 0x00, 0x00, 0x00];
        let result = strip(&data, &test_path());
        assert!(result.is_err());
    }

    #[test]
    fn test_is_metadata_marker() {
        assert!(is_metadata_marker(markers::APP1)); // EXIF.
        assert!(is_metadata_marker(markers::APP13)); // IPTC.
        assert!(is_metadata_marker(markers::COM)); // Comment.
        assert!(is_metadata_marker(0xE2)); // APP2.

        assert!(!is_metadata_marker(markers::APP0)); // JFIF - keep.
        assert!(!is_metadata_marker(markers::DQT)); // Essential.
        assert!(!is_metadata_marker(markers::SOF0)); // Essential.
        assert!(!is_metadata_marker(markers::APP14)); // Adobe - keep for color.
    }

    #[test]
    fn test_is_standalone_marker() {
        assert!(is_standalone_marker(markers::SOI));
        assert!(is_standalone_marker(markers::EOI));
        assert!(is_standalone_marker(markers::RST0));
        assert!(is_standalone_marker(markers::RST7));

        assert!(!is_standalone_marker(markers::APP0));
        assert!(!is_standalone_marker(markers::DQT));
    }
}
