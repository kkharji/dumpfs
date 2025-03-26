/*!
 * DumpFS - Generate XML representation of directory contents for LLM context
 *
 * This library creates structured XML representations of directory contents
 * for use as context for Large Language Models.
 */

pub mod config;
pub mod scanner;
pub mod types;
pub mod utils;
pub mod writer;

#[cfg(test)]
mod tests;

// Re-export main components for easier access
pub use config::Config;
pub use scanner::Scanner;
pub use types::{Node, DirectoryNode, FileNode, BinaryNode, SymlinkNode, FileType, Metadata};
pub use utils::{count_files, format_file_size};
pub use writer::XmlWriter;

/// Version of the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
