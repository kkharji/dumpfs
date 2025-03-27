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
pub use error::{GitError, GitResult};
pub use progress::{GitProgress, ProgressReporter};
pub use repository::{Repository, RepositoryBuilder};
pub use url::{is_git_url, parse_git_url, GitHost, GitRepoInfo};

use std::io;
use std::path::PathBuf;

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
