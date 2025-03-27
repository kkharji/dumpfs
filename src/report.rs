/*!
 * Reporting functionality for DumpFS
 *
 * Provides functionality for generating formatted reports of scan results
 * using the tabled library for clean, consistent table rendering.
 */

use std::collections::HashMap;
use std::time::Duration;

use tabled::{
    settings::{object::Columns, Alignment, Modify, Padding, Style},
    Table, Tabled,
};

/// Information about a file in the report
#[derive(Debug, Clone, Default)]
pub struct FileReportInfo {
    /// Number of lines in the file
    pub lines: usize,
    /// Number of characters in the file
    pub chars: usize,
    /// Number of tokens in the file (if tokenizer is enabled)
    pub tokens: Option<usize>,
}

/// Statistics for a directory scan
#[derive(Debug, Clone)]
pub struct ScanReport {
    /// Output file path
    pub output_file: String,
    /// Time taken to scan
    pub duration: Duration,
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

/// Format of the report output
pub enum ReportFormat {
    /// Console table output
    ConsoleTable,
    // Other formats could be added in the future
    // JSON, HTML, etc.
}

/// Report generator for scan results
pub struct Reporter {
    format: ReportFormat,
}

impl Reporter {
    /// Create a new reporter
    pub fn new(format: ReportFormat) -> Self {
        Self { format }
    }

    /// Format a number with human-readable units
    fn format_number(&self, num: usize) -> String {
        if num >= 1_000_000 {
            format!("{:.1}M", num as f64 / 1_000_000.0)
        } else if num >= 1_000 {
            format!("{:.1}K", num as f64 / 1_000.0)
        } else {
            num.to_string()
        }
    }

    /// Generate a report string based on scan statistics
    pub fn generate_report(&self, report: &ScanReport) -> String {
        match self.format {
            ReportFormat::ConsoleTable => self.generate_console_report(report),
            // Additional formats could be added here
        }
    }

    /// Print the report to stdout
    pub fn print_report(&self, report: &ScanReport) {
        println!("\n{}", self.generate_report(report));
    }

    // Format path to be relative and handle truncation if needed
    fn format_path(&self, path: &str, max_len: usize) -> String {
        // Strip leading paths to show only project-relative path
        let parts: Vec<&str> = path.split('/').collect();

        // If the path contains "projs/dumpfs", extract everything after that
        let mut rel_path = path.to_string();
        if let Some(pos) = path.find("projs/dumpfs") {
            if let Some(p) = path.get(pos + "projs/dumpfs".len() + 1..) {
                rel_path = p.to_string();
            }
        }

        // If relative path is empty, use the original filename
        if rel_path.is_empty() && !parts.is_empty() {
            rel_path = parts.last().unwrap_or(&"").to_string();
        }

        // Truncate if too long
        if rel_path.len() <= max_len {
            return rel_path;
        }

        // If too long, preserve the most meaningful part (filename and parent dirs)
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() <= 2 {
            return format!("...{}", &path[path.len().saturating_sub(max_len - 3)..]);
        }

        // Keep the last few segments
        let mut result = String::new();
        let mut current_len = 3; // Start with "..."
        let mut segments = Vec::new();

        for part in parts.iter().rev() {
            let part_len = part.len() + 1; // +1 for '/'
            if current_len + part_len <= max_len {
                segments.push(*part);
                current_len += part_len;
            } else {
                break;
            }
        }

        result.push_str("...");
        for part in segments.iter().rev() {
            result.push('/');
            result.push_str(part);
        }

        result
    }

