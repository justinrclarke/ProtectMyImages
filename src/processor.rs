//! File processing pipeline.
//!
//! This module handles file discovery, format detection, and processing.
//! Supports parallel processing for improved performance on multi-core systems.

use crate::cli::Config;
use crate::error::{Error, Result};
use crate::formats::{detect_format_from_extension, strip_metadata};
use crate::parallel::{self, ThreadPool};
use crate::terminal::{
    ProcessingStats, ProgressBar, Styled, format_size, print_error, print_info, print_success,
    print_warning, stdout_supports_color,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Instant;

/// Result of processing a single file.
#[derive(Debug)]
pub enum ProcessResult {
    /// File was successfully processed.
    Success {
        input: PathBuf,
        output: PathBuf,
        bytes_removed: u64,
    },
    /// File was skipped (unsupported format, etc.).
    Skipped { path: PathBuf, reason: String },
    /// Processing failed.
    Failed { path: PathBuf, error: String },
}

// Make ProcessResult Send + Sync for parallel processing
unsafe impl Send for ProcessResult {}

/// File processor with parallel execution support.
pub struct Processor {
    config: Config,
    stats: ProcessingStats,
    start_time: Instant,
}

impl Processor {
    /// Create a new processor with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            stats: ProcessingStats::new(),
            start_time: Instant::now(),
        }
    }

    /// Process all files from the configuration paths.
    pub fn run(&mut self) -> Result<ProcessingStats> {
        // Collect files to process.
        let files = self.collect_files()?;

        if files.is_empty() {
            if !self.config.quiet {
                print_warning("No supported image files found");
            }
            return Ok(self.stats.clone());
        }

        let file_count = files.len();
        let num_jobs = self
            .config
            .jobs
            .unwrap_or_else(parallel::available_parallelism);

        if !self.config.quiet {
            let parallel_info = if num_jobs > 1 {
                format!(" (using {} threads)", num_jobs)
            } else {
                String::new()
            };
            print_info(&format!(
                "Found {} image file(s) to process{}",
                file_count, parallel_info
            ));

            // Show hardware acceleration info in verbose mode
            if self.config.verbose {
                print_info(&crate::simd::acceleration_report());
            }
        }

        // Create output directory if needed.
        if let Some(ref output_dir) = self.config.output_dir {
            if !output_dir.exists() {
                if self.config.dry_run {
                    if self.config.verbose {
                        print_info(&format!("Would create directory: {}", output_dir.display()));
                    }
                } else {
                    fs::create_dir_all(output_dir)
                        .map_err(|e| Error::io_with_path(e, output_dir))?;
                }
            }
        }

        // Process files (parallel or sequential based on configuration)
        if num_jobs > 1 && file_count > 1 {
            self.run_parallel(files, num_jobs)?;
        } else {
            self.run_sequential(files)?;
        }

        self.stats.set_duration(self.start_time.elapsed());
        Ok(self.stats.clone())
    }

    /// Run processing sequentially.
    fn run_sequential(&mut self, files: Vec<PathBuf>) -> Result<()> {
        let mut progress = ProgressBar::new(files.len());

        for path in &files {
            progress.set_current_file(path.display().to_string());

            let result = self.process_file(path);
            self.handle_result(result);

            if !self.config.quiet {
                progress.inc();
            }
        }

        if !self.config.quiet {
            progress.finish();
        }

        Ok(())
    }

    /// Run processing in parallel using a thread pool.
    fn run_parallel(&mut self, files: Vec<PathBuf>, num_threads: usize) -> Result<()> {
        let file_count = files.len();
        let mut progress = ProgressBar::new(file_count);

        // Channel for receiving results from workers
        let (result_tx, result_rx) = mpsc::channel::<ProcessResult>();

        // Create shared configuration for workers
        let config = Arc::new(self.config.clone());

        // Create thread pool
        let pool = ThreadPool::new(num_threads);

        // Submit all jobs
        for path in files {
            let result_tx = result_tx.clone();
            let config = Arc::clone(&config);

            pool.execute(move || {
                let result = process_file_standalone(&path, &config);
                let _ = result_tx.send(result);
            });
        }

        // Drop our sender so the channel closes when all workers are done
        drop(result_tx);

        // Collect results as they complete
        let mut completed = 0;
        while let Ok(result) = result_rx.recv() {
            // Update progress bar
            if let ProcessResult::Success { ref input, .. } = result {
                progress.set_current_file(input.display().to_string());
            }

            self.handle_result(result);
            completed += 1;

            if !self.config.quiet {
                progress.set(completed);
            }
        }

        if !self.config.quiet {
            progress.finish();
        }

        Ok(())
    }

    /// Collect all files to process from the configuration paths.
    fn collect_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for path in &self.config.paths {
            if path.is_file() {
                if self.is_supported_file(path) {
                    files.push(path.clone());
                } else if self.config.verbose {
                    print_warning(&format!("Skipping unsupported file: {}", path.display()));
                }
            } else if path.is_dir() {
                self.collect_from_directory(path, &mut files)?;
            } else if !path.exists() {
                return Err(Error::NotFound { path: path.clone() });
            }
        }

        Ok(files)
    }

    /// Collect files from a directory.
    fn collect_from_directory(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        let entries = fs::read_dir(dir).map_err(|e| Error::io_with_path(e, dir))?;

        for entry in entries {
            let entry = entry.map_err(|e| Error::io_with_path(e, dir))?;
            let path = entry.path();

            if path.is_file() {
                if self.is_supported_file(&path) {
                    files.push(path);
                }
            } else if path.is_dir() && self.config.recursive {
                self.collect_from_directory(&path, files)?;
            }
        }

        Ok(())
    }

    /// Check if a file has a supported extension.
    fn is_supported_file(&self, path: &Path) -> bool {
        detect_format_from_extension(path).is_some()
    }

    /// Process a single file (instance method for sequential processing).
    fn process_file(&self, path: &Path) -> ProcessResult {
        process_file_standalone(path, &self.config)
    }

    /// Get the output path for a file.
    pub fn get_output_path(config: &Config, input: &Path) -> PathBuf {
        if config.in_place {
            return input.to_path_buf();
        }

        if let Some(ref output_dir) = config.output_dir {
            let file_name = input.file_name().unwrap_or_default();
            return output_dir.join(file_name);
        }

        // Default: add _clean suffix.
        let stem = input.file_stem().unwrap_or_default().to_string_lossy();
        let ext = input
            .extension()
            .map(|e| e.to_string_lossy())
            .unwrap_or_default();

        if ext.is_empty() {
            input.with_file_name(format!("{}_clean", stem))
        } else {
            input.with_file_name(format!("{}_clean.{}", stem, ext))
        }
    }

    /// Handle a processing result.
    fn handle_result(&mut self, result: ProcessResult) {
        let color_enabled = stdout_supports_color();

        match result {
            ProcessResult::Success {
                input,
                output,
                bytes_removed,
            } => {
                self.stats.add_success(bytes_removed);

                if !self.config.quiet {
                    let input_name = input.file_name().unwrap_or_default().to_string_lossy();
                    let msg = if self.config.dry_run {
                        format!(
                            "Would clean {} (would remove {})",
                            Styled::with_color_support(input_name.as_ref(), color_enabled).blue(),
                            format_size(bytes_removed)
                        )
                    } else if self.config.in_place {
                        format!(
                            "Cleaned {} (removed {})",
                            Styled::with_color_support(input_name.as_ref(), color_enabled).blue(),
                            format_size(bytes_removed)
                        )
                    } else {
                        let output_name = output.file_name().unwrap_or_default().to_string_lossy();
                        format!(
                            "Cleaned {} {} {} (removed {})",
                            Styled::with_color_support(input_name.as_ref(), color_enabled).blue(),
                            Styled::with_color_support("\u{2192}", color_enabled).dim(),
                            Styled::with_color_support(output_name.as_ref(), color_enabled).blue(),
                            format_size(bytes_removed)
                        )
                    };
                    print_success(&msg);
                }
            }
            ProcessResult::Skipped { path, reason } => {
                self.stats.add_skipped();

                if !self.config.quiet && self.config.verbose {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    print_warning(&format!("Skipped {}: {}", name, reason));
                }
            }
            ProcessResult::Failed { path, error } => {
                self.stats.add_failure();

                if !self.config.quiet {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    print_error(&format!("Failed to process {}: {}", name, error));
                }
            }
        }
    }
}

