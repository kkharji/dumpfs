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
use dumpfs::report::{ReportFormat, Reporter, ScanReport};
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
        .build_global()
    {
        eprintln!("Warning: Failed to set thread pool size: {}", e);
    }

    // Create progress bar with advanced Unicode styling
    let progress = ProgressBar::new(0);
    progress.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} {prefix:.bold.cyan} {wide_msg:.dim.white} {pos}/{len} ({percent}%) ⏱️  Elapsed: {elapsed_precise}  Remaining: {eta_precise}  Speed: {per_sec}/s")
        .unwrap());
    progress.enable_steady_tick(std::time::Duration::from_millis(100));
    progress.set_prefix("📊 Setup");

    progress.set_message(format!(
        "📂 Scanning directory: {}",
        config.target_dir.display()
    ));

    // Add gitignore status message
    if config.respect_gitignore {
        progress.set_message(match &config.gitignore_path {
            Some(path) => format!("🔍 Using custom gitignore file: {}", path.display()),
            None => "🔍 Respecting .gitignore files in the project".to_string(),
        });
    }

    // Count files for progress tracking
    let total_files = match count_files(&config.target_dir, &config) {
        Ok(count) => {
            progress.set_message(format!("🔎 Found {} files to process", count));
            count
        }
        Err(e) => {
            progress.set_message(format!("⚠️ Warning: Failed to count files: {}", e));
            0
        }
    };

    progress.set_length(total_files);
    progress.set_prefix("📊 Processing");
    progress.set_message("Starting scan...");

    // Create scanner and writer
    let scanner = Scanner::new(config.clone(), Arc::new(progress.clone()));
    let writer = XmlWriter::new(config.clone());

    // Start timing both scan and write operations
    let start_time = Instant::now();

    // Scan directory
    let root_node = scanner.scan()?;

    // Write XML output
    writer.write(&root_node)?;

    // Calculate total duration (scan + write)
    let total_duration = start_time.elapsed();

    // Clear the progress bar
    progress.finish_and_clear();

    // Get scanner statistics
    let scanner_stats = scanner.get_statistics();

    // Prepare the scan report
    let scan_report = ScanReport {
        output_file: config.output_file.display().to_string(),
        duration: total_duration,
        files_processed: scanner_stats.files_processed,
        total_lines: scanner_stats.total_lines,
        total_chars: scanner_stats.total_chars,
        total_tokens: scanner_stats.total_tokens,
        file_details: scanner_stats.file_details,
        token_cache_hits: scanner_stats.token_cache_hits,
        token_cache_misses: scanner_stats.token_cache_misses,
    };

    // Create a reporter and print the report
    let reporter = Reporter::new(ReportFormat::ConsoleTable);
    reporter.print_report(&scan_report);

    Ok(())
}
