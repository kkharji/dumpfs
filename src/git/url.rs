/*!
 * Git URL parsing and handling
 */

use std::path::PathBuf;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

use super::error::{GitError, GitResult};

// Statically compiled regexes for better performance
static HTTP_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^https?://(?:www\.)?(?:github\.com|gitlab\.com|bitbucket\.org|.*)/[^/]+/[^/]+(?:\.git)?$",
    )
    .unwrap()
});

static SSH_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^git@(?:github\.com|gitlab\.com|bitbucket\.org|[^:]+):[^/]+/[^/]+(?:\.git)?$")
        .unwrap()
});

static SSH_PARSE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^git@([^:]+):([^/]+)/([^/]+)(?:\.git)?$").unwrap());

/// Git hosting platform types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHost {
    /// GitHub repository
    GitHub,
    /// GitLab repository
    GitLab,
    /// Bitbucket repository
    Bitbucket,
    /// Other Git hosting
    Other(String),
}

impl std::fmt::Display for GitHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitHost::GitHub => write!(f, "GitHub"),
            GitHost::GitLab => write!(f, "GitLab"),
            GitHost::Bitbucket => write!(f, "Bitbucket"),
            GitHost::Other(host) => write!(f, "{}", host),
        }
    }
}

/// Information about a Git repository
#[derive(Debug, Clone)]
pub struct GitRepoInfo {
    /// Original URL
    pub url: String,
    /// Git hosting platform
    pub host: GitHost,
    /// Repository owner/username
    pub owner: String,
    /// Repository name
    pub name: String,
    /// Local cache path
    pub cache_path: PathBuf,
}

impl std::fmt::Display for GitRepoInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.host, self.owner, self.name)
    }
}

impl FromStr for GitRepoInfo {
    type Err = GitError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        // Check if the URL is valid
        if !HTTP_REGEX.is_match(url) && !SSH_REGEX.is_match(url) {
            return Err(GitError::InvalidUrl(url.to_string()));
        }

        // Handle HTTP/HTTPS URLs
        if url.starts_with("http://") || url.starts_with("https://") {
            if let Ok(parsed_url) = Url::parse(url) {
                let host_str = parsed_url
                    .host_str()
                    .ok_or_else(|| GitError::InvalidUrl(format!("Invalid host in URL: {}", url)))?;

                // Get path without leading slash
                let path = parsed_url.path();
                let path = path.strip_prefix('/').unwrap_or(path);

                let path_segments: Vec<&str> = path.split('/').collect();

                if path_segments.len() < 2 {
                    return Err(GitError::InvalidUrl(format!(
                        "Missing owner or repository in URL: {}",
                        url
                    )));
                }

                let owner = path_segments[0].to_string();
                let mut name = path_segments[1].to_string();

                // Remove .git suffix if present
                if name.ends_with(".git") {
                    name = name[0..name.len() - 4].to_string();
                }

                let host = match host_str {
                    "github.com" => GitHost::GitHub,
                    "gitlab.com" => GitHost::GitLab,
                    "bitbucket.org" => GitHost::Bitbucket,
                    _ => GitHost::Other(host_str.to_string()),
                };

                let cache_path = get_cache_path(&host, &owner, &name);

                return Ok(GitRepoInfo {
                    url: url.to_string(),
                    host,
                    owner,
                    name,
                    cache_path,
                });
            }
        }

        // Handle SSH URLs (git@github.com:owner/repo.git)
        if url.starts_with("git@") {
            if let Some(captures) = SSH_PARSE_REGEX.captures(url) {
                if let (Some(host_match), Some(owner_match), Some(name_match)) =
                    (captures.get(1), captures.get(2), captures.get(3))
                {
                    let host_str = host_match.as_str();
                    let owner = owner_match.as_str().to_string();
                    let mut name = name_match.as_str().to_string();

                    // Remove .git suffix if present
                    if name.ends_with(".git") {
                        name = name[0..name.len() - 4].to_string();
                    }

                    let host = match host_str {
                        "github.com" => GitHost::GitHub,
                        "gitlab.com" => GitHost::GitLab,
                        "bitbucket.org" => GitHost::Bitbucket,
                        _ => GitHost::Other(host_str.to_string()),
                    };

                    let cache_path = get_cache_path(&host, &owner, &name);

                    return Ok(GitRepoInfo {
                        url: url.to_string(),
                        host,
                        owner,
                        name,
                        cache_path,
                    });
                }
            }
        }

