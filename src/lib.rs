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
pub mod processor;
pub mod terminal;

pub use cli::Config;
pub use error::{Error, Result};
pub use formats::{detect_format, strip_metadata, ImageFormat};
pub use processor::Processor;
pub use terminal::{print_error, print_info, print_success, print_warning, ProcessingStats};
