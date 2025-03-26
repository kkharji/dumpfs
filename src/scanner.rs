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

/// Scanner statistics
#[derive(Debug, Clone, Default)]
pub struct ScannerStatistics {
    /// Number of files processed
    pub files_processed: usize,
    /// Total number of lines
    pub total_lines: usize,
    /// Total number of characters
    pub total_chars: usize,
    /// Details for each file
    pub file_details: HashMap<String, FileReportInfo>,
}

/// Scanner for directory contents
pub struct Scanner {
    /// Scanner configuration
    config: Config,
    /// Progress bar
    pub progress: Arc<ProgressBar>,
    /// Scanner statistics
    statistics: Arc<Mutex<ScannerStatistics>>,
}

impl Scanner {
    /// Create a new scanner
    pub fn new(config: Config, progress: Arc<ProgressBar>) -> Self {
        Self {
            config,
            progress,
            statistics: Arc::new(Mutex::new(ScannerStatistics::default())),
        }
    }

    /// Get scanner statistics
    pub fn get_statistics(&self) -> ScannerStatistics {
        self.statistics.lock().unwrap().clone()
    }

    /// Scan the target directory and return the directory tree
    pub fn scan(&self) -> io::Result<DirectoryNode> {
        let abs_path = fs::canonicalize(&self.config.target_dir)?;
        let dir_name = abs_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        self.scan_directory(&abs_path, &PathBuf::from(&dir_name))
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
                let entry_name = entry_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let new_rel_path = rel_path.join(&entry_name);

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
        self.progress
            .set_message(format!("Current file: {}", display_name));

        let file_type = self.get_file_type(abs_path)?;
        let metadata = self.get_metadata(abs_path)?;
        // Use the relative path for reporting
        let file_path = rel_path.to_string_lossy().to_string();

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
                    stats
                        .file_details
                        .insert(file_path, FileReportInfo { lines: 0, chars: 0 });
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
        // Get the relative path from the full path
        let file_path = path.to_string_lossy().to_string();

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
                stats
                    .file_details
                    .insert(file_path, FileReportInfo { lines: 0, chars: 0 });
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

                // Update statistics
                {
                    let mut stats = self.statistics.lock().unwrap();
                    stats.files_processed += 1;
                    stats.total_lines += line_count;
                    stats.total_chars += char_count;
                    stats.file_details.insert(
                        file_path,
                        FileReportInfo {
                            lines: line_count,
                            chars: char_count,
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