        Err(GitError::InvalidUrl(url.to_string()))
    }
}

/// Check if a path is a Git repository URL
pub fn is_git_url(path: &str) -> bool {
    path.parse::<GitRepoInfo>().is_ok()
}

/// Parse a Git repository URL into components
pub fn parse_git_url(url: &str) -> GitResult<GitRepoInfo> {
    url.parse()
}

/// Get the cache directory path for a repository
pub fn get_cache_path(host: &GitHost, owner: &str, name: &str) -> PathBuf {
    let mut cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("~/.cache"));
    cache_dir = cache_dir.join("dumpfs");

    match host {
        GitHost::GitHub => cache_dir.join("github").join(owner).join(name),
        GitHost::GitLab => cache_dir.join("gitlab").join(owner).join(name),
        GitHost::Bitbucket => cache_dir.join("bitbucket").join(owner).join(name),
        GitHost::Other(host_name) => cache_dir.join("git").join(host_name).join(owner).join(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_git_url() {
        // Test GitHub URLs
        assert!(is_git_url(&"https://github.com/username/repo".to_string()));
        assert!(is_git_url(
            &"https://github.com/username/repo.git".to_string()
        ));
        assert!(is_git_url(&"git@github.com:username/repo.git".to_string()));

        // Test GitLab URLs
        assert!(is_git_url(&"https://gitlab.com/username/repo".to_string()));
        assert!(is_git_url(
            &"https://gitlab.com/username/repo.git".to_string()
        ));
        assert!(is_git_url(&"git@gitlab.com:username/repo.git".to_string()));

        // Test Bitbucket URLs
        assert!(is_git_url(
            &"https://bitbucket.org/username/repo".to_string()
        ));
        assert!(is_git_url(
            &"https://bitbucket.org/username/repo.git".to_string()
        ));
        assert!(is_git_url(
            &"git@bitbucket.org:username/repo.git".to_string()
        ));

        // Test custom Git host URLs
        assert!(is_git_url(
            &"https://git.example.com/username/repo".to_string()
        ));
        assert!(is_git_url(
            &"https://git.example.com/username/repo.git".to_string()
        ));
        assert!(is_git_url(
            &"git@git.example.com:username/repo.git".to_string()
        ));

        // Test invalid URLs
        assert!(!is_git_url(&"https://github.com".to_string()));
        assert!(!is_git_url(&"https://github.com/username".to_string()));
        assert!(!is_git_url(&"git@github.com".to_string()));
        assert!(!is_git_url(&"/path/to/local/directory".to_string()));
        assert!(!is_git_url(&"username/repo".to_string()));
    }

    #[test]
    fn test_parse_git_url() {
        // Test GitHub HTTPS URL
        let repo = parse_git_url(&"https://github.com/username/repo".to_string()).unwrap();
        assert_eq!(repo.url, "https://github.com/username/repo");
        assert!(matches!(repo.host, GitHost::GitHub));
        assert_eq!(repo.owner, "username");
        assert_eq!(repo.name, "repo");

        // Test GitHub SSH URL
        let repo = parse_git_url(&"git@github.com:username/repo.git".to_string()).unwrap();
        assert_eq!(repo.url, "git@github.com:username/repo.git");
        assert!(matches!(repo.host, GitHost::GitHub));
        assert_eq!(repo.owner, "username");
        assert_eq!(repo.name, "repo");

        // Test custom host cache path
        let host = GitHost::Other("example.com".to_string());
        let owner = "username";
        let name = "repo";
        let cache_path = get_cache_path(&host, owner, name);
        assert!(cache_path.ends_with(
            &std::path::Path::new("git")
                .join("example.com")
                .join("username")
                .join("repo")
        ));
    }
}
