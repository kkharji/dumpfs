/*!
 * Core types and data structures for the DumpFS application
 */

use std::path::PathBuf;
use std::time::SystemTime;

/// Represents different types of filesystem entries
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    /// Text file with readable content
    TextFile,
    /// Binary file (non-text)
    BinaryFile,
    /// Symbolic link to another file
    Symlink,
    /// Directory containing other entries
    Directory,
    /// Other file types
    Other,
}

/// Metadata about a filesystem entry
#[derive(Debug, Clone)]
pub struct Metadata {
    /// Size in bytes
    pub size: u64,
    /// Last modification time
    pub modified: SystemTime,
    /// File permissions in octal format
    pub permissions: String,
}

/// Represents a directory in the file system
#[derive(Debug, Clone)]
pub struct DirectoryNode {
    /// Directory name
    pub name: String,
    /// Relative path from scan root
    pub path: PathBuf,
    /// Directory metadata
    pub metadata: Metadata,
    /// Directory contents
    pub contents: Vec<Node>,
}

/// Represents a text file
#[derive(Debug, Clone)]
pub struct FileNode {
    /// File name
    pub name: String,
    /// Relative path from scan root
    pub path: PathBuf,
    /// File metadata
    pub metadata: Metadata,
    /// File content (may be None if too large)
    pub content: Option<String>,
}

/// Represents a binary file
#[derive(Debug, Clone)]
pub struct BinaryNode {
    /// File name
    pub name: String,
    /// Relative path from scan root
    pub path: PathBuf,
    /// File metadata
    pub metadata: Metadata,
}

/// Represents a symbolic link
#[derive(Debug, Clone)]
pub struct SymlinkNode {
    /// Link name
    pub name: String,
    /// Relative path from scan root
    pub path: PathBuf,
    /// Link metadata
    pub metadata: Metadata,
    /// Target of the symlink
    pub target: String,
}

/// A generic filesystem node
#[derive(Debug, Clone)]
pub enum Node {
    /// Directory node
    Directory(DirectoryNode),
    /// Text file node
    File(FileNode),
    /// Binary file node
    Binary(BinaryNode),
    /// Symbolic link node
    Symlink(SymlinkNode),
}
