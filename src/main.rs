//! PMI - Protect My Images
//!
//! A CLI tool that strips metadata from images to protect user privacy.

use pmi::cli::{Config, help_message, version_message};
use pmi::processor::Processor;
use pmi::terminal::{print_error, print_summary};
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    // Parse CLI arguments.
    let config = match Config::parse(&args) {
        Ok(c) => c,
        Err(e) => {
            print_error(&e.to_string());
            eprintln!();
            eprintln!("For more information, try '--help'");
            return ExitCode::from(1);
        }
    };

    // Handle help flag.
    if config.help {
        println!("{}", help_message());
        return ExitCode::SUCCESS;
    }

    // Handle version flag.
    if config.version {
        println!("{}", version_message());
        return ExitCode::SUCCESS;
    }

    // Run the processor.
    let mut processor = Processor::new(config.clone());
    match processor.run() {
        Ok(stats) => {
            // Print summary.
            print_summary(&stats, config.quiet);

            // Exit with failure if any files failed.
            if stats.failed > 0 {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            print_error(&e.to_string());
            ExitCode::from(1)
        }
    }
}
