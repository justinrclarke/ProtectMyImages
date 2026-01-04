//! Command-line argument parsing for PMI.
//!
//! This module provides a hand-rolled argument parser without external dependencies.

use crate::error::{Error, Result};
use std::path::PathBuf;

/// Application version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name.
pub const NAME: &str = "pmi";

/// CLI configuration parsed from command-line arguments.
#[derive(Debug, Clone)]
pub struct Config {
    /// Input paths (files or directories).
    pub paths: Vec<PathBuf>,
    /// Output directory for cleaned images.
    pub output_dir: Option<PathBuf>,
    /// Process directories recursively.
    pub recursive: bool,
    /// Overwrite existing output files.
    pub force: bool,
    /// Modify files in place.
    pub in_place: bool,
    /// Show detailed processing info.
    pub verbose: bool,
    /// Suppress all output except errors.
    pub quiet: bool,
    /// Show what would be done without making changes.
    pub dry_run: bool,
    /// Show help message.
    pub help: bool,
    /// Show version.
    pub version: bool,
    /// Number of parallel jobs (threads) for processing.
    pub jobs: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            output_dir: None,
            recursive: false,
            force: false,
            in_place: false,
            verbose: false,
            quiet: false,
            dry_run: false,
            help: false,
            version: false,
            jobs: None,
        }
    }
}

impl Config {
    /// Parse configuration from command-line arguments.
    pub fn parse<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut config = Config::default();
        let mut args = args.into_iter().peekable();

        // Skip the program name if present.
        args.next();

        while let Some(arg) = args.next() {
            let arg = arg.as_ref();

            if arg.starts_with("--") {
                // Long option.
                let opt = &arg[2..];

                if opt.contains('=') {
                    // --option=value format.
                    let (key, value) = opt.split_once('=').unwrap();
                    config.handle_long_option_with_value(key, value)?;
                } else {
                    // --option or --option value format.
                    config.handle_long_option(opt, &mut args)?;
                }
            } else if arg.starts_with('-') && arg.len() > 1 {
                // Short option(s).
                let chars: Vec<char> = arg[1..].chars().collect();

                for (i, c) in chars.iter().enumerate() {
                    let is_last = i == chars.len() - 1;
                    config.handle_short_option(*c, is_last, &mut args)?;
                }
            } else {
                // Positional argument (path).
                config.paths.push(PathBuf::from(arg));
            }
        }

        // Validate configuration.
        if !config.help && !config.version && config.paths.is_empty() {
            return Err(Error::MissingArgument {
                argument: String::from("<PATHS>"),
            });
        }

        // Cannot use both quiet and verbose.
        if config.quiet && config.verbose {
            return Err(Error::InvalidArgument {
                argument: String::from("--quiet, --verbose"),
                reason: String::from("Cannot use both --quiet and --verbose"),
            });
        }

        Ok(config)
    }

    fn handle_long_option<I, S>(&mut self, opt: &str, args: &mut std::iter::Peekable<I>) -> Result<()>
    where
        I: Iterator<Item = S>,
        S: AsRef<str>,
    {
        match opt {
            "help" => self.help = true,
            "version" => self.version = true,
            "recursive" => self.recursive = true,
            "force" => self.force = true,
            "in-place" => self.in_place = true,
            "verbose" => self.verbose = true,
            "quiet" => self.quiet = true,
            "dry-run" => self.dry_run = true,
            "output-dir" => {
                let value = args.next().ok_or_else(|| Error::MissingArgument {
                    argument: String::from("--output-dir <DIR>"),
                })?;
                self.output_dir = Some(PathBuf::from(value.as_ref()));
            }
            "jobs" => {
                let value = args.next().ok_or_else(|| Error::MissingArgument {
                    argument: String::from("--jobs <N>"),
                })?;
                self.jobs = Some(parse_jobs(value.as_ref())?);
            }
            _ => {
                return Err(Error::InvalidArgument {
                    argument: format!("--{}", opt),
                    reason: String::from("Unknown option"),
                });
            }
        }
        Ok(())
    }

    fn handle_long_option_with_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "output-dir" => {
                self.output_dir = Some(PathBuf::from(value));
            }
            "jobs" => {
                self.jobs = Some(parse_jobs(value)?);
            }
            _ => {
                return Err(Error::InvalidArgument {
                    argument: format!("--{}", key),
                    reason: String::from("Unknown option or option does not take a value"),
                });
            }
        }
        Ok(())
    }

    fn handle_short_option<I, S>(
        &mut self,
        c: char,
        is_last: bool,
        args: &mut std::iter::Peekable<I>,
    ) -> Result<()>
    where
        I: Iterator<Item = S>,
        S: AsRef<str>,
    {
        match c {
            'h' => self.help = true,
            'V' => self.version = true,
            'r' => self.recursive = true,
            'f' => self.force = true,
            'i' => self.in_place = true,
            'v' => self.verbose = true,
            'q' => self.quiet = true,
            'n' => self.dry_run = true,
            'o' => {
                if !is_last {
                    return Err(Error::InvalidArgument {
                        argument: String::from("-o"),
                        reason: String::from("-o must be the last option in a combined flag"),
                    });
                }
                let value = args.next().ok_or_else(|| Error::MissingArgument {
                    argument: String::from("-o <DIR>"),
                })?;
                self.output_dir = Some(PathBuf::from(value.as_ref()));
            }
            'j' => {
                if !is_last {
                    return Err(Error::InvalidArgument {
                        argument: String::from("-j"),
                        reason: String::from("-j must be the last option in a combined flag"),
                    });
                }
                let value = args.next().ok_or_else(|| Error::MissingArgument {
                    argument: String::from("-j <N>"),
                })?;
                self.jobs = Some(parse_jobs(value.as_ref())?);
            }
            _ => {
                return Err(Error::InvalidArgument {
                    argument: format!("-{}", c),
                    reason: String::from("Unknown option"),
                });
            }
        }
        Ok(())
    }
}

