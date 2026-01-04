//! Image format detection and metadata stripping.
//!
//! This module provides format detection via magic bytes and a trait
//! for stripping metadata from various image formats.

pub mod gif;
pub mod jpeg;
pub mod png;
pub mod tiff;
pub mod webp;

use crate::error::{Error, Result};
use std::path::Path;

/// Supported image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
    Tiff,
}

impl ImageFormat {
    /// Get the format name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "JPEG",
            ImageFormat::Png => "PNG",
            ImageFormat::Gif => "GIF",
            ImageFormat::WebP => "WebP",
            ImageFormat::Tiff => "TIFF",
        }
    }

    /// Get common file extensions for this format.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            ImageFormat::Jpeg => &["jpg", "jpeg", "jpe", "jfif"],
            ImageFormat::Png => &["png"],
            ImageFormat::Gif => &["gif"],
            ImageFormat::WebP => &["webp"],
            ImageFormat::Tiff => &["tif", "tiff"],
        }
    }
}

/// Magic bytes for format detection.
mod magic {
    /// JPEG magic bytes: FF D8 FF
    pub const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF];

    /// PNG magic bytes: 89 50 4E 47 0D 0A 1A 0A
    pub const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    /// GIF magic bytes: GIF87a or GIF89a
    pub const GIF87A: &[u8] = b"GIF87a";
    pub const GIF89A: &[u8] = b"GIF89a";

    /// WebP magic bytes: RIFF....WEBP
    pub const RIFF: &[u8] = b"RIFF";
    pub const WEBP: &[u8] = b"WEBP";

    /// TIFF magic bytes (little-endian): II*\0
    pub const TIFF_LE: &[u8] = &[0x49, 0x49, 0x2A, 0x00];

    /// TIFF magic bytes (big-endian): MM\0*
    pub const TIFF_BE: &[u8] = &[0x4D, 0x4D, 0x00, 0x2A];
}

/// Detect image format from magic bytes.
pub fn detect_format(data: &[u8]) -> Option<ImageFormat> {
    if data.len() < 12 {
        return None;
    }

    // Check JPEG.
    if data.starts_with(magic::JPEG) {
        return Some(ImageFormat::Jpeg);
    }

    // Check PNG.
    if data.starts_with(magic::PNG) {
        return Some(ImageFormat::Png);
    }

    // Check GIF.
    if data.starts_with(magic::GIF87A) || data.starts_with(magic::GIF89A) {
        return Some(ImageFormat::Gif);
    }

    // Check WebP (RIFF container with WEBP identifier).
    if data.starts_with(magic::RIFF) && &data[8..12] == magic::WEBP {
        return Some(ImageFormat::WebP);
    }

    // Check TIFF.
    if data.starts_with(magic::TIFF_LE) || data.starts_with(magic::TIFF_BE) {
        return Some(ImageFormat::Tiff);
    }

    None
}

/// Detect format from file extension.
pub fn detect_format_from_extension(path: &Path) -> Option<ImageFormat> {
    let ext = path.extension()?.to_str()?.to_lowercase();

    for format in [
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::WebP,
        ImageFormat::Tiff,
    ] {
        if format.extensions().contains(&ext.as_str()) {
            return Some(format);
        }
    }

    None
}

/// Result of stripping metadata from an image.
#[derive(Debug)]
pub struct StripResult {
    /// The cleaned image data.
    pub data: Vec<u8>,
    /// Number of bytes of metadata removed.
    pub bytes_removed: u64,
}

impl StripResult {
    /// Create a new strip result.
    pub fn new(data: Vec<u8>, bytes_removed: u64) -> Self {
        Self {
            data,
            bytes_removed,
        }
    }
}

/// Strip metadata from image data.
///
/// Detects the image format and strips all metadata while preserving
/// the image data.
pub fn strip_metadata(data: &[u8], path: &Path) -> Result<StripResult> {
    let format = detect_format(data).ok_or_else(|| {
        let ext_format = detect_format_from_extension(path);
        Error::unsupported_format(path, ext_format.map(|f| f.name()))
    })?;

    let original_size = data.len() as u64;

    let result = match format {
        ImageFormat::Jpeg => jpeg::strip(data, path)?,
        ImageFormat::Png => png::strip(data, path)?,
        ImageFormat::Gif => gif::strip(data, path)?,
        ImageFormat::WebP => webp::strip(data, path)?,
        ImageFormat::Tiff => tiff::strip(data, path)?,
    };

    let bytes_removed = original_size.saturating_sub(result.len() as u64);

    Ok(StripResult::new(result, bytes_removed))
}

/// Check if a file appears to be a supported image format.
pub fn is_supported_format(data: &[u8]) -> bool {
    detect_format(data).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_jpeg() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01];
        assert_eq!(detect_format(&data), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn test_detect_png() {
        let data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D];
        assert_eq!(detect_format(&data), Some(ImageFormat::Png));
    }

    #[test]
    fn test_detect_gif87a() {
        let data = b"GIF87atest12";
        assert_eq!(detect_format(data), Some(ImageFormat::Gif));
    }

    #[test]
    fn test_detect_gif89a() {
        let data = b"GIF89atest12";
        assert_eq!(detect_format(data), Some(ImageFormat::Gif));
    }

    #[test]
    fn test_detect_webp() {
        let mut data = vec![0u8; 12];
        data[..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        assert_eq!(detect_format(&data), Some(ImageFormat::WebP));
    }

    #[test]
    fn test_detect_tiff_le() {
        let data = [0x49, 0x49, 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format(&data), Some(ImageFormat::Tiff));
    }

    #[test]
    fn test_detect_tiff_be() {
        let data = [0x4D, 0x4D, 0x00, 0x2A, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format(&data), Some(ImageFormat::Tiff));
    }

    #[test]
    fn test_detect_unknown() {
        let data = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(detect_format(&data), None);
    }

    #[test]
    fn test_detect_too_short() {
        let data = [0xFF, 0xD8];
        assert_eq!(detect_format(&data), None);
    }

    #[test]
    fn test_detect_from_extension_jpeg() {
        assert_eq!(
            detect_format_from_extension(Path::new("photo.jpg")),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(
            detect_format_from_extension(Path::new("photo.JPEG")),
            Some(ImageFormat::Jpeg)
        );
    }

    #[test]
    fn test_detect_from_extension_png() {
        assert_eq!(
            detect_format_from_extension(Path::new("image.png")),
            Some(ImageFormat::Png)
        );
    }

    #[test]
    fn test_detect_from_extension_unknown() {
        assert_eq!(detect_format_from_extension(Path::new("file.bmp")), None);
    }

    #[test]
    fn test_format_name() {
        assert_eq!(ImageFormat::Jpeg.name(), "JPEG");
        assert_eq!(ImageFormat::Png.name(), "PNG");
        assert_eq!(ImageFormat::Gif.name(), "GIF");
        assert_eq!(ImageFormat::WebP.name(), "WebP");
        assert_eq!(ImageFormat::Tiff.name(), "TIFF");
    }

    #[test]
    fn test_format_extensions() {
        assert!(ImageFormat::Jpeg.extensions().contains(&"jpg"));
        assert!(ImageFormat::Jpeg.extensions().contains(&"jpeg"));
        assert!(ImageFormat::Png.extensions().contains(&"png"));
    }

    #[test]
    fn test_is_supported_format() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01];
        assert!(is_supported_format(&jpeg));

        let unknown = [0x00; 12];
        assert!(!is_supported_format(&unknown));
    }
}
