/*!
 * DumpFS - Generate XML representation of directory contents for LLM context
 *
 * This library creates structured XML representations of directory contents
 * for use as context for Large Language Models.
 */

pub mod clipboard;
pub mod config;
pub mod error;
pub mod git;
pub mod report;
pub mod scanner;
pub mod tokenizer;
pub mod types;
pub mod utils;
pub mod writer;

#[cfg(test)]
mod tests;

// Re-export main components for easier access
pub use clipboard::{copy_to_clipboard, ClipboardError};
pub use config::Config;
pub use error::{DumpFsError, Result, ResultExt};
pub use report::{FileReportInfo, ReportFormat, Reporter, ScanReport};
pub use scanner::Scanner;
pub use types::{BinaryNode, DirectoryNode, FileNode, FileType, Metadata, Node, SymlinkNode};
pub use utils::{count_files, format_file_size};
pub use writer::FsWriterFormatter;

// No process_path export needed

/// Version of the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
