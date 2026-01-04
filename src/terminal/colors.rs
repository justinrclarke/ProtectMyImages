//! ANSI color codes and terminal styling.
//!
//! Provides colorized output for the CLI using ANSI escape sequences.

use std::fmt;
use std::io::{self, IsTerminal};

/// ANSI escape code prefix.
const ESC: &str = "\x1b[";

/// ANSI reset code.
const RESET: &str = "\x1b[0m";

/// Check if stdout is a terminal that supports colors.
pub fn stdout_supports_color() -> bool {
    io::stdout().is_terminal() && !no_color_env()
}

/// Check if stderr is a terminal that supports colors.
pub fn stderr_supports_color() -> bool {
    io::stderr().is_terminal() && !no_color_env()
}

/// Check if the NO_COLOR environment variable is set.
fn no_color_env() -> bool {
    std::env::var("NO_COLOR").is_ok()
}

/// Terminal colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Default,
    /// 256-color mode (0-255).
    Color256(u8),
}

impl Color {
    /// Get the ANSI foreground color code.
    fn fg_code(self) -> String {
        match self {
            Color::Black => String::from("30"),
            Color::Red => String::from("31"),
            Color::Green => String::from("32"),
            Color::Yellow => String::from("33"),
            Color::Blue => String::from("34"),
            Color::Magenta => String::from("35"),
            Color::Cyan => String::from("36"),
            Color::White => String::from("37"),
            Color::Default => String::from("39"),
            Color::Color256(n) => format!("38;5;{}", n),
        }
    }
}

/// Text style attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Bold,
    Dim,
    Italic,
    Underline,
}

impl Style {
    /// Get the ANSI style code.
    fn code(self) -> &'static str {
        match self {
            Style::Bold => "1",
            Style::Dim => "2",
            Style::Italic => "3",
            Style::Underline => "4",
        }
    }
}

/// A styled string with color and formatting.
#[derive(Debug, Clone)]
pub struct Styled {
    content: String,
    color: Option<Color>,
    styles: Vec<Style>,
    enabled: bool,
}

impl Styled {
    /// Create a new styled string.
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            color: None,
            styles: Vec::new(),
            enabled: stdout_supports_color(),
        }
    }

    /// Create a styled string with explicit color support setting.
    pub fn with_color_support(content: impl Into<String>, enabled: bool) -> Self {
        Self {
            content: content.into(),
            color: None,
            styles: Vec::new(),
            enabled,
        }
    }

    /// Set the foreground color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Add a style attribute.
    pub fn style(mut self, style: Style) -> Self {
        if !self.styles.contains(&style) {
            self.styles.push(style);
        }
        self
    }

    /// Make the text bold.
    pub fn bold(self) -> Self {
        self.style(Style::Bold)
    }

    /// Make the text dim.
    pub fn dim(self) -> Self {
        self.style(Style::Dim)
    }

    /// Make the text red.
    pub fn red(self) -> Self {
        self.color(Color::Red)
    }

    /// Make the text green.
    pub fn green(self) -> Self {
        self.color(Color::Green)
    }

    /// Make the text yellow.
    pub fn yellow(self) -> Self {
        self.color(Color::Yellow)
    }

    /// Make the text blue.
    pub fn blue(self) -> Self {
        self.color(Color::Blue)
    }

    /// Make the text cyan.
    pub fn cyan(self) -> Self {
        self.color(Color::Cyan)
    }
}

impl fmt::Display for Styled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.enabled || (self.color.is_none() && self.styles.is_empty()) {
            return write!(f, "{}", self.content);
        }

        // Build the escape sequence.
        let mut codes = Vec::new();

        for style in &self.styles {
            codes.push(style.code().to_string());
        }

        if let Some(color) = self.color {
            codes.push(color.fg_code());
        }

        write!(f, "{}{}{}{}", ESC, codes.join(";"), "m", self.content)?;
        write!(f, "{}", RESET)
    }
}

/// Message symbols with colors.
pub struct Symbols {
    enabled: bool,
}

impl Symbols {
    /// Create a new symbols instance.
    pub fn new(color_enabled: bool) -> Self {
        Self {
            enabled: color_enabled,
        }
    }

    /// Success symbol (green checkmark).
    pub fn success(&self) -> Styled {
        Styled::with_color_support("\u{2713}", self.enabled).green().bold()
    }

    /// Error symbol (red X).
    pub fn error(&self) -> Styled {
        Styled::with_color_support("\u{2717}", self.enabled).red().bold()
    }

    /// Warning symbol (yellow warning sign).
    pub fn warning(&self) -> Styled {
        Styled::with_color_support("\u{26A0}", self.enabled).yellow().bold()
    }

    /// Info symbol (blue info).
    pub fn info(&self) -> Styled {
        Styled::with_color_support("\u{2139}", self.enabled).blue().bold()
    }

    /// Arrow symbol for progress.
    pub fn arrow(&self) -> Styled {
        Styled::with_color_support("\u{2192}", self.enabled).cyan()
    }
}

/// Print a success message.
pub fn print_success(message: &str) {
    let symbols = Symbols::new(stdout_supports_color());
    println!("{} {}", symbols.success(), message);
}

/// Print an error message to stderr.
pub fn print_error(message: &str) {
    let symbols = Symbols::new(stderr_supports_color());
    eprintln!("{} {}", symbols.error(), message);
}

/// Print a warning message.
pub fn print_warning(message: &str) {
    let symbols = Symbols::new(stdout_supports_color());
    println!("{} {}", symbols.warning(), message);
}

/// Print an info message.
pub fn print_info(message: &str) {
    let symbols = Symbols::new(stdout_supports_color());
    println!("{} {}", symbols.info(), message);
}

/// Format a file size in human-readable form.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_styled_no_formatting() {
        let s = Styled::with_color_support("hello", false);
        assert_eq!(s.to_string(), "hello");
    }

    #[test]
    fn test_styled_with_color_disabled() {
        let s = Styled::with_color_support("hello", false).red().bold();
        assert_eq!(s.to_string(), "hello");
    }

    #[test]
    fn test_styled_with_color_enabled() {
        let s = Styled::with_color_support("hello", true).red();
        let output = s.to_string();
        assert!(output.contains("\x1b["));
        assert!(output.contains("31")); // Red color code
        assert!(output.contains("hello"));
        assert!(output.contains("\x1b[0m")); // Reset
    }

    #[test]
    fn test_styled_bold() {
        let s = Styled::with_color_support("hello", true).bold();
        let output = s.to_string();
        assert!(output.contains("1")); // Bold code
    }

    #[test]
    fn test_styled_multiple_styles() {
        let s = Styled::with_color_support("hello", true).bold().red();
        let output = s.to_string();
        assert!(output.contains("1")); // Bold
        assert!(output.contains("31")); // Red
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn test_symbols() {
        let symbols = Symbols::new(false);
        assert!(symbols.success().to_string().contains('\u{2713}'));
        assert!(symbols.error().to_string().contains('\u{2717}'));
        assert!(symbols.warning().to_string().contains('\u{26A0}'));
        assert!(symbols.info().to_string().contains('\u{2139}'));
    }
}
