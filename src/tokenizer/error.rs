//! Error types for the tokenizer module

use std::io;
use thiserror::Error;

/// Result type for tokenizer operations
pub type TokenizerResult<T> = Result<T, TokenizerError>;

/// Errors that can occur during tokenization
#[derive(Error, Debug)]
pub enum TokenizerError {
    /// Error from API call
    #[error("API error: {0}")]
    ApiError(String),

    /// Error from tokenizer library
    #[error("Tokenizer error: {0}")]
    TokenizerError(String),

    /// Model is not supported
    #[error("Unsupported model: {0}")]
    UnsupportedModel(String),

    /// Required environment variable not set
    #[error("Environment variable not set: {0}")]
    EnvVarError(String),

    /// Cache operation error
    #[error("Cache error: {0}")]
    CacheError(String),

    /// Failed to acquire lock on cache
    #[error("Failed to acquire lock on cache")]
    CacheLockError,

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Request error
    #[error("Request error: {0}")]
    RequestError(String),
}

impl From<reqwest::Error> for TokenizerError {
    fn from(error: reqwest::Error) -> Self {
        TokenizerError::RequestError(error.to_string())
    }
}