    // Create a summary table using the tabled crate
    fn create_summary_table(&self, report: &ScanReport) -> String {
        // Define the summary table data structure
        #[derive(Tabled)]
        struct SummaryRow {
            #[tabled(rename = "Metric")]
            key: String,

            #[tabled(rename = "Value")]
            value: String,
        }

        let mut rows = Vec::new();

        // Add rows to the summary table
        rows.push(SummaryRow {
            key: "📂 Output File".to_string(),
            value: report.output_file.clone(),
        });

        rows.push(SummaryRow {
            key: "⏱️ Process Time".to_string(),
            value: format!("{:.4?}", report.duration),
        });

        rows.push(SummaryRow {
            key: "📄 Files Processed".to_string(),
            value: self.format_number(report.files_processed),
        });

        rows.push(SummaryRow {
            key: "📝 Total Lines".to_string(),
            value: self.format_number(report.total_lines),
        });

        // Use actual token count if available, otherwise use estimate
        let token_text = if let Some(tokens) = report.total_tokens {
            format!("{} tokens (counted)", self.format_number(tokens))
        } else {
            let estimated_tokens = report.total_chars / 4;
            format!(
                "{} tokens (estimated)",
                self.format_number(estimated_tokens)
            )
        };

        rows.push(SummaryRow {
            key: "📦 LLM Tokens".to_string(),
            value: token_text,
        });

        // Add cache statistics if available
        if let (Some(hits), Some(misses)) = (report.token_cache_hits, report.token_cache_misses) {
            let total = hits + misses;
            let hit_rate = if total > 0 {
                format!("{:.1}%", (hits as f64 / total as f64) * 100.0)
            } else {
                "0.0%".to_string()
            };

            rows.push(SummaryRow {
                key: "🔄 Cache Hit Rate".to_string(),
                value: format!("{} ({} hits / {} total)", hit_rate, hits, total),
            });
        }

        // Create and style the table
        let mut table = Table::new(rows);
        table
            .with(Style::rounded())
            .with(Padding::new(1, 1, 0, 0))
            .with(Modify::new(Columns::new(..)).with(Alignment::left()));

        table.to_string()
    }

    // Create a files table using the tabled crate
    fn create_files_table(&self, report: &ScanReport) -> String {
        // Define the files table data structure
        #[derive(Tabled)]
        struct FileRow {
            #[tabled(rename = "File Path")]
            path: String,

            #[tabled(rename = "Lines")]
            lines: String,

            #[tabled(rename = "Est. Tokens")]
            tokens: String,
        }

        // Sort files by character count
        let mut files: Vec<_> = report.file_details.iter().collect();
        files.sort_by(|(_, a), (_, b)| b.chars.cmp(&a.chars));

        // Determine if we show all files or just top 10
        let files_to_show = if report.file_details.len() > 15 {
            &files[0..10]
        } else {
            &files[..]
        };

        // Generate rows for the table
        let rows: Vec<FileRow> = files_to_show
            .iter()
            .map(|(path, info)| {
                // Format and truncate path if needed
                let display_path = self.format_path(path, 60);

                // Use actual token count if available, otherwise estimate
                let token_count = if let Some(tokens) = info.tokens {
                    self.format_number(tokens)
                } else {
                    let estimated_tokens = info.chars / 4;
                    self.format_number(estimated_tokens)
                };

                FileRow {
                    path: display_path,
                    lines: self.format_number(info.lines),
                    tokens: token_count,
                }
            })
            .collect();

        // Create and style the table
        let mut table = Table::new(rows);
        table
            .with(Style::rounded())
            .with(Padding::new(1, 1, 0, 0))
            .with(Modify::new(Columns::new(..)).with(Alignment::left()));

        table.to_string()
    }

    // Generate a console table report
    fn generate_console_report(&self, report: &ScanReport) -> String {
        // Generate summary and files tables
        let summary_table = self.create_summary_table(report);
        let files_table = self.create_files_table(report);

        // Create proper section titles
        let summary_title = "✅  EXTRACTION COMPLETE";
        let files_title = if report.file_details.len() > 15 {
            "📋  TOP 10 LARGEST FILES BY CHARACTER COUNT  📋"
        } else {
            "📋  PROCESSED FILES"
        };

        // Combine them with appropriate spacing and titles, but put files first
        format!(
            "{}\n{}\n\n{}\n{}",
            files_title, files_table, summary_title, summary_table
        )
    }
}
