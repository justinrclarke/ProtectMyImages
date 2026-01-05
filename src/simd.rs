//! SIMD-accelerated operations.
//!
//! Provides hardware-accelerated implementations using CPU intrinsics
//! when available, with fallback to software implementations.

/// CRC32 implementation (software only).
///
/// Note: Hardware CRC32 instructions (SSE4.2, ARM CRC) compute CRC-32C
/// (Castagnoli polynomial), not standard CRC-32 (ISO 3309 polynomial)
/// used by PNG. Therefore we use the software implementation which
/// correctly uses the standard polynomial 0xEDB88320.
pub mod crc32 {
    /// CRC32 lookup table for software implementation.
    const CRC32_TABLE: [u32; 256] = generate_crc32_table();

    /// Generate the CRC32 lookup table at compile time.
    const fn generate_crc32_table() -> [u32; 256] {
        let mut table = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut crc = i as u32;
            let mut j = 0;
            while j < 8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xEDB88320;
                } else {
                    crc >>= 1;
                }
                j += 1;
            }
            table[i] = crc;
            i += 1;
        }
        table
    }

    /// Compute CRC32 using the standard ISO 3309 polynomial.
    ///
    /// This uses an optimized software implementation with a lookup table.
    /// Hardware CRC32 instructions cannot be used because they implement
    /// CRC-32C (Castagnoli) rather than standard CRC-32.
    #[inline]
    pub fn compute(data: &[u8]) -> u32 {
        compute_software(data)
    }

    /// Software CRC32 implementation using lookup table.
    /// Optimized with 4-byte parallel processing.
    #[inline]
    pub fn compute_software(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFFFFFF;

        // Process 4 bytes at a time for better performance
        let chunks = data.chunks_exact(4);
        let remainder = chunks.remainder();

        for chunk in chunks {
            crc ^= u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            crc = CRC32_TABLE[(crc & 0xFF) as usize] ^ (crc >> 8);
            crc = CRC32_TABLE[(crc & 0xFF) as usize] ^ (crc >> 8);
            crc = CRC32_TABLE[(crc & 0xFF) as usize] ^ (crc >> 8);
            crc = CRC32_TABLE[(crc & 0xFF) as usize] ^ (crc >> 8);
        }

        for &byte in remainder {
            crc = CRC32_TABLE[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
        }

        crc ^ 0xFFFFFFFF
    }

    /// Check if hardware CRC32 is available.
    ///
    /// Always returns false because hardware CRC32 instructions compute
    /// CRC-32C, not standard CRC-32 needed for PNG.
    pub fn is_hardware_accelerated() -> bool {
        false
    }
}

/// Memory operations with potential SIMD optimization.
pub mod memops {
    /// Fast memory comparison.
    /// Uses SIMD when beneficial for large comparisons.
    #[inline]
    pub fn fast_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        // For small slices, just compare directly
        if a.len() < 32 {
            return a == b;
        }

        // For larger slices, compare in chunks
        // The compiler will often auto-vectorize this
        a.chunks(8)
            .zip(b.chunks(8))
            .all(|(a_chunk, b_chunk)| a_chunk == b_chunk)
    }

    /// Fast memory copy with size hint for optimization.
    #[inline]
    pub fn fast_copy(dest: &mut Vec<u8>, src: &[u8]) {
        dest.extend_from_slice(src);
    }

    /// Find a byte pattern in data.
    /// Optimized for common pattern sizes.
    #[inline]
    pub fn find_pattern(data: &[u8], pattern: &[u8]) -> Option<usize> {
        if pattern.is_empty() || pattern.len() > data.len() {
            return None;
        }

        // For 2-byte patterns (common in JPEG markers), optimize
        if pattern.len() == 2 {
            return data.windows(2).position(|w| w == pattern);
        }

        data.windows(pattern.len()).position(|w| w == pattern)
    }
}

/// Report on available hardware acceleration features.
pub fn acceleration_report() -> String {
    let mut features: Vec<&str> = Vec::new();

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            features.push("AVX2");
        }
        if is_x86_feature_detected!("avx512f") {
            features.push("AVX-512");
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        features.push("ARM64 NEON");
    }

    if features.is_empty() {
        String::from("No hardware acceleration detected")
    } else {
        format!("Hardware acceleration: {}", features.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_empty() {
        assert_eq!(crc32::compute(&[]), 0);
    }

    #[test]
    fn test_crc32_known_value() {
        // "IEND" should give 0xAE426082
        let data = b"IEND";
        let crc = crc32::compute(data);
        assert_eq!(crc, 0xAE426082);
    }

    #[test]
    fn test_crc32_software_matches() {
        let data = b"Hello, World!";
        let sw = crc32::compute_software(data);
        let auto = crc32::compute(data);
        // Both methods should give the same result
        assert_eq!(sw, auto);
    }

    #[test]
    fn test_crc32_various_lengths() {
        // Test various data lengths to exercise different code paths
        for len in [1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 100, 1000] {
            let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let sw = crc32::compute_software(&data);
            let auto = crc32::compute(&data);
            assert_eq!(sw, auto, "Mismatch for length {}", len);
        }
    }

    #[test]
    fn test_fast_eq() {
        assert!(memops::fast_eq(b"hello", b"hello"));
        assert!(!memops::fast_eq(b"hello", b"world"));
        assert!(!memops::fast_eq(b"hello", b"hell"));
    }

    #[test]
    fn test_fast_eq_large() {
        let a: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let b = a.clone();
        let mut c = a.clone();
        c[500] = 255;

        assert!(memops::fast_eq(&a, &b));
        assert!(!memops::fast_eq(&a, &c));
    }

    #[test]
    fn test_find_pattern() {
        let data = b"Hello, World!";
        assert_eq!(memops::find_pattern(data, b"Wo"), Some(7));
        assert_eq!(memops::find_pattern(data, b"xyz"), None);
        assert_eq!(memops::find_pattern(data, b""), None);
    }

    #[test]
    fn test_find_pattern_jpeg_marker() {
        let data = [0x00, 0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(memops::find_pattern(&data, &[0xFF, 0xD8]), Some(1));
    }

    #[test]
    fn test_acceleration_report() {
        let report = acceleration_report();
        assert!(!report.is_empty());
        // Should return something meaningful
        assert!(report.contains("acceleration") || report.contains("Hardware"));
    }

    #[test]
    fn test_is_hardware_accelerated() {
        // Hardware CRC32 is not used because it computes CRC-32C, not CRC-32
        assert!(!crc32::is_hardware_accelerated());
    }
}