/// Process a single file (standalone function for parallel execution).
fn process_file_standalone(path: &Path, config: &Config) -> ProcessResult {
    // Read the file.
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            return ProcessResult::Failed {
                path: path.to_path_buf(),
                error: Error::io_with_path(e, path).to_string(),
            };
        }
    };

    // Strip metadata.
    let strip_result = match strip_metadata(&data, path) {
        Ok(r) => r,
        Err(e) => {
            return ProcessResult::Failed {
                path: path.to_path_buf(),
                error: e.to_string(),
            };
        }
    };

    // Determine output path.
    let output_path = Processor::get_output_path(config, path);

    // Check if output exists.
    if output_path.exists() && !config.force && !config.in_place {
        return ProcessResult::Failed {
            path: path.to_path_buf(),
            error: format!(
                "Output file '{}' already exists (use --force to overwrite)",
                output_path.display()
            ),
        };
    }

    // Write output (or simulate for dry run).
    if config.dry_run {
        ProcessResult::Success {
            input: path.to_path_buf(),
            output: output_path,
            bytes_removed: strip_result.bytes_removed,
        }
    } else {
        match write_output_standalone(&output_path, &strip_result.data, config.in_place) {
            Ok(()) => ProcessResult::Success {
                input: path.to_path_buf(),
                output: output_path,
                bytes_removed: strip_result.bytes_removed,
            },
            Err(e) => ProcessResult::Failed {
                path: path.to_path_buf(),
                error: e,
            },
        }
    }
}

