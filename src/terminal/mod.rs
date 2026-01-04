//! Terminal utilities for colored output and progress reporting.

pub mod colors;
pub mod progress;

pub use colors::{
    format_size, print_error, print_info, print_success, print_warning,
    stderr_supports_color, stdout_supports_color, Color, Style, Styled, Symbols,
};
pub use progress::{print_summary, ProcessingStats, ProgressBar, ProgressConfig, Spinner};
