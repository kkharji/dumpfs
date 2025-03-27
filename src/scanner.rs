/*!
 * Directory and file scanning functionality
 */

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use glob_match::glob_match;
use ignore::{DirEntry as IgnoreDirEntry, WalkBuilder};
use indicatif::ProgressBar;
use rayon::prelude::*;
use walkdir::{DirEntry, WalkDir};

use crate::config::Config;
use crate::types::{BinaryNode, DirectoryNode, FileNode, FileType, Metadata, Node, SymlinkNode};
use crate::utils::{format_file_size, DEFAULT_IGNORE};

use crate::report::FileReportInfo;
use crate::tokenizer::{create_tokenizer, get_global_cache_stats, Tokenizer};

/// Scanner statistics
#[derive(Debug, Clone, Default)]
pub struct ScannerStatistics {
    /// Number of files processed
    pub files_processed: usize,
    /// Total number of lines
    pub total_lines: usize,
    /// Total number of characters
    pub total_chars: usize,
    /// Total number of tokens (if tokenizer is enabled)
    pub total_tokens: Option<usize>,
    /// Details for each file
    pub file_details: HashMap<String, FileReportInfo>,
    /// Token cache hits (if tokenizer caching is enabled)
    pub token_cache_hits: Option<usize>,
    /// Token cache misses (if tokenizer caching is enabled)
    pub token_cache_misses: Option<usize>,
}

/// Scanner for directory contents
pub struct Scanner {
    /// Scanner configuration
    config: Config,
    /// Progress bar
    pub progress: Arc<ProgressBar>,
    /// Scanner statistics
    statistics: Arc<Mutex<ScannerStatistics>>,
    /// Tokenizer (if enabled)
    tokenizer: Option<Box<dyn Tokenizer>>,
}