/// Parse a jobs value (positive integer).
fn parse_jobs(value: &str) -> Result<usize> {
    value.parse::<usize>().map_err(|_| Error::InvalidArgument {
        argument: String::from("--jobs"),
        reason: format!("'{}' is not a valid number", value),
    }).and_then(|n| {
        if n == 0 {
            Err(Error::InvalidArgument {
                argument: String::from("--jobs"),
                reason: String::from("Number of jobs must be at least 1"),
            })
        } else {
            Ok(n)
        }
    })
}

/// Generate the help message.
pub fn help_message() -> String {
    format!(
        r#"{} {} - Protect My Images

Strip metadata from images to protect your privacy.

USAGE:
    {} [OPTIONS] <PATHS>...

ARGUMENTS:
    <PATHS>...    Image files or directories to process

OPTIONS:
    -o, --output-dir <DIR>    Save cleaned images to specified directory
    -r, --recursive           Process directories recursively
    -f, --force               Overwrite existing output files
    -i, --in-place            Modify files in place (default: create *_clean suffix)
    -j, --jobs <N>            Number of parallel threads (default: auto-detect CPU cores)
    -v, --verbose             Show detailed processing information
    -q, --quiet               Suppress all output except errors
    -n, --dry-run             Show what would be done without making changes
    -h, --help                Print this help message
    -V, --version             Print version information

EXAMPLES:
    {} photo.jpg                      Create photo_clean.jpg
    {} -i photo.jpg                   Overwrite photo.jpg in place
    {} -o ./clean/ *.jpg              Output to ./clean/ directory
    {} -r ./photos/                   Process directory recursively
    {} -j 4 -r ./photos/              Process with 4 threads
    {} -n -v ./photos/                Dry run with verbose output

SUPPORTED FORMATS:
    JPEG (.jpg, .jpeg)
    PNG  (.png)
    GIF  (.gif)
    WebP (.webp)
    TIFF (.tif, .tiff)
"#,
        NAME, VERSION, NAME, NAME, NAME, NAME, NAME, NAME, NAME
    )
}

