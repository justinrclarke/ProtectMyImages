//! PMI - Protect My Images
//!
//! A CLI tool that strips metadata from images to protect user privacy.
//!
//! # Supported Formats
//!
//! - JPEG (.jpg, .jpeg)
//! - PNG (.png)
//! - GIF (.gif)
//! - WebP (.webp)
//! - TIFF (.tif, .tiff)
//!
//! # Features
//!
//! - Parallel processing using a thread pool
//! - SIMD-accelerated CRC32 for PNG processing (SSE4.2 on x86_64)
//! - Zero external dependencies
//!
//! # Example
//!
//! ```no_run
//! use pmi::cli::Config;
//! use pmi::processor::Processor;
//!
//! let config = Config::parse(["pmi", "image.jpg"]).unwrap();
//! let mut processor = Processor::new(config);
//! let stats = processor.run().unwrap();
//! println!("Processed {} files", stats.processed);
//! ```

pub mod cli;
pub mod error;
pub mod formats;
pub mod parallel;
pub mod processor;
pub mod simd;
pub mod terminal;

pub use cli::Config;
pub use error::{Error, Result};
pub use formats::{ImageFormat, detect_format, strip_metadata};
pub use parallel::{ThreadPool, available_parallelism};
pub use processor::Processor;
pub use simd::acceleration_report;
pub use terminal::{ProcessingStats, print_error, print_info, print_success, print_warning};
