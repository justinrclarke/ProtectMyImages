//! Progress bar and status reporting.
//!
//! Provides progress indication for batch operations.

use super::colors::{format_size, stdout_supports_color, Styled, Symbols};
use std::io::{self, IsTerminal, Write};
use std::time::{Duration, Instant};

/// Progress bar configuration.
#[derive(Debug, Clone)]
pub struct ProgressConfig {
    /// Total number of items to process.
    pub total: usize,
    /// Width of the progress bar in characters.
    pub width: usize,
    /// Whether to show the progress bar.
    pub enabled: bool,
    /// Character for completed progress.
    pub filled_char: char,
    /// Character for remaining progress.
    pub empty_char: char,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            total: 0,
            width: 20,
            enabled: io::stdout().is_terminal(),
            filled_char: '\u{2588}', // Full block
            empty_char: '\u{2591}', // Light shade
        }
    }
}

/// Progress bar state.
pub struct ProgressBar {
    config: ProgressConfig,
    current: usize,
    current_file: String,
    start_time: Instant,
    color_enabled: bool,
}

impl ProgressBar {
    /// Create a new progress bar.
    pub fn new(total: usize) -> Self {
        Self {
            config: ProgressConfig {
                total,
                ..Default::default()
            },
            current: 0,
            current_file: String::new(),
            start_time: Instant::now(),
            color_enabled: stdout_supports_color(),
        }
    }

    /// Create a progress bar with custom configuration.
    pub fn with_config(config: ProgressConfig) -> Self {
        Self {
            color_enabled: stdout_supports_color(),
            config,
            current: 0,
            current_file: String::new(),
            start_time: Instant::now(),
        }
    }

    /// Check if the progress bar is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Set the current file being processed.
    pub fn set_current_file(&mut self, file: impl Into<String>) {
        self.current_file = file.into();
    }

    /// Increment the progress by one.
    pub fn inc(&mut self) {
        self.current += 1;
        if self.config.enabled {
            self.render();
        }
    }

    /// Set the progress to a specific value.
    pub fn set(&mut self, current: usize) {
        self.current = current;
        if self.config.enabled {
            self.render();
        }
    }

    /// Get the current progress percentage.
    pub fn percentage(&self) -> f64 {
        if self.config.total == 0 {
            0.0
        } else {
            (self.current as f64 / self.config.total as f64) * 100.0
        }
    }

    /// Get the elapsed time.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Render the progress bar to stdout.
    fn render(&self) {
        let percent = self.percentage();
        let filled = ((percent / 100.0) * self.config.width as f64) as usize;
        let empty = self.config.width.saturating_sub(filled);

        let bar: String = std::iter::repeat(self.config.filled_char)
            .take(filled)
            .chain(std::iter::repeat(self.config.empty_char).take(empty))
            .collect();

        let file_display = if self.current_file.len() > 30 {
            format!("...{}", &self.current_file[self.current_file.len() - 27..])
        } else {
            self.current_file.clone()
        };

        let bar_styled = Styled::with_color_support(&bar, self.color_enabled).cyan();

        // Use carriage return to overwrite the line.
        print!(
            "\r[{}] {}/{} files ({:.0}%) - {}",
            bar_styled, self.current, self.config.total, percent, file_display
        );

        // Clear any remaining characters from previous line.
        print!("                    ");
        print!("\r[{}] {}/{} files ({:.0}%) - {}",
            bar_styled, self.current, self.config.total, percent, file_display);

        let _ = io::stdout().flush();
    }

    /// Finish the progress bar and move to a new line.
    pub fn finish(&self) {
        if self.config.enabled {
            println!();
        }
    }

    /// Finish with a message.
    pub fn finish_with_message(&self, message: &str) {
        if self.config.enabled {
            println!("\r{}", message);
        }
    }
}

/// Statistics for the processing summary.
#[derive(Debug, Default, Clone)]
pub struct ProcessingStats {
    /// Number of successfully processed files.
    pub processed: usize,
    /// Number of failed files.
    pub failed: usize,
    /// Number of skipped files.
    pub skipped: usize,
    /// Total bytes of metadata removed.
    pub metadata_removed: u64,
    /// Processing duration.
    pub duration: Duration,
}

impl ProcessingStats {
    /// Create new empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a successful processing result.
    pub fn add_success(&mut self, metadata_bytes: u64) {
        self.processed += 1;
        self.metadata_removed += metadata_bytes;
    }

    /// Add a failed processing result.
    pub fn add_failure(&mut self) {
        self.failed += 1;
    }

    /// Add a skipped file.
    pub fn add_skipped(&mut self) {
        self.skipped += 1;
    }

    /// Set the duration.
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// Get the total number of files encountered.
    pub fn total(&self) -> usize {
        self.processed + self.failed + self.skipped
    }
}

