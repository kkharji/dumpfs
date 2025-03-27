/*!
 * Command-line interface for DumpFS
 */

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use dumpfs::error::{DumpFsError, Result};

use clap::{CommandFactory, Parser};
use clap_complete::{generate, CompleteEnv, Shell};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::ThreadPoolBuilder;

use dumpfs::config::{Args, Config, GitCachePolicy};
use dumpfs::git::{self, GitRepoInfo};
use dumpfs::report::{ReportFormat, Reporter, ScanReport};
use dumpfs::scanner::Scanner;
use dumpfs::utils::count_files;
use dumpfs::writer::XmlWriter;

/// Generate shell completions
fn print_completions(generator: Shell, cmd: &mut clap::Command) {
    generate(
        generator,
        cmd,
        cmd.get_name().to_string(),
        &mut io::stdout(),
    );
}

/// Process a path that could be a local directory or a Git repository URL
///
/// Arguments:
/// - path: The path or URL to process
/// - git_cache_policy: Policy for handling Git repository caching
/// - progress: Optional progress bar for reporting (will create one if None)
///
/// Returns:
/// - The target directory path (local or cloned repo)
/// - Optional repository information if it's a Git URL
fn process_path(
    path: &str,
    git_cache_policy: GitCachePolicy,
    progress: Option<&ProgressBar>,
) -> Result<(PathBuf, Option<String>, Option<GitRepoInfo>)> {
    // If not a Git URL, just return the path as is
    if !git::is_git_url(path) {
        return Ok((PathBuf::from(path), None, None));
    }

    // Parse the Git URL
    let repo_info = git::parse_git_url(path)
        .map_err(|e| DumpFsError::InvalidArgument(format!("Invalid Git URL: {}", e)))?;

    // Create a progress reporter adapter
    struct ProgressBarAdapter<'a> {
        bar: &'a ProgressBar,
        repo_info: &'a GitRepoInfo,
        is_clone: bool,
    }

    impl<'a> git::ProgressReporter for ProgressBarAdapter<'a> {
        fn report(&self, git_progress: &git::GitProgress) {
            let percent = git_progress.percentage();

            // Format progress message
            let action = if self.is_clone { "Cloning" } else { "Updating" };
            let msg = format!(
                "{} {}/{} ({}/{}), {} downloaded",
                action,
                self.repo_info.owner,
                self.repo_info.name,
                git_progress.received_objects,
                git_progress.total_objects,
                git_progress.formatted_bytes()
            );

            self.bar.set_message(msg);
            self.bar.set_position(percent as u64);
        }
    }

    // Use the provided progress bar or create a new one
    let progress_bar = match progress {
        Some(p) => p,
        None => {
            // Create a new progress bar if none is provided
            let new_bar = ProgressBar::new(100);
            new_bar.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} {prefix:.bold.cyan} {msg} [{bar:40.cyan/blue}] {percent}%")
                    .map_err(|e| DumpFsError::Unexpected(format!("Failed to create progress bar style: {}", e)))?
            );

            // Since this is a temporary variable, we'll just leak it to avoid ownership issues
            Box::leak(Box::new(new_bar))
        }
    };

    // Check if repository already exists
    let repo_exists = git::Repository::exists(&repo_info);

    // Handle based on policy
    match (git_cache_policy, repo_exists) {
        // Repository doesn't exist, always clone
        (_, false) => {
            progress_bar.set_prefix("🔄 Cloning");
            progress_bar.set_message(format!(
                "Cloning repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            let reporter = ProgressBarAdapter {
                bar: progress_bar,
                repo_info: &repo_info,
                is_clone: true,
            };

            let repo = match git::Repository::clone(repo_info.clone(), Some(&reporter)) {
                Ok(repo) => {
                    progress_bar.finish_with_message(format!(
                        "Repository cloned: {}/{}",
                        repo_info.owner, repo_info.name
                    ));
                    repo
                }
                Err(e) => {
                    progress_bar.abandon_with_message(format!("Failed to clone repository: {}", e));
                    return Err(DumpFsError::Git(e));
                }
            };

            Ok((repo.path().clone(), Some(path.to_string()), Some(repo_info)))
        }

        // Force clone even if exists
        (GitCachePolicy::ForceClone, true) => {
            // Delete existing repo
            progress_bar.set_prefix("🗑️ Removing");
            progress_bar.set_message(format!(
                "Removing existing repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            // Remove the directory to force a fresh clone
            if let Err(e) = std::fs::remove_dir_all(&repo_info.cache_path) {
                progress_bar
                    .abandon_with_message(format!("Failed to remove existing repository: {}", e));
                return Err(DumpFsError::Io(e));
            }

            // Clone the repository
            progress_bar.set_prefix("🔄 Cloning");
            progress_bar.set_message(format!(
                "Cloning repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            let reporter = ProgressBarAdapter {
                bar: progress_bar,
                repo_info: &repo_info,
                is_clone: true,
            };

            let repo = match git::Repository::clone(repo_info.clone(), Some(&reporter)) {
                Ok(repo) => {
                    progress_bar.finish_with_message(format!(
                        "Repository cloned: {}/{}",
                        repo_info.owner, repo_info.name
                    ));
                    repo
                }
                Err(e) => {
                    progress_bar.abandon_with_message(format!("Failed to clone repository: {}", e));
                    return Err(DumpFsError::Git(e));
                }
            };

            Ok((repo.path().clone(), Some(path.to_string()), Some(repo_info)))
        }

        // Pull if exists
        (GitCachePolicy::AlwaysPull, true) => {
            progress_bar.set_prefix("🔄 Updating");
            progress_bar.set_message(format!(
                "Updating repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            let reporter = ProgressBarAdapter {
                bar: progress_bar,
                repo_info: &repo_info,
                is_clone: false,
            };

            let mut repo = match git::Repository::open(repo_info.clone()) {
                Ok(repo) => repo,
                Err(e) => {
                    progress_bar.abandon_with_message(format!("Failed to open repository: {}", e));
                    return Err(DumpFsError::Git(e));
                }
            };

            if let Err(e) = repo.pull(Some(&reporter)) {
                progress_bar.abandon_with_message(format!("Failed to update repository: {}", e));
                return Err(DumpFsError::Git(e));
            }

            progress_bar.finish_with_message(format!(
                "Repository updated: {}/{}",
                repo_info.owner, repo_info.name
            ));

            Ok((repo.path().clone(), Some(path.to_string()), Some(repo_info)))
        }

        // Use cache without pulling
        (GitCachePolicy::UseCache, true) => {
            progress_bar.set_prefix("📂 Using cached");
            progress_bar.set_message(format!(
                "Using cached repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            progress_bar.finish_with_message(format!(
                "Using cached repository: {}/{}",
                repo_info.owner, repo_info.name
            ));

            Ok((
                repo_info.cache_path.clone(),
                Some(path.to_string()),
                Some(repo_info),
            ))
        }
    }
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
    let (processed_path, repo_url, git_repo) = match process_path(
        &args.directory_path,
        config.git_cache_policy,
        Some(&progress),
    ) {
        Ok(result) => result,
        Err(e) => {
            progress.abandon_with_message(format!("Error processing path: {}", e));
            eprintln!("Error processing path: {}", e);
            return Err(e);
        }
    };

    // Update config with processed path and repo info
    config.target_dir = processed_path;
    config.repo_url = repo_url;
    config.git_repo = git_repo;

    // Adjust output file location for git repositories
    if let Some(repo) = &config.git_repo {
        // Check if output file is a relative path with no directory component
        let output_path = PathBuf::from(&args.output_file);
        if !output_path.is_absolute()
            && (output_path.parent().is_none()
                || output_path
                    .parent()
                    .expect("Parent should be Some if not None")
                    == Path::new(""))
        {
            // Use the repository directory for the output file
            config.output_file = repo.cache_path.join(output_path);
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

    Ok(())
}