/// Generate the version message.
pub fn version_message() -> String {
    format!("{} {}", NAME, VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_path() {
        let config = Config::parse(["pmi", "photo.jpg"]).unwrap();
        assert_eq!(config.paths.len(), 1);
        assert_eq!(config.paths[0], PathBuf::from("photo.jpg"));
    }

    #[test]
    fn test_parse_multiple_paths() {
        let config = Config::parse(["pmi", "a.jpg", "b.png", "c.gif"]).unwrap();
        assert_eq!(config.paths.len(), 3);
    }

    #[test]
    fn test_parse_help_short() {
        let config = Config::parse(["pmi", "-h"]).unwrap();
        assert!(config.help);
    }

    #[test]
    fn test_parse_help_long() {
        let config = Config::parse(["pmi", "--help"]).unwrap();
        assert!(config.help);
    }

    #[test]
    fn test_parse_version_short() {
        let config = Config::parse(["pmi", "-V"]).unwrap();
        assert!(config.version);
    }

    #[test]
    fn test_parse_version_long() {
        let config = Config::parse(["pmi", "--version"]).unwrap();
        assert!(config.version);
    }

    #[test]
    fn test_parse_recursive() {
        let config = Config::parse(["pmi", "-r", "dir/"]).unwrap();
        assert!(config.recursive);
    }

    #[test]
    fn test_parse_force() {
        let config = Config::parse(["pmi", "--force", "file.jpg"]).unwrap();
        assert!(config.force);
    }

    #[test]
    fn test_parse_in_place() {
        let config = Config::parse(["pmi", "-i", "file.jpg"]).unwrap();
        assert!(config.in_place);
    }

    #[test]
    fn test_parse_verbose() {
        let config = Config::parse(["pmi", "-v", "file.jpg"]).unwrap();
        assert!(config.verbose);
    }

    #[test]
    fn test_parse_quiet() {
        let config = Config::parse(["pmi", "-q", "file.jpg"]).unwrap();
        assert!(config.quiet);
    }

    #[test]
    fn test_parse_dry_run() {
        let config = Config::parse(["pmi", "-n", "file.jpg"]).unwrap();
        assert!(config.dry_run);
    }

    #[test]
    fn test_parse_output_dir_short() {
        let config = Config::parse(["pmi", "-o", "/output", "file.jpg"]).unwrap();
        assert_eq!(config.output_dir, Some(PathBuf::from("/output")));
    }

    #[test]
    fn test_parse_output_dir_long() {
        let config = Config::parse(["pmi", "--output-dir", "/output", "file.jpg"]).unwrap();
        assert_eq!(config.output_dir, Some(PathBuf::from("/output")));
    }

    #[test]
    fn test_parse_output_dir_equals() {
        let config = Config::parse(["pmi", "--output-dir=/output", "file.jpg"]).unwrap();
        assert_eq!(config.output_dir, Some(PathBuf::from("/output")));
    }

    #[test]
    fn test_parse_combined_short_flags() {
        let config = Config::parse(["pmi", "-rfv", "dir/"]).unwrap();
        assert!(config.recursive);
        assert!(config.force);
        assert!(config.verbose);
    }

    #[test]
    fn test_parse_combined_flags_with_value() {
        let config = Config::parse(["pmi", "-rfo", "/output", "dir/"]).unwrap();
        assert!(config.recursive);
        assert!(config.force);
        assert_eq!(config.output_dir, Some(PathBuf::from("/output")));
    }

    #[test]
    fn test_missing_paths() {
        let result = Config::parse(["pmi"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_quiet_and_verbose_conflict() {
        let result = Config::parse(["pmi", "-q", "-v", "file.jpg"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_long_option() {
        let result = Config::parse(["pmi", "--unknown", "file.jpg"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_short_option() {
        let result = Config::parse(["pmi", "-x", "file.jpg"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_output_dir_value() {
        let result = Config::parse(["pmi", "-o"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_help_message_contains_usage() {
        let help = help_message();
        assert!(help.contains("USAGE:"));
        assert!(help.contains("OPTIONS:"));
        assert!(help.contains("EXAMPLES:"));
    }

    #[test]
    fn test_version_message() {
        let version = version_message();
        assert!(version.contains("pmi"));
    }

    #[test]
    fn test_parse_jobs_short() {
        let config = Config::parse(["pmi", "-j", "4", "file.jpg"]).unwrap();
        assert_eq!(config.jobs, Some(4));
    }

    #[test]
    fn test_parse_jobs_long() {
        let config = Config::parse(["pmi", "--jobs", "8", "file.jpg"]).unwrap();
        assert_eq!(config.jobs, Some(8));
    }

    #[test]
    fn test_parse_jobs_equals() {
        let config = Config::parse(["pmi", "--jobs=12", "file.jpg"]).unwrap();
        assert_eq!(config.jobs, Some(12));
    }

    #[test]
    fn test_parse_jobs_invalid() {
        let result = Config::parse(["pmi", "-j", "abc", "file.jpg"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_jobs_zero() {
        let result = Config::parse(["pmi", "-j", "0", "file.jpg"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_jobs_missing_value() {
        let result = Config::parse(["pmi", "-j"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_jobs_combined_flags() {
        let config = Config::parse(["pmi", "-rvj", "4", "dir/"]).unwrap();
        assert!(config.recursive);
        assert!(config.verbose);
        assert_eq!(config.jobs, Some(4));
    }
}
