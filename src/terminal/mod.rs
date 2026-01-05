//! Terminal utilities for colored output and progress reporting.

pub mod colors;
pub mod progress;

pub use colors::{
    Color, Style, Styled, Symbols, format_size, print_error, print_info, print_success,
    print_warning, stderr_supports_color, stdout_supports_color,
};
pub use progress::{ProcessingStats, ProgressBar, ProgressConfig, Spinner, print_summary};
