/*!
 * Git repository handling functionality
 */

mod cache;
mod error;
mod progress;
mod repository;
mod url;

// Re-export public items
pub use cache::clean_cache;
use clap::ValueEnum;
pub use error::{GitError, GitResult};
use indicatif::{ProgressBar, ProgressStyle};
use progress::ProgressBarAdapter;
pub use progress::{GitProgress, ProgressReporter};
pub use repository::{Repository, RepositoryBuilder};
pub use url::{is_git_url, parse_git_url, GitHost, GitRepoInfo};

use std::io;
use std::path::PathBuf;

/// Policy for handling Git repository caching
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GitCachePolicy {
    /// Always pull latest changes for existing repositories (default)
    AlwaysPull,
    /// Delete and re-clone existing repositories
    ForceClone,
    /// Use cached repositories without pulling updates
    UseCache,
}

impl Default for GitCachePolicy {
    fn default() -> Self {
        Self::AlwaysPull
    }
}

/// Clone or update a Git repository
///
/// This function maintains compatibility with the original API
/// while using the new implementation internally.
pub fn clone_repository<P: ProgressReporter>(
    url: &str,
    progress_fn: Option<&P>,
) -> io::Result<PathBuf> {
    // Parse the URL
    let info = match url::parse_git_url(url) {
        Ok(info) => info,
        Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidInput, e.to_string())),
    };

    // Check if repository already exists
    if Repository::exists(&info) {
        // Try to open and pull
        match Repository::open(info.clone()) {
            Ok(mut repo) => {
                if let Err(e) = repo.pull(progress_fn) {
                    return Err(io::Error::new(io::ErrorKind::Other, e.to_string()));
                }
                Ok(repo.path().clone())
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    } else {
        // Clone the repository
        match Repository::clone(info.clone(), progress_fn) {
            Ok(repo) => Ok(repo.path().clone()),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
        }
    }
}

// Create a progress reporter adapter
pub fn process_path(
    path: &str,
    git_cache_policy: GitCachePolicy,
    progress: Option<&ProgressBar>,
) -> GitResult<(PathBuf, Option<String>, Option<GitRepoInfo>)> {
    // If not a Git URL, just return the path as is
    if !is_git_url(path) {
        return Ok((PathBuf::from(path), None, None));
    }

    // Parse the Git URL
    let repo_info = parse_git_url(path)?;

    // Use the provided progress bar or create a new one
    let progress_bar = match progress {
        Some(p) => p,
        None => {
            // Create a new progress bar if none is provided
            let new_bar = ProgressBar::new(100);
            new_bar.set_style(ProgressStyle::default_bar().template(
                "{spinner:.green} {prefix:.bold.cyan} {msg} [{bar:40.cyan/blue}] {percent}%",
            )?);

            // Since this is a temporary variable, we'll just leak it to avoid ownership issues
            Box::leak(Box::new(new_bar))
        }
    };

    // Check if repository already exists
    let repo_exists = Repository::exists(&repo_info);

    // Handle based on policy
    match (git_cache_policy, repo_exists) {
        // Repository doesn't exist, always clone
        (_, false) => {
            progress_bar.set_prefix("🔄 Cloning");
            progress_bar.set_message(format!(
                "Cloning repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            let reporter = ProgressBarAdapter {
                bar: progress_bar,
                repo_info: &repo_info,
                is_clone: true,
            };

            let repo = Repository::clone(repo_info.clone(), Some(&reporter))
                .inspect(|_| {
                    progress_bar.finish_with_message(format!(
                        "Repository cloned: {}/{}",
                        repo_info.owner, repo_info.name
                    ));
                })
                .inspect_err(|e| {
                    progress_bar.abandon_with_message(format!("Failed to clone repository: {}", e));
                })?;

            Ok((repo.path().clone(), Some(path.to_string()), Some(repo_info)))
        }

        // Force clone even if exists
        (GitCachePolicy::ForceClone, true) => {
            // Delete existing repo
            progress_bar.set_prefix("🗑️ Removing");
            progress_bar.set_message(format!(
                "Removing existing repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            // Remove the directory to force a fresh clone
            std::fs::remove_dir_all(&repo_info.cache_path).inspect_err(|e| {
                progress_bar
                    .abandon_with_message(format!("Failed to remove existing repository: {}", e));
            })?;

            // Clone the repository
            progress_bar.set_prefix("🔄 Cloning");
            progress_bar.set_message(format!(
                "Cloning repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            let reporter = ProgressBarAdapter {
                bar: progress_bar,
                repo_info: &repo_info,
                is_clone: true,
            };

            let repo = Repository::clone(repo_info.clone(), Some(&reporter))
                .inspect_err(|e| {
                    progress_bar.abandon_with_message(format!("Failed to clone repository: {}", e))
                })
                .inspect(|_| {
                    progress_bar.finish_with_message(format!(
                        "Repository cloned: {}/{}",
                        repo_info.owner, repo_info.name
                    ))
                })?;

            Ok((repo.path().clone(), Some(path.to_string()), Some(repo_info)))
        }

        // Pull if exists
        (GitCachePolicy::AlwaysPull, true) => {
            progress_bar.set_prefix("🔄 Updating");
            progress_bar.set_message(format!(
                "Updating repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            let reporter = ProgressBarAdapter {
                bar: progress_bar,
                repo_info: &repo_info,
                is_clone: false,
            };

            let mut repo = Repository::open(repo_info.clone()).inspect_err(|e| {
                progress_bar.abandon_with_message(format!("Failed to open repository: {}", e));
            })?;

            repo.pull(Some(&reporter)).inspect_err(|e| {
                progress_bar.abandon_with_message(format!("Failed to update repository: {}", e))
            })?;

            progress_bar.finish_with_message(format!(
                "Repository updated: {}/{}",
                repo_info.owner, repo_info.name
            ));

            Ok((repo.path().clone(), Some(path.to_string()), Some(repo_info)))
        }

        // Use cache without pulling
        (GitCachePolicy::UseCache, true) => {
            progress_bar.set_prefix("📂 Using cached");
            progress_bar.set_message(format!(
                "Using cached repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            progress_bar.finish_with_message(format!(
                "Using cached repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            Ok((
                repo_info.cache_path.clone(),
                Some(path.to_string()),
                Some(repo_info),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    #[test]
    fn test_git_host_display() {
        assert_eq!(GitHost::GitHub.to_string(), "GitHub");
        assert_eq!(GitHost::GitLab.to_string(), "GitLab");
        assert_eq!(GitHost::Bitbucket.to_string(), "Bitbucket");
        assert_eq!(
            GitHost::Other("custom.com".to_string()).to_string(),
            "custom.com"
        );
    }

    #[test]
    fn test_git_repo_info_display() {
        let info = GitRepoInfo {
            url: "https://github.com/username/repo".to_string(),
            host: GitHost::GitHub,
            owner: "username".to_string(),
            name: "repo".to_string(),
            cache_path: PathBuf::from("/tmp/cache/github/username/repo"),
        };

        assert_eq!(info.to_string(), "GitHub/username/repo");
    }
}
