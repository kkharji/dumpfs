/*!
 * Git repository operations
 */

use std::fs;
use std::path::PathBuf;

use git2::{FetchOptions, RemoteCallbacks, Repository as Git2Repository};

use super::error::{GitError, GitResult};
use super::progress::{GitProgress, ProgressReporter};
use super::url::GitRepoInfo;

/// Git repository with associated information
pub struct Repository {
    /// Inner git2 repository instance
    inner: Git2Repository,
    /// Repository information
    info: GitRepoInfo,
}

impl Repository {
    /// Open an existing Git repository
    pub fn open(info: GitRepoInfo) -> GitResult<Self> {
        let repo = Git2Repository::open(&info.cache_path).map_err(GitError::OpenError)?;

        Ok(Self { inner: repo, info })
    }

    /// Check if a repository exists at the given cache path
    pub fn exists(info: &GitRepoInfo) -> bool {
        info.cache_path.join(".git").exists()
    }

    /// Clone a Git repository
    pub fn clone<P: ProgressReporter>(info: GitRepoInfo, progress: Option<&P>) -> GitResult<Self> {
        // Create cache directory if it doesn't exist
        fs::create_dir_all(&info.cache_path).map_err(GitError::IoError)?;

        // Setup builder with progress reporting
        let mut builder = git2::build::RepoBuilder::new();

        if let Some(reporter) = progress {
            let mut callbacks = RemoteCallbacks::new();
            callbacks.transfer_progress(|stats| {
                let progress = GitProgress {
                    total_objects: stats.total_objects(),
                    received_objects: stats.received_objects(),
                    indexed_objects: stats.indexed_objects(),
                    local_objects: stats.local_objects(),
                    total_deltas: stats.total_deltas(),
                    indexed_deltas: stats.indexed_deltas(),
                    received_bytes: stats.received_bytes(),
                };
                reporter.report(&progress);
                true
            });

            let mut fetch_options = FetchOptions::new();
            fetch_options.remote_callbacks(callbacks);
            builder.fetch_options(fetch_options);
        }

        // Clone the repository
        let repo = builder
            .clone(&info.url, &info.cache_path)
            .map_err(GitError::CloneError)?;

        Ok(Self { inner: repo, info })
    }

    /// Pull latest changes for an existing repository
    pub fn pull<P: ProgressReporter>(&mut self, progress: Option<&P>) -> GitResult<()> {
        // Set up fetch options with progress reporting
        let mut fetch_options = FetchOptions::new();

        if let Some(reporter) = progress {
            let mut callbacks = RemoteCallbacks::new();
            callbacks.transfer_progress(|stats| {
                let progress = GitProgress {
                    total_objects: stats.total_objects(),
                    received_objects: stats.received_objects(),
                    indexed_objects: stats.indexed_objects(),
                    local_objects: stats.local_objects(),
                    total_deltas: stats.total_deltas(),
                    indexed_deltas: stats.indexed_deltas(),
                    received_bytes: stats.received_bytes(),
                };
                reporter.report(&progress);
                true
            });

            fetch_options.remote_callbacks(callbacks);
        }

        // Fetch from remote
        let mut remote = self
            .inner
            .find_remote("origin")
            .map_err(GitError::FetchError)?;

        remote
            .fetch(&["main", "master"], Some(&mut fetch_options), None)
            .map_err(GitError::FetchError)?;

        // Find remote branch to reset to
        let remote_branch = self
            .inner
            .find_reference("refs/remotes/origin/master")
            .or_else(|_| self.inner.find_reference("refs/remotes/origin/main"))
            .map_err(GitError::FetchError)?;

        // Get object to reset to
        let obj = self
            .inner
            .revparse_single(remote_branch.name().unwrap())
            .map_err(GitError::FetchError)?;

        // Reset to remote branch
        self.inner
            .reset(&obj, git2::ResetType::Hard, None)
            .map_err(GitError::FetchError)?;

        Ok(())
    }

    /// Get repository information
    pub fn info(&self) -> &GitRepoInfo {
        &self.info
    }

    /// Get path to the repository
    pub fn path(&self) -> &PathBuf {
        &self.info.cache_path
    }
}

/// Repository operation builder for more flexible configuration
pub struct RepositoryBuilder {
    /// Repository information
    info: GitRepoInfo,
    /// Optional fetch options
    fetch_options: Option<FetchOptions<'static>>,
}

impl RepositoryBuilder {
    /// Create a new repository builder
    pub fn new(info: GitRepoInfo) -> Self {
        Self {
            info,
            fetch_options: None,
        }
    }

    /// Configure with progress reporting
    pub fn with_progress<P: ProgressReporter + 'static>(mut self, reporter: P) -> Self {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.transfer_progress(move |stats| {
            let progress = GitProgress {
                total_objects: stats.total_objects(),
                received_objects: stats.received_objects(),
                indexed_objects: stats.indexed_objects(),
                local_objects: stats.local_objects(),
                total_deltas: stats.total_deltas(),
                indexed_deltas: stats.indexed_deltas(),
                received_bytes: stats.received_bytes(),
            };
            reporter.report(&progress);
            true
        });

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);
        self.fetch_options = Some(fetch_options);

        self
    }

    /// Clone the repository
    pub fn clone(self) -> GitResult<Repository> {
        // Create cache directory if it doesn't exist
        fs::create_dir_all(&self.info.cache_path).map_err(GitError::IoError)?;

        // Setup builder
        let mut builder = git2::build::RepoBuilder::new();

        if let Some(fetch_options) = self.fetch_options {
            builder.fetch_options(fetch_options);
        }

        // Clone the repository
        let repo = builder
            .clone(&self.info.url, &self.info.cache_path)
            .map_err(GitError::CloneError)?;

        Ok(Repository {
            inner: repo,
            info: self.info,
        })
    }

    /// Open an existing repository
    pub fn open(self) -> GitResult<Repository> {
        Repository::open(self.info)
    }
}