/// Write output to a file atomically (standalone function for parallel execution).
fn write_output_standalone(
    path: &Path,
    data: &[u8],
    in_place: bool,
) -> std::result::Result<(), String> {
    if in_place {
        let temp_path = path.with_extension("pmi_tmp");

        let mut file = fs::File::create(&temp_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;
        file.write_all(data)
            .map_err(|e| format!("Failed to write temp file: {}", e))?;
        file.flush()
            .map_err(|e| format!("Failed to flush temp file: {}", e))?;

        fs::rename(&temp_path, path).map_err(|e| format!("Failed to rename temp file: {}", e))?;
    } else {
        let mut file =
            fs::File::create(path).map_err(|e| format!("Failed to create output file: {}", e))?;
        file.write_all(data)
            .map_err(|e| format!("Failed to write output file: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config(paths: Vec<PathBuf>) -> Config {
        Config {
            paths,
            output_dir: None,
            recursive: false,
            force: false,
            in_place: false,
            verbose: false,
            quiet: true,
            dry_run: false,
            help: false,
            version: false,
            jobs: Some(1), // Sequential for tests.
        }
    }

    #[test]
    fn test_get_output_path_default() {
        let config = create_test_config(vec![]);

        let input = PathBuf::from("/path/to/image.jpg");
        let output = Processor::get_output_path(&config, &input);
        assert_eq!(output, PathBuf::from("/path/to/image_clean.jpg"));
    }

    #[test]
    fn test_get_output_path_in_place() {
        let mut config = create_test_config(vec![]);
        config.in_place = true;

        let input = PathBuf::from("/path/to/image.jpg");
        let output = Processor::get_output_path(&config, &input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_get_output_path_output_dir() {
        let mut config = create_test_config(vec![]);
        config.output_dir = Some(PathBuf::from("/output"));

        let input = PathBuf::from("/path/to/image.jpg");
        let output = Processor::get_output_path(&config, &input);
        assert_eq!(output, PathBuf::from("/output/image.jpg"));
    }

    #[test]
    fn test_is_supported_file() {
        let config = create_test_config(vec![]);
        let processor = Processor::new(config);

        assert!(processor.is_supported_file(Path::new("test.jpg")));
        assert!(processor.is_supported_file(Path::new("test.jpeg")));
        assert!(processor.is_supported_file(Path::new("test.png")));
        assert!(processor.is_supported_file(Path::new("test.gif")));
        assert!(processor.is_supported_file(Path::new("test.webp")));
        assert!(processor.is_supported_file(Path::new("test.tiff")));

        assert!(!processor.is_supported_file(Path::new("test.txt")));
        assert!(!processor.is_supported_file(Path::new("test.pdf")));
        assert!(!processor.is_supported_file(Path::new("test.bmp")));
    }
}
