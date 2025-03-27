/*!
 * Error types for Git operations
 */

use thiserror::Error;

/// Errors that can occur during Git operations
#[derive(Error, Debug)]
pub enum GitError {
    /// Invalid Git URL format
    #[error("Invalid Git URL: {0}")]
    InvalidUrl(String),

    /// Error opening a Git repository
    #[error("Failed to open repository: {0}")]
    OpenError(git2::Error),

    /// Error cloning a Git repository
    #[error("Failed to clone repository: {0}")]
    CloneError(git2::Error),

    /// Error fetching from remote
    #[error("Failed to fetch from remote: {0}")]
    FetchError(git2::Error),

    /// Git2 error (generic)
    #[error("Git error: {0}")]
    Git2Error(#[from] git2::Error),

    /// IO error during Git operations
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Repository not found
    #[error("Repository not found: {0}")]
    NotFound(String),
}

/// Specialized Result type for Git operations
pub type GitResult<T> = Result<T, GitError>;
