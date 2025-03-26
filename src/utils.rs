/*!
 * Utility functions for DumpFS
 */

use std::io;
use std::path::Path;
use std::sync::Arc;

use ignore::WalkBuilder;
use indicatif::ProgressBar;
use once_cell::sync::Lazy;
use walkdir::WalkDir;

use crate::config::Config;
use crate::scanner::Scanner;

/// Count total files for progress tracking
pub fn count_files(dir: &Path, config: &Config) -> io::Result<u64> {
    let scanner = Scanner::new(config.clone(), Arc::new(ProgressBar::hidden()));
    let mut count = 0;

    if config.respect_gitignore {
        // Use ignore crate's Walk to handle .gitignore patterns
        let mut walker = WalkBuilder::new(dir);

        // Custom gitignore file if specified
        if let Some(gitignore_path) = &config.gitignore_path {
            walker.add_custom_ignore_filename(gitignore_path);
        }

        for entry in walker.build().filter_map(Result::ok) {
            if entry.file_type().map_or(false, |ft| ft.is_file())
                && !scanner.should_ignore(entry.path())
                && scanner.should_include(entry.path())
            {
                count += 1;
            }
        }
    } else {
        // Use walkdir without gitignore support
        for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file()
                && !scanner.should_ignore(entry.path())
                && scanner.should_include(entry.path())
            {
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Format a human-readable file size
pub fn format_file_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}

/// Default patterns to ignore
pub static DEFAULT_IGNORE: Lazy<Vec<&'static str>> = Lazy::new(|| {
    vec![
        // Version Control
        ".git",
        ".svn",
        ".hg",
        ".bzr",
        ".gitignore",
        ".gitattributes",
        // OS Files
        ".DS_Store",
        "Thumbs.db",
        "desktop.ini",
        "ehthumbs.db",
        "*.lnk",
        "*.url",
        ".directory",
        // Dependencies
        "node_modules",
        "bower_components",
        ".npm",
        "package-lock.json",
        "yarn.lock",
        ".yarn",
        "vendor",
        "composer.lock",
        ".pnpm-store",
        // Build & Dist
        "dist",
        "build",
        "out",
        "bin",
        "release",
        "*.min.js",
        "*.min.css",
        "bundle.*",
        // Python
        "__pycache__",
        ".pytest_cache",
        ".coverage",
        "venv",
        "env",
        ".env",
        ".venv",
        "*.pyc",
        "*.pyo",
        "*.pyd",
        ".python-version",
        "*.egg-info",
        "*.egg",
        "develop-eggs",
        // Rust
        "target",
        "Cargo.lock",
        ".cargo",
        // IDEs & Editors
        ".idea",
        ".vscode",
        ".vs",
        ".sublime-*",
        "*.swp",
        "*.swo",
        "*~",
        ".project",
        ".settings",
        ".classpath",
        ".factorypath",
        "*.iml",
        "*.iws",
        "*.ipr",
        // Caches & Temp
        ".cache",
        "tmp",
        "temp",
        "logs",
        ".sass-cache",
        ".eslintcache",
        "*.log",
        "npm-debug.log*",
        "yarn-debug.log*",
        "yarn-error.log*",
        // Other Build Tools
        ".gradle",
        "gradle",
        ".maven",
        ".m2",
        "*.class",
        "*.jar",
        "*.war",
        "*.ear",
        // JavaScript/TypeScript
        "coverage",
        ".nyc_output",
        ".next",
        "*.tsbuildinfo",
        ".nuxt",
        ".output",
        // .NET
        "bin",
        "obj",
        "Debug",
        "Release",
        "packages",
        "*.suo",
        "*.user",
        "*.pubxml",
        "*.pubxml.user",
        // Documentation
        "_site",
        ".jekyll-cache",
        ".docusaurus",
        // Mobile Development
        ".gradle",
        "build",
        "xcuserdata",
        "*.xcworkspace",
        "Pods/",
        ".expo",
        // Database
        "*.sqlite",
        "*.sqlite3",
        "*.db",
        // Archives
        "*.zip",
        "*.tar.gz",
        "*.tgz",
        "*.rar",
        // Kubernetes
        ".kube",
        "*.kubeconfig",
        // Terraform
        ".terraform",
        "*.tfstate",
        "*.tfvars",
        // Ansible
        "*.retry",
    ]
});
