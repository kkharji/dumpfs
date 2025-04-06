/*!
 * Command-line interface for DumpFS
 */

use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use dumpfs::clipboard;
use dumpfs::error::{DumpFsError, Result};

use clap::{CommandFactory, Parser};
use clap_complete::{generate, CompleteEnv, Shell};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::ThreadPoolBuilder;

use dumpfs::config::{Args, Config};
use dumpfs::git;
use dumpfs::report::{ReportFormat, Reporter, ScanReport};
use dumpfs::scanner::Scanner;
use dumpfs::utils::count_files;

/// Generate shell completions
fn print_completions(generator: Shell, cmd: &mut clap::Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}

fn main() -> Result<()> {
    // Enable automatic shell completion
    CompleteEnv::with_factory(Args::command).complete();

    // Parse command line arguments
    let args = Args::parse();

    // Handle completions if requested
    if let Some(generator) = args.generate {
        let mut cmd = Args::command();
        eprintln!("Generating completion file for {generator:?}...");
        print_completions(generator, &mut cmd);
        return Ok(());
    }

    // Handle cache cleaning if requested
    if let Some(days) = args.clean_cache {
        eprintln!(
            "Cleaning Git repository cache (older than {} days)...",
            days
        );
        match git::clean_cache(days) {
            Ok(count) => {
                eprintln!("Removed {} repositories from cache", count);
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error cleaning cache: {}", e);
                return Err(DumpFsError::Io(e));
            }
        }
    }

    // Create progress bar with advanced Unicode styling
    let progress = ProgressBar::new(0);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} {prefix:.bold.cyan} {wide_msg:.dim.white} {pos}/{len} ({percent}%) ⏱️  Elapsed: {elapsed_precise}  Remaining: {eta_precise}  Speed: {per_sec}/s")
            .map_err(|e| DumpFsError::Unexpected(format!("Failed to create progress style: {}", e)))?
    );
    progress.enable_steady_tick(std::time::Duration::from_millis(100));
    progress.set_prefix("📊 Setup");

    // Create initial configuration
    let mut config = Config::from_args(args.clone());

    // Process path (either local directory or git repository URL)
    progress.set_message(format!("Processing path: {}", args.directory_path));
    let (processed_path, repo_url, git_repo) = match git::process_path(
        &args.directory_path,
        config.git_cache_policy,
        Some(&progress),
    ) {
        Ok(result) => result,
        Err(e) => {
            progress.abandon_with_message(format!("Error processing path: {}", e));
            eprintln!("Error processing path: {}", e);
            return Err(e.into());
        }
    };

    // Update config with processed path and repo info
    config.target_dir = processed_path;
    config.repo_url = repo_url;
    config.git_repo = git_repo;

    // Adjust output file location for git repositories
    if let Some(repo) = &config.git_repo {
        // Check if output file is a relative path with no directory component
        if !config.output_file.is_absolute()
            && (config.output_file.parent().is_none()
                || config
                    .output_file
                    .parent()
                    .expect("Parent should be Some if not None")
                    == Path::new(""))
        {
            // Use the repository directory for the output file
            config.output_file = repo.cache_path.join(config.output_file);
        }
    }

    // Validate configuration
    config.validate()?;

    // Configure thread pool
    if let Err(e) = ThreadPoolBuilder::new()
        .num_threads(config.num_threads)
        .build_global()
    {
        eprintln!("Warning: Failed to set thread pool size: {}", e);
    }

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

    // Start timing both scan and write operations
    let start_time = Instant::now();

    // Scan directory
    let root_node = scanner.scan()?;

    // Write XML output
    config.format.write(config.clone(), &root_node)?;

    // Calculate total duration (scan + write)
    let total_duration = start_time.elapsed();

    // Clear the progress bar
    progress.finish_and_clear();

    // Get scanner statistics
    let scanner_stats = scanner.get_statistics()?;

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

    // Handle clipboard functionality if --clip is specified
    if config.clip || config.stdout {
        // Get the output file content
        let output_content = std::fs::read_to_string(&config.output_file)?;
        if config.stdout {
            std::io::stdout().write_all(output_content.as_bytes())?;
        }

        if config.clip {
            // Copy to clipboard
            match clipboard::copy_to_clipboard(&output_content) {
                Ok(_) => {
                    eprintln!("✅ Output copied to clipboard successfully");
                }
                Err(e) => {
                    eprintln!("❌ Failed to copy to clipboard: {}", e);
                    // Don't return error as the main functionality (file generation) succeeded
                }
            }
        }
    }

    Ok(())
}
