//! Integration tests for PMI.

use pmi::cli::Config;
use pmi::formats::{detect_format, strip_metadata, ImageFormat};
use pmi::processor::Processor;
use std::fs;
use std::path::PathBuf;

mod helpers {
    //! Test helpers for creating minimal image files.

    /// Create a minimal JPEG for testing.
    pub fn create_minimal_jpeg() -> Vec<u8> {
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
            // DHT (simplified).
            0xFF, 0xC4, 0x00, 0x14, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00,
            // SOS.
            0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00,
            // Minimal scan data.
            0x7F,
            // EOI.
            0xFF, 0xD9,
        ]
    }

    /// Create a JPEG with EXIF metadata.
    pub fn create_jpeg_with_exif() -> Vec<u8> {
        let mut data = Vec::new();

        // SOI.
        data.extend_from_slice(&[0xFF, 0xD8]);

        // APP0 (JFIF).
        data.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        data.extend_from_slice(b"JFIF\x00");
        data.extend_from_slice(&[0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00]);

        // APP1 (EXIF) - this should be stripped.
        let exif_data = b"Exif\x00\x00Test EXIF data that should be removed for privacy";
        let exif_len = (exif_data.len() + 2) as u16;
        data.extend_from_slice(&[0xFF, 0xE1]);
        data.extend_from_slice(&exif_len.to_be_bytes());
        data.extend_from_slice(exif_data);

        // COM (comment) - this should be stripped.
        let comment = b"Test comment with personal info";
        let comment_len = (comment.len() + 2) as u16;
        data.extend_from_slice(&[0xFF, 0xFE]);
        data.extend_from_slice(&comment_len.to_be_bytes());
        data.extend_from_slice(comment);

        // Append the rest from minimal JPEG.
        let minimal = create_minimal_jpeg();
        // Skip SOI and APP0.
        let dqt_pos = minimal.windows(2).position(|w| w == [0xFF, 0xDB]).unwrap();
        data.extend_from_slice(&minimal[dqt_pos..]);

        data
    }

    /// Create a minimal PNG.
    pub fn create_minimal_png() -> Vec<u8> {
        // PNG signature.
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        // IHDR chunk.
        let ihdr_data = [
            0x00, 0x00, 0x00, 0x01, // Width: 1
            0x00, 0x00, 0x00, 0x01, // Height: 1
            0x08, 0x00, 0x00, 0x00, 0x00, // 8-bit grayscale
        ];
        write_png_chunk(&mut data, b"IHDR", &ihdr_data);

        // IDAT chunk (minimal).
        let idat_data = [0x78, 0x9C, 0x62, 0x60, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01];
        write_png_chunk(&mut data, b"IDAT", &idat_data);

        // IEND chunk.
        write_png_chunk(&mut data, b"IEND", &[]);

        data
    }

    /// Write a PNG chunk.
    fn write_png_chunk(output: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(chunk_type);
        output.extend_from_slice(data);
        // CRC (simplified - just write a placeholder).
        let crc = png_crc32(chunk_type, data);
        output.extend_from_slice(&crc.to_be_bytes());
    }

    /// Calculate CRC32 for PNG chunk.
    fn png_crc32(chunk_type: &[u8], data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;
        for &byte in chunk_type.iter().chain(data.iter()) {
            crc ^= byte as u32;
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
            }
        }
        crc ^ 0xFFFFFFFF
    }
}

#[test]
fn test_format_detection_jpeg() {
    let data = helpers::create_minimal_jpeg();
    assert_eq!(detect_format(&data), Some(ImageFormat::Jpeg));
}

#[test]
fn test_format_detection_png() {
    let data = helpers::create_minimal_png();
    assert_eq!(detect_format(&data), Some(ImageFormat::Png));
}

#[test]
fn test_strip_jpeg_removes_exif() {
    let data = helpers::create_jpeg_with_exif();
    let path = PathBuf::from("test.jpg");

    let result = strip_metadata(&data, &path).unwrap();

    // Should be smaller.
    assert!(result.data.len() < data.len());
    assert!(result.bytes_removed > 0);

    // Should not contain EXIF marker.
    let has_exif = result.data.windows(2).any(|w| w == [0xFF, 0xE1]);
    assert!(!has_exif, "EXIF marker should be removed");

    // Should not contain COM marker.
    let has_com = result.data.windows(2).any(|w| w == [0xFF, 0xFE]);
    assert!(!has_com, "COM marker should be removed");
}

