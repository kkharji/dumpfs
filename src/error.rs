//! Global error handling for dumpfs
//!
//! This module provides a centralized error type that can represent errors
//! from all modules in the project.

use std::io;
use thiserror::Error;

use crate::git::GitError;
use crate::tokenizer::TokenizerError;

/// Global error type for dumpfs operations
#[derive(Error, Debug)]
pub enum DumpFsError {
    /// Git-related errors
    #[error("Git error: {0}")]
    Git(#[from] GitError),

    /// Tokenizer-related errors
    #[error("Tokenizer error: {0}")]
    Tokenizer(#[from] TokenizerError),

    /// File system errors
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// XML processing errors
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// JSON processing errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Regular expression errors
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    /// Scanner errors
    #[error("Scanner error: {0}")]
    Scanner(String),

    /// Writer errors
    #[error("Writer error: {0}")]
    Writer(String),

    /// Path not found
    #[error("Path not found: {0}")]
    PathNotFound(String),

    /// Invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Unexpected error
    #[error("Unexpected error: {0}")]
    Unexpected(String),
}

/// Specialized Result type for dumpfs operations
pub type Result<T> = std::result::Result<T, DumpFsError>;

/// Creates a DumpFsError with a formatted message
#[macro_export]
macro_rules! error {
    ($error_type:ident, $($arg:tt)*) => {
        $crate::error::DumpFsError::$error_type(format!($($arg)*))
    };
}

/// Returns an error result with a formatted message
#[macro_export]
macro_rules! bail {
    ($error_type:ident, $($arg:tt)*) => {
        return Err($crate::error!($error_type, $($arg)*))
    };
}

/// Ensures a condition is true, otherwise returns an error
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $error_type:ident, $($arg:tt)*) => {
        if !($cond) {
            $crate::bail!($error_type, $($arg)*)
        }
    };
}

/// Extension trait for adding context to errors
pub trait ResultExt<T, E> {
    /// Add additional context to an error
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display;
}

impl<T, E: std::error::Error + 'static> ResultExt<T, E> for std::result::Result<T, E> {
    fn with_context<C, F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> C,
        C: std::fmt::Display,
    {
        self.map_err(|e| {
            let context = f();
            DumpFsError::Unexpected(format!("{}: {}", context, e))
        })
    }
}

// Allow converting DumpFsError to io::Error for backward compatibility with tests
impl From<DumpFsError> for io::Error {
    fn from(err: DumpFsError) -> Self {
        io::Error::new(io::ErrorKind::Other, err.to_string())
    }
}