/// Print a summary report box.
pub fn print_summary(stats: &ProcessingStats, quiet: bool) {
    if quiet {
        return;
    }

    let color_enabled = stdout_supports_color();
    let symbols = Symbols::new(color_enabled);

    // Box drawing characters.
    let top_left = '\u{256D}';
    let top_right = '\u{256E}';
    let bottom_left = '\u{2570}';
    let bottom_right = '\u{256F}';
    let horizontal = '\u{2500}';
    let vertical = '\u{2502}';
    let t_right = '\u{251C}';
    let t_left = '\u{2524}';

    let width = 40;
    let inner_width = width - 2;

    let horizontal_line: String = std::iter::repeat(horizontal).take(inner_width).collect();

    // Build the box.
    println!();
    println!(
        "{}{}{}",
        top_left, horizontal_line, top_right
    );

    // Title.
    let title = "Processing Complete";
    let padding = (inner_width - title.len()) / 2;
    println!(
        "{}{:padding$}{}{:rest$}{}",
        vertical,
        "",
        Styled::with_color_support(title, color_enabled).bold(),
        "",
        vertical,
        padding = padding,
        rest = inner_width - padding - title.len()
    );

    println!(
        "{}{}{}",
        t_right, horizontal_line, t_left
    );

    // Stats.
    let processed_str = format!(
        "  {} Processed:  {} files",
        symbols.success(),
        stats.processed
    );
    println!(
        "{}{:<width$}{}",
        vertical,
        processed_str,
        vertical,
        width = inner_width
    );

    if stats.failed > 0 {
        let failed_str = format!(
            "  {} Failed:      {} files",
            symbols.error(),
            stats.failed
        );
        println!(
            "{}{:<width$}{}",
            vertical,
            failed_str,
            vertical,
            width = inner_width
        );
    }

    if stats.skipped > 0 {
        let skipped_str = format!(
            "  {} Skipped:     {} files",
            symbols.warning(),
            stats.skipped
        );
        println!(
            "{}{:<width$}{}",
            vertical,
            skipped_str,
            vertical,
            width = inner_width
        );
    }

    // Empty line.
    println!("{}{:width$}{}", vertical, "", vertical, width = inner_width);

    // Metadata removed.
    let metadata_str = format!(
        "  Metadata removed: {}",
        format_size(stats.metadata_removed)
    );
    println!(
        "{}{:<width$}{}",
        vertical,
        metadata_str,
        vertical,
        width = inner_width
    );

    // Duration.
    let duration_str = format!("  Time elapsed: {:.1}s", stats.duration.as_secs_f64());
    println!(
        "{}{:<width$}{}",
        vertical,
        duration_str,
        vertical,
        width = inner_width
    );

    println!(
        "{}{}{}",
        bottom_left, horizontal_line, bottom_right
    );
}

/// Spinner for indeterminate progress.
pub struct Spinner {
    frames: Vec<char>,
    current_frame: usize,
    message: String,
    enabled: bool,
}

impl Spinner {
    /// Create a new spinner with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            frames: vec!['\u{280B}', '\u{2819}', '\u{2839}', '\u{2838}', '\u{283C}', '\u{2834}', '\u{2826}', '\u{2827}', '\u{2807}', '\u{280F}'],
            current_frame: 0,
            message: message.into(),
            enabled: io::stdout().is_terminal(),
        }
    }

    /// Advance the spinner and render.
    pub fn tick(&mut self) {
        if !self.enabled {
            return;
        }

        let frame = self.frames[self.current_frame];
        self.current_frame = (self.current_frame + 1) % self.frames.len();

        let color_enabled = stdout_supports_color();
        let styled_frame = Styled::with_color_support(frame.to_string(), color_enabled).cyan();

        print!("\r{} {}", styled_frame, self.message);
        let _ = io::stdout().flush();
    }

    /// Finish the spinner with a final message.
    pub fn finish(&self, message: &str) {
        if self.enabled {
            print!("\r");
            // Clear the line.
            print!("{:width$}", "", width = self.message.len() + 5);
            print!("\r{}\n", message);
            let _ = io::stdout().flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_percentage() {
        let pb = ProgressBar::new(100);
        assert_eq!(pb.percentage(), 0.0);
    }

    #[test]
    fn test_progress_bar_percentage_after_inc() {
        let mut pb = ProgressBar::new(100);
        pb.config.enabled = false; // Disable rendering for test.
        pb.inc();
        assert_eq!(pb.percentage(), 1.0);
    }

    #[test]
    fn test_progress_bar_percentage_half() {
        let mut pb = ProgressBar::new(100);
        pb.config.enabled = false;
        pb.set(50);
        assert_eq!(pb.percentage(), 50.0);
    }

    #[test]
    fn test_progress_bar_percentage_zero_total() {
        let pb = ProgressBar::new(0);
        assert_eq!(pb.percentage(), 0.0);
    }

    #[test]
    fn test_processing_stats_default() {
        let stats = ProcessingStats::new();
        assert_eq!(stats.processed, 0);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.skipped, 0);
        assert_eq!(stats.metadata_removed, 0);
    }

    #[test]
    fn test_processing_stats_add_success() {
        let mut stats = ProcessingStats::new();
        stats.add_success(1024);
        assert_eq!(stats.processed, 1);
        assert_eq!(stats.metadata_removed, 1024);
    }

    #[test]
    fn test_processing_stats_add_failure() {
        let mut stats = ProcessingStats::new();
        stats.add_failure();
        assert_eq!(stats.failed, 1);
    }

    #[test]
    fn test_processing_stats_add_skipped() {
        let mut stats = ProcessingStats::new();
        stats.add_skipped();
        assert_eq!(stats.skipped, 1);
    }

    #[test]
    fn test_processing_stats_total() {
        let mut stats = ProcessingStats::new();
        stats.add_success(100);
        stats.add_success(100);
        stats.add_failure();
        stats.add_skipped();
        assert_eq!(stats.total(), 4);
    }

    #[test]
    fn test_spinner_creation() {
        let spinner = Spinner::new("Loading...");
        assert_eq!(spinner.message, "Loading...");
        assert_eq!(spinner.current_frame, 0);
    }
}
