/*!
 * Git repository cache management
 */

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Clean up old repositories from cache
pub fn clean_cache(days: u64) -> io::Result<usize> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("~/.cache"))
        .join("dumpfs");

    if !cache_dir.exists() {
        return Ok(0);
    }

    let now = SystemTime::now();
    let max_age = Duration::from_secs(days * 24 * 60 * 60);

    // Clean all provider directories
    let providers = ["github", "gitlab", "bitbucket", "git"];

    providers
        .iter()
        .map(|provider| cache_dir.join(provider))
        .filter(|path| path.exists())
        .try_fold(0, |acc, path| {
            let count = clean_cache_dir(&path, &max_age, &now)?;
            Ok(acc + count)
        })
}

/// Clean up repositories in a specific cache directory
fn clean_cache_dir(dir: &Path, max_age: &Duration, now: &SystemTime) -> io::Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }

    let mut count = 0;

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if path.join(".git").exists() {
            // It's a repository, check age
            if let Ok(metadata) = fs::metadata(&path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(age) = now.duration_since(modified) {
                        if age > *max_age {
                            // Remove old repository
                            fs::remove_dir_all(&path)?;
                            count += 1;
                        }
                    }
                }
            }
        } else {
            // It's a directory structure (like owner), recurse
            count += clean_cache_dir(&path, max_age, now)?;
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_clean_cache() -> io::Result<()> {
        // Create a temporary directory for testing
        let temp_dir = tempdir()?;
        let cache_dir = temp_dir.path().join("dumpfs");

        // Create structure for a GitHub repo
        let repo_path = cache_dir.join("github").join("username").join("repo");
        fs::create_dir_all(&repo_path)?;

        // Create a .git directory to identify it as a repo
        fs::create_dir_all(repo_path.join(".git"))?;

        // Create a file with old modification time
        let file_path = repo_path.join("test.txt");
        let mut file = File::create(&file_path)?;
        writeln!(file, "Test content")?;

        // Override cache dir location for testing
        let original_cache_dir = env::var("XDG_CACHE_HOME").ok();
        env::set_var("XDG_CACHE_HOME", temp_dir.path());

        // Call clean_cache_dir directly with zero days (should clean everything)
        let now = SystemTime::now();
        let max_age = Duration::from_secs(0); // 0 days means everything is older
        let cleaned = clean_cache_dir(&cache_dir.join("github"), &max_age, &now)?;

        assert_eq!(cleaned, 1); // Should clean up our one repo

        // Restore original env var
        if let Some(original) = original_cache_dir {
            env::set_var("XDG_CACHE_HOME", original);
        } else {
            env::remove_var("XDG_CACHE_HOME");
        }

        Ok(())
    }
}
