//! Custom error types for PMI.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Result type alias for PMI operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during PMI operations.
#[derive(Debug)]
pub enum Error {
    /// I/O error with optional path context.
    Io {
        source: io::Error,
        path: Option<PathBuf>,
    },
    /// Invalid or corrupted image file.
    InvalidImage { path: PathBuf, reason: String },
    /// Unsupported image format.
    UnsupportedFormat {
        path: PathBuf,
        detected: Option<String>,
    },
    /// CLI argument parsing error.
    InvalidArgument { argument: String, reason: String },
    /// Missing required argument.
    MissingArgument { argument: String },
    /// File not found.
    NotFound { path: PathBuf },
    /// Permission denied.
    PermissionDenied { path: PathBuf },
    /// Output file already exists and --force not specified.
    OutputExists { path: PathBuf },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io { source, path } => {
                if let Some(p) = path {
                    write!(f, "I/O error for '{}': {}", p.display(), source)
                } else {
                    write!(f, "I/O error: {}", source)
                }
            }
            Error::InvalidImage { path, reason } => {
                write!(f, "Invalid image '{}': {}", path.display(), reason)
            }
            Error::UnsupportedFormat { path, detected } => {
                if let Some(fmt) = detected {
                    write!(f, "Unsupported format '{}' for '{}'", fmt, path.display())
                } else {
                    write!(f, "Unknown or unsupported format for '{}'", path.display())
                }
            }
            Error::InvalidArgument { argument, reason } => {
                write!(f, "Invalid argument '{}': {}", argument, reason)
            }
            Error::MissingArgument { argument } => {
                write!(f, "Missing required argument: {}", argument)
            }
            Error::NotFound { path } => {
                write!(f, "File not found: '{}'", path.display())
            }
            Error::PermissionDenied { path } => {
                write!(f, "Permission denied: '{}'", path.display())
            }
            Error::OutputExists { path } => {
                write!(
                    f,
                    "Output file '{}' already exists (use --force to overwrite)",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io {
            source: err,
            path: None,
        }
    }
}

impl Error {
    /// Create an I/O error with path context.
    pub fn io_with_path(err: io::Error, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        match err.kind() {
            io::ErrorKind::NotFound => Error::NotFound { path },
            io::ErrorKind::PermissionDenied => Error::PermissionDenied { path },
            _ => Error::Io {
                source: err,
                path: Some(path),
            },
        }
    }

    /// Create an invalid image error.
    pub fn invalid_image(path: impl Into<PathBuf>, reason: impl Into<String>) -> Self {
        Error::InvalidImage {
            path: path.into(),
            reason: reason.into(),
        }
    }

    /// Create an unsupported format error.
    pub fn unsupported_format(path: impl Into<PathBuf>, detected: Option<&str>) -> Self {
        Error::UnsupportedFormat {
            path: path.into(),
            detected: detected.map(String::from),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_display() {
        let err = Error::Io {
            source: io::Error::new(io::ErrorKind::Other, "test error"),
            path: None,
        };
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn test_io_error_with_path_display() {
        let err = Error::Io {
            source: io::Error::new(io::ErrorKind::Other, "test error"),
            path: Some(PathBuf::from("/test/path.jpg")),
        };
        assert!(err.to_string().contains("/test/path.jpg"));
    }

    #[test]
    fn test_invalid_image_display() {
        let err = Error::invalid_image("/test/image.jpg", "corrupted header");
        assert!(err.to_string().contains("Invalid image"));
        assert!(err.to_string().contains("corrupted header"));
    }

    #[test]
    fn test_unsupported_format_display() {
        let err = Error::unsupported_format("/test/file.bmp", Some("BMP"));
        assert!(err.to_string().contains("Unsupported format"));
        assert!(err.to_string().contains("BMP"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::Other, "test");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io { .. }));
    }

    #[test]
    fn test_io_with_path_not_found() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let err = Error::io_with_path(io_err, "/test/path");
        assert!(matches!(err, Error::NotFound { .. }));
    }

    #[test]
    fn test_io_with_path_permission_denied() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "denied");
        let err = Error::io_with_path(io_err, "/test/path");
        assert!(matches!(err, Error::PermissionDenied { .. }));
    }
}