impl Scanner {
    /// Create a new scanner
    pub fn new(config: Config, progress: Arc<ProgressBar>) -> Self {
        // Create tokenizer if model is specified
        let tokenizer = if let Some(model) = config.model {
            let project_dir = config.target_dir.to_string_lossy().to_string();
            match create_tokenizer(model, &project_dir) {
                Ok(t) => {
                    progress.set_message(format!("Using tokenizer for model: {model:?}"));
                    Some(t)
                }
                Err(e) => {
                    eprintln!("Error creating tokenizer: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            config,
            progress,
            statistics: Arc::new(Mutex::new(ScannerStatistics::default())),
            tokenizer,
        }
    }

    /// Normalize a path to be relative to the repository root
    pub fn normalize_path(&self, path: &Path) -> PathBuf {
        // If we have git repo info, make paths relative to repo root
        if let Some(repo_info) = &self.config.git_repo {
            // Try to make the path relative to the repository cache path
            if let Ok(rel_path) = path.strip_prefix(&repo_info.cache_path) {
                return if rel_path == Path::new("") {
                    // If it's the root, just return an empty path
                    PathBuf::new()
                } else {
                    // Otherwise, return the relative path
                    rel_path.to_path_buf()
                };
            }
        }

        // Default case: use the path as is
        path.to_path_buf()
    }

    /// Convert an absolute path to a normalized relative path for reporting
    pub fn get_normalized_path_for_reporting(&self, abs_path: &Path) -> String {
        if let Some(repo_info) = &self.config.git_repo {
            // For git repos, use owner/repo/path format
            if let Ok(rel_path) = abs_path.strip_prefix(&repo_info.cache_path) {
                // If it's a directory with no path components, just return owner/repo
                if rel_path == Path::new("") {
                    format!("{}/{}", repo_info.owner, repo_info.name)
                } else {
                    format!(
                        "{}/{}/{}",
                        repo_info.owner,
                        repo_info.name,
                        rel_path.display()
                    )
                }
            } else {
                // Fallback to full path
                abs_path.display().to_string()
            }
        } else {
            // For local paths, just use the path as is
            abs_path.display().to_string()
        }
    }

    /// Get scanner statistics
    pub fn get_statistics(&self) -> ScannerStatistics {
        let mut stats = self.statistics.lock().unwrap().clone();

        // If we have a tokenizer, get cache stats from global counters
        if self.tokenizer.is_some() {
            let cache_stats = get_global_cache_stats();
            stats.token_cache_hits = Some(cache_stats.hits);
            stats.token_cache_misses = Some(cache_stats.misses);
        }

        stats
    }

    /// Scan the target directory and return the directory tree
    pub fn scan(&self) -> io::Result<DirectoryNode> {
        let abs_path = fs::canonicalize(&self.config.target_dir)?;

        // Determine the base directory name and path
        let dir_name = if let Some(repo_info) = &self.config.git_repo {
            // For git repos, use the repo name
            repo_info.name.clone()
        } else {
            // For local directories, use the directory name
            abs_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };

        // Create the initial relative path
        let rel_path = PathBuf::from(&dir_name);

        self.scan_directory(&abs_path, &rel_path)
    }

    /// Scan a directory and return its node representation
    fn scan_directory(&self, abs_path: &Path, rel_path: &Path) -> io::Result<DirectoryNode> {
        let metadata = self.get_metadata(abs_path)?;
        let mut contents = Vec::new();

        // Determine which entries to process based on whether we're using gitignore
        if self.config.respect_gitignore {
            // Use ignore crate's Walk to handle .gitignore patterns
            let mut walker = WalkBuilder::new(abs_path);
            walker.max_depth(Some(1)); // Limit depth to just the current directory

            // Use custom gitignore file if specified
            if let Some(gitignore_path) = &self.config.gitignore_path {
                walker.add_custom_ignore_filename(gitignore_path);
            }

            // Get all entries using the ignore walker
            let entries: Vec<IgnoreDirEntry> = walker
                .build()
                .filter_map(Result::ok)
                .filter(|e| e.path() != abs_path) // Skip the root directory itself
                .filter(|e| !self.should_ignore(e.path()))
                .filter(|e| self.should_include(e.path()))
                .collect();

            // Split into directories and files
            let (dirs, files): (Vec<_>, Vec<_>) =
                entries.into_iter().partition(|e| e.path().is_dir());

            // Process directories first (sequential)
            for entry in dirs {
                let entry_path = entry.path();
                // Use normalize_path to get the correct relative path
                let normalized_path = self.normalize_path(entry_path);
                let new_rel_path = if normalized_path.components().count() > 0 {
                    // If we have a normalized path, use it
                    normalized_path
                } else {
                    // Otherwise, just join with the entry name
                    let entry_name = entry_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    rel_path.join(&entry_name)
                };

                match self.scan_directory(entry_path, &new_rel_path) {
                    Ok(dir_node) => contents.push(Node::Directory(dir_node)),
                    Err(e) => {
                        eprintln!("Error processing directory {}: {}", entry_path.display(), e)
                    }
                }
            }

            // Process files in parallel
            let file_nodes: Vec<Node> = files
                .par_iter()
                .filter_map(|entry| {
                    let entry_path = entry.path();
                    let entry_name = entry_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let new_rel_path = rel_path.join(&entry_name);

                    match self.process_file(entry_path, &new_rel_path) {
                        Ok(node) => Some(node),
                        Err(e) => {
                            eprintln!("Error processing {}: {}", entry_path.display(), e);
                            None
                        }
                    }
                })
                .collect();

            contents.extend(file_nodes);
        } else {
            // Use traditional walkdir approach when not respecting .gitignore
            let entries: Vec<DirEntry> = WalkDir::new(abs_path)
                .max_depth(1)
                .min_depth(1)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|e| !self.should_ignore(e.path()))
                .filter(|e| self.should_include(e.path()))
                .collect();

            // Split into directories and files
            let (dirs, files): (Vec<_>, Vec<_>) =
                entries.into_iter().partition(|e| e.file_type().is_dir());

            // Process directories first (sequential)
            for entry in dirs {
                let entry_name = entry.file_name().to_string_lossy().to_string();
                let new_rel_path = rel_path.join(&entry_name);

                match self.scan_directory(entry.path(), &new_rel_path) {
                    Ok(dir_node) => contents.push(Node::Directory(dir_node)),
                    Err(e) => eprintln!(
                        "Error processing directory {}: {}",
                        entry.path().display(),
                        e
                    ),
                }
            }

            // Process files in parallel
            let file_nodes: Vec<Node> = files
                .par_iter()
                .filter_map(|entry| {
                    let entry_name = entry.file_name().to_string_lossy().to_string();
                    let new_rel_path = rel_path.join(&entry_name);

                    match self.process_file(entry.path(), &new_rel_path) {
                        Ok(node) => Some(node),
                        Err(e) => {
                            eprintln!("Error processing {}: {}", entry.path().display(), e);
                            None
                        }
                    }
                })
                .collect();

            contents.extend(file_nodes);
        }

        Ok(DirectoryNode {
            name: abs_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            path: rel_path.to_path_buf(),
            metadata,
            contents,
        })
    }

    /// Process a single file and return its node representation
    fn process_file(&self, abs_path: &Path, rel_path: &Path) -> io::Result<Node> {
        self.progress.inc(1);

        // Update progress message to show current file
        let file_name = abs_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Update the progress message with the filename
        // Truncate if too long to avoid display issues
        let display_name = if file_name.len() > 40 {
            format!("...{}", &file_name[file_name.len().saturating_sub(37)..])
        } else {
            file_name.clone()
        };

        // Enhance display with repository context if applicable
        let progress_message = if let Some(repo_info) = &self.config.git_repo {
            format!(
                "Current file: {}/{}/{}",
                repo_info.owner, repo_info.name, display_name
            )
        } else {
            format!("Current file: {}", display_name)
        };

        self.progress.set_message(progress_message);

        let file_type = self.get_file_type(abs_path)?;
        let metadata = self.get_metadata(abs_path)?;

        // Use the normalized path for reporting
        let file_path = if let Some(repo_info) = &self.config.git_repo {
            // For repositories, use the format owner/repo/path
            format!(
                "{}/{}/{}",
                repo_info.owner,
                repo_info.name,
                rel_path.display()
            )
        } else {
            // For local directories, use the relative path as is
            rel_path.to_string_lossy().to_string()
        };

        match file_type {
            FileType::TextFile => {
                let content = self.read_file_content(abs_path)?;
                Ok(Node::File(FileNode {
                    name: file_name,
                    path: rel_path.to_path_buf(),
                    metadata,
                    content,
                }))
            }
            FileType::BinaryFile => {
                // Update statistics for binary files
                {
                    let mut stats = self.statistics.lock().unwrap();
                    stats.files_processed += 1;
                    stats.file_details.insert(
                        file_path,
                        FileReportInfo {
                            lines: 0,
                            chars: 0,
                            tokens: None,
                        },
                    );
                }

                Ok(Node::Binary(BinaryNode {
                    name: file_name,
                    path: rel_path.to_path_buf(),
                    metadata,
                }))
            }
            FileType::Symlink => {
                let target = fs::read_link(abs_path)?.to_string_lossy().to_string();

                // Update statistics for symlinks
                {
                    let mut stats = self.statistics.lock().unwrap();
                    stats.files_processed += 1;
                    stats.file_details.insert(
                        file_path,
                        FileReportInfo {
                            lines: 0,
                            chars: target.chars().count(),
                            tokens: None,
                        },
                    );
                }

                Ok(Node::Symlink(SymlinkNode {
                    name: file_name,
                    path: rel_path.to_path_buf(),
                    metadata,
                    target,
                }))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Unexpected file type for {}", abs_path.display()),
            )),
        }
    }

    /// Check if a file should be ignored based on patterns and defaults
    pub fn should_ignore(&self, path: &Path) -> bool {
        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        // Check custom ignore patterns
        for pattern in &self.config.ignore_patterns {
            if glob_match(pattern, &file_name) {
                return true;
            }
        }

        // Check default ignore patterns
        if DEFAULT_IGNORE.iter().any(|&p| p == file_name) {
            return true;
        }

        // Don't process the output file itself
        if path.ends_with(&self.config.output_file) {
            return true;
        }

        false
    }

    /// Check if a file should be included based on patterns
    pub fn should_include(&self, path: &Path) -> bool {
        // If no include patterns, include everything
        if self.config.include_patterns.is_empty() {
            return true;
        }

        let file_name = path.file_name().unwrap_or_default().to_string_lossy();

        // Check against include patterns
        for pattern in &self.config.include_patterns {
            if glob_match(pattern, &file_name) {
                return true;
            }
        }

        false
    }

    /// Determine the type of a file
    fn get_file_type(&self, path: &Path) -> io::Result<FileType> {
        let metadata = fs::metadata(path)?;

        if metadata.is_dir() {
            return Ok(FileType::Directory);
        }

        if metadata.file_type().is_symlink() {
            return Ok(FileType::Symlink);
        }

        if metadata.is_file() {
            // For smaller files, try to detect if they're text
            if metadata.len() < 8_000_000 {
                // Read a sample of the file to determine type
                let mut buffer = vec![0; std::cmp::min(8192, metadata.len() as usize)];
                if !buffer.is_empty() {
                    let mut file = File::open(path)?;
                    let bytes_read = file.read(&mut buffer)?;
                    buffer.truncate(bytes_read);

                    // Simple heuristic for text files: check for valid UTF-8 and high text-to-binary ratio
                    if String::from_utf8(buffer.clone()).is_ok() {
                        // Count binary characters (0x00-0x08, 0x0E-0x1F)
                        let binary_count = buffer
                            .iter()
                            .filter(|&&b| (b < 9) || (b > 13 && b < 32))
                            .count();
                        let binary_ratio = binary_count as f32 / buffer.len() as f32;

                        if binary_ratio < 0.1 {
                            return Ok(FileType::TextFile);
                        }
                    }
                }
            }

            // Default to binary for any non-text file
            return Ok(FileType::BinaryFile);
        }

        Ok(FileType::Other)
    }

    /// Extract metadata from a file
    fn get_metadata(&self, path: &Path) -> io::Result<Metadata> {
        let fs_metadata = fs::metadata(path)?;

        Ok(Metadata {
            size: fs_metadata.len(),
            modified: fs_metadata.modified()?,
            permissions: format!("{:o}", fs_metadata.permissions().mode() & 0o777),
        })
    }

    /// Read the content of a text file and update statistics
    fn read_file_content(&self, path: &Path) -> io::Result<Option<String>> {
        let metadata = fs::metadata(path)?;
        // Get the normalized path for reporting
        let file_path = self.get_normalized_path_for_reporting(path);

        // Skip large files
        if metadata.len() > 1_048_576 {
            // 1MB limit
            let message = format!(
                "File too large to include content. Size: {}",
                format_file_size(metadata.len())
            );

            // Still update statistics for skipped files
            {
                let mut stats = self.statistics.lock().unwrap();
                stats.files_processed += 1;
                stats.file_details.insert(
                    file_path,
                    FileReportInfo {
                        lines: 0,
                        chars: 0,
                        tokens: None,
                    },
                );
            }

            return Ok(Some(message));
        }

        // Read file content
        let mut content = String::new();
        match File::open(path) {
            Ok(file) => {
                let mut line_count = 0;
                let mut char_count = 0;

                // Count lines and chars
                let reader = BufReader::new(&file);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            line_count += 1;
                            char_count += line.chars().count();
                            // Add newline char that's stripped by lines() iterator
                            char_count += 1;
                        }
                        Err(_) => break,
                    }
                }

                // Re-read file for content
                let mut file = File::open(path)?;
                if let Err(e) = file.read_to_string(&mut content) {
                    return Ok(Some(format!("Failed to read file content: {}", e)));
                }

                // Count tokens if tokenizer is enabled
                let token_count = if let Some(tokenizer) = &self.tokenizer {
                    match tokenizer.count_tokens(&content) {
                        Ok(count) => Some(count.tokens),
                        Err(e) => {
                            eprintln!("Error counting tokens for {}: {}", path.display(), e);
                            None
                        }
                    }
                } else {
                    None
                };

                // Update statistics
                {
                    let mut stats = self.statistics.lock().unwrap();
                    stats.files_processed += 1;
                    stats.total_lines += line_count;
                    stats.total_chars += char_count;

                    // Update token count if available
                    if let Some(tokens) = token_count {
                        stats.total_tokens = Some(stats.total_tokens.unwrap_or(0) + tokens);
                    }

                    stats.file_details.insert(
                        file_path,
                        FileReportInfo {
                            lines: line_count,
                            chars: char_count,
                            tokens: token_count,
                        },
                    );
                }
            }
            Err(e) => {
                return Ok(Some(format!("Failed to open file: {}", e)));
            }
        }

        Ok(Some(content))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use indicatif::ProgressBar;

    use crate::config::{Config, GitCachePolicy};
    use crate::git::{GitHost, GitRepoInfo};
    use crate::scanner::Scanner;

    #[test]
    fn test_normalize_path() {
        // Create a test config with a mock Git repository
        let repo_path = PathBuf::from("/tmp/cache/dumpfs/github/username/repo");
        let git_repo = GitRepoInfo {
            url: "https://github.com/username/repo".to_string(),
            host: GitHost::GitHub,
            owner: "username".to_string(),
            name: "repo".to_string(),
            cache_path: repo_path.clone(),
        };

        let config = Config {
            target_dir: repo_path.clone(),
            output_file: PathBuf::from("output.xml"),
            ignore_patterns: vec![],
            include_patterns: vec![],
            num_threads: 1,
            respect_gitignore: false,
            gitignore_path: None,
            model: None,
            repo_url: Some("https://github.com/username/repo".to_string()),
            git_repo: Some(git_repo),
            git_cache_policy: GitCachePolicy::AlwaysPull,
        };

        let scanner = Scanner::new(config, Arc::new(ProgressBar::hidden()));

        // Test paths at various depths
        let test_cases = vec![
            // Path in repo root should normalize to empty path or just filename
            (repo_path.join("file.txt"), PathBuf::from("file.txt")),
            // Path in subdirectory should be relative to repo root
            (
                repo_path.join("src").join("main.rs"),
                PathBuf::from("src/main.rs"),
            ),
            // Path outside repo shouldn't change
            (
                PathBuf::from("/other/path/file.txt"),
                PathBuf::from("/other/path/file.txt"),
            ),
        ];

        for (input, expected) in test_cases {
            let normalized = scanner.normalize_path(&input);
            assert_eq!(normalized, expected);
        }
    }

    #[test]
    fn test_get_normalized_path_for_reporting() {
        // Create a test config with a mock Git repository
        let repo_path = PathBuf::from("/tmp/cache/dumpfs/github/username/repo");
        let git_repo = GitRepoInfo {
            url: "https://github.com/username/repo".to_string(),
            host: GitHost::GitHub,
            owner: "username".to_string(),
            name: "repo".to_string(),
            cache_path: repo_path.clone(),
        };

        let config = Config {
            target_dir: repo_path.clone(),
            output_file: PathBuf::from("output.xml"),
            ignore_patterns: vec![],
            include_patterns: vec![],
            num_threads: 1,
            respect_gitignore: false,
            gitignore_path: None,
            model: None,
            repo_url: Some("https://github.com/username/repo".to_string()),
            git_repo: Some(git_repo),
            git_cache_policy: GitCachePolicy::AlwaysPull,
        };

        let scanner = Scanner::new(config, Arc::new(ProgressBar::hidden()));

        // Test path formatting for different types of paths
        let root_path = repo_path.clone();
        let src_path = repo_path.join("src").join("main.rs");

        // Repository root should show as "username/repo"
        let root_display = scanner.get_normalized_path_for_reporting(&root_path);
        assert_eq!(root_display, "username/repo");

        // File in repo should show as "username/repo/src/main.rs"
        let src_display = scanner.get_normalized_path_for_reporting(&src_path);
        assert_eq!(src_display, "username/repo/src/main.rs");

        // Path outside repo should just use the full path
        let other_path = PathBuf::from("/other/path/file.txt");
        let other_display = scanner.get_normalized_path_for_reporting(&other_path);
        assert_eq!(other_display, "/other/path/file.txt");
    }
}
