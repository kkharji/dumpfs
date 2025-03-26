/*!
 * Command-line interface for DumpFS
 */

use std::io;
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::ThreadPoolBuilder;

use dumpfs::config::{Args, Config};
use dumpfs::scanner::Scanner;
use dumpfs::utils::count_files;
use dumpfs::writer::XmlWriter;

fn main() -> io::Result<()> {
    // Parse command line arguments
    let args = Args::parse();
    
    // Create configuration
    let config = Config::from_args(args);
    
    // Validate configuration
    config.validate()?;
    
    // Configure thread pool
    if let Err(e) = ThreadPoolBuilder::new()
        .num_threads(config.num_threads)
        .build_global() {
        eprintln!("Warning: Failed to set thread pool size: {}", e);
    }
    
    println!("Scanning directory: {}", config.target_dir.display());
    
    // Count files for progress tracking
    let total_files = match count_files(&config.target_dir, &config) {
        Ok(count) => {
            println!("Found {} files to process", count);
            count
        },
        Err(e) => {
            eprintln!("Warning: Failed to count files: {}", e);
            0
        }
    };
    
    // Create progress bar
    let progress = ProgressBar::new(total_files);
    progress.set_style(ProgressStyle::default_bar()
        .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars("#>-"));
    
    // Create scanner and writer
    let scanner = Scanner::new(config.clone(), Arc::new(progress.clone()));
    let writer = XmlWriter::new(config.clone());
    
    // Scan directory
    let start_time = Instant::now();
    let root_node = scanner.scan()?;
    
    // Write XML output
    writer.write(&root_node)?;
    
    // Finish progress
    progress.finish_with_message(format!(
        "Directory content extracted to {} in {:.2?}",
        config.output_file.display(),
        start_time.elapsed()
    ));
    
    Ok(())
}
