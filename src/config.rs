/*!
 * Configuration handling for DumpFS
 */

use std::io;
use std::path::PathBuf;

use clap::Parser;

/// Command-line arguments for DumpFS
#[derive(Parser, Debug)]
#[clap(
    name = "dumpfs",
    version = env!("CARGO_PKG_VERSION"),
    about = "Generate XML representation of directory contents for LLM context",
    long_about = "Creates an XML representation of a directory structure and its contents, designed for providing context to Large Language Models (LLMs)."
)]
pub struct Args {
    /// Target directory to process
    #[clap(default_value = ".")]
    pub directory_path: String,

    /// Output XML file name
    #[clap(default_value = ".content.xml")]
    pub output_file: String,

    /// Comma-separated list of patterns to ignore
    #[clap(long, value_delimiter = ',')]
    pub ignore_patterns: Vec<String>,

    /// Comma-separated list of patterns to include (if specified, only matching files are included)
    #[clap(long, value_delimiter = ',')]
    pub include_patterns: Vec<String>,

    /// Number of threads to use for processing
    #[clap(long, default_value = "4")]
    pub threads: usize,
}

/// Application configuration
#[derive(Clone, Debug)]
pub struct Config {
    /// Target directory to process
    pub target_dir: PathBuf,
    
    /// Output XML file path
    pub output_file: PathBuf,
    
    /// Patterns to ignore
    pub ignore_patterns: Vec<String>,
    
    /// Patterns to include (if empty, include all)
    pub include_patterns: Vec<String>,
    
    /// Number of threads to use for processing
    pub num_threads: usize,
}

impl Config {
    /// Create configuration from command-line arguments
    pub fn from_args(args: Args) -> Self {
        Self {
            target_dir: PathBuf::from(args.directory_path),
            output_file: PathBuf::from(args.output_file),
            ignore_patterns: args.ignore_patterns,
            include_patterns: args.include_patterns,
            num_threads: args.threads,
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> io::Result<()> {
        // Check if target directory exists and is readable
        if !self.target_dir.exists() || !self.target_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Target directory not found: {}", self.target_dir.display())
            ));
        }
        
        // Check if output file directory exists and is writable
        if let Some(parent) = self.output_file.parent() {
            if !parent.exists() && parent != PathBuf::from("") {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Output directory not found: {}", parent.display())
                ));
            }
        }
        
        Ok(())
    }
}