#[test]
fn test_cli_parse_basic() {
    let config = Config::parse(["pmi", "image.jpg"]).unwrap();
    assert_eq!(config.paths.len(), 1);
    assert!(!config.verbose);
    assert!(!config.quiet);
}

#[test]
fn test_cli_parse_flags() {
    let config = Config::parse(["pmi", "-i", "-v", "image.jpg"]).unwrap();
    assert!(config.in_place);
    assert!(config.verbose);
}

#[test]
fn test_cli_parse_output_dir() {
    let config = Config::parse(["pmi", "-o", "/output", "image.jpg"]).unwrap();
    assert_eq!(config.output_dir, Some(PathBuf::from("/output")));
}

#[test]
fn test_cli_help_flag() {
    let config = Config::parse(["pmi", "--help"]).unwrap();
    assert!(config.help);
}

#[test]
fn test_cli_version_flag() {
    let config = Config::parse(["pmi", "-V"]).unwrap();
    assert!(config.version);
}

#[test]
fn test_cli_missing_paths() {
    let result = Config::parse(["pmi"]);
    assert!(result.is_err());
}

#[test]
fn test_cli_quiet_verbose_conflict() {
    let result = Config::parse(["pmi", "-q", "-v", "image.jpg"]);
    assert!(result.is_err());
}

#[test]
fn test_processor_dry_run() {
    // Create a temporary test file.
    let temp_dir = std::env::temp_dir().join("pmi_test");
    let _ = fs::create_dir_all(&temp_dir);
    let test_file = temp_dir.join("test_dry_run.jpg");
    fs::write(&test_file, helpers::create_jpeg_with_exif()).unwrap();

    // Run processor in dry-run mode.
    let config = Config {
        paths: vec![test_file.clone()],
        output_dir: None,
        recursive: false,
        force: false,
        in_place: false,
        verbose: false,
        quiet: true,
        dry_run: true,
        help: false,
        version: false,
    };

    let mut processor = Processor::new(config);
    let stats = processor.run().unwrap();

    assert_eq!(stats.processed, 1);
    assert!(stats.metadata_removed > 0);

    // Original file should be unchanged.
    let original_data = fs::read(&test_file).unwrap();
    let has_exif = original_data.windows(2).any(|w| w == [0xFF, 0xE1]);
    assert!(has_exif, "Original file should still have EXIF in dry-run mode");

    // Cleanup.
    let _ = fs::remove_file(&test_file);
}

#[test]
fn test_processor_creates_clean_file() {
    // Create a temporary test file.
    let temp_dir = std::env::temp_dir().join("pmi_test");
    let _ = fs::create_dir_all(&temp_dir);
    let test_file = temp_dir.join("test_clean.jpg");
    let clean_file = temp_dir.join("test_clean_clean.jpg");

    // Remove any existing clean file.
    let _ = fs::remove_file(&clean_file);

    fs::write(&test_file, helpers::create_jpeg_with_exif()).unwrap();

    // Run processor.
    let config = Config {
        paths: vec![test_file.clone()],
        output_dir: None,
        recursive: false,
        force: false,
        in_place: false,
        verbose: false,
        quiet: true,
        dry_run: false,
        help: false,
        version: false,
    };

    let mut processor = Processor::new(config);
    let stats = processor.run().unwrap();

    assert_eq!(stats.processed, 1);

    // Clean file should exist.
    assert!(clean_file.exists(), "Clean file should be created");

    // Clean file should not have EXIF.
    let clean_data = fs::read(&clean_file).unwrap();
    let has_exif = clean_data.windows(2).any(|w| w == [0xFF, 0xE1]);
    assert!(!has_exif, "Clean file should not have EXIF");

    // Cleanup.
    let _ = fs::remove_file(&test_file);
    let _ = fs::remove_file(&clean_file);
}
