/*!
 * Tests for DumpFS functionality
 */

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indicatif::ProgressBar;
use quick_xml::events::Event;
use quick_xml::Reader;
use tempfile::tempdir;

use crate::config::{Config, GitCachePolicy};
use crate::git::{GitHost, GitRepoInfo};
// Git module imports not needed as tests are moved
use crate::scanner::Scanner;
use crate::writer::XmlWriter;

// Helper function to create a test directory structure
fn setup_test_directory() -> io::Result<tempfile::TempDir> {
    let temp_dir = tempdir()?;

    // Create a simple directory structure
    fs::create_dir(temp_dir.path().join("dir1"))?;
    fs::create_dir(temp_dir.path().join("dir2"))?;
    fs::create_dir(temp_dir.path().join("dir1").join("subdir"))?;

    // Create text files
    let mut file1 = File::create(temp_dir.path().join("file1.txt"))?;
    writeln!(file1, "This is a text file with content")?;

    let mut file2 = File::create(temp_dir.path().join("dir1").join("file2.txt"))?;
    writeln!(file2, "This is another text file\nwith multiple lines")?;

    let mut file3 = File::create(
        temp_dir
            .path()
            .join("dir1")
            .join("subdir")
            .join("file3.txt"),
    )?;
    writeln!(file3, "Nested file content")?;

    // Create files to be ignored
    fs::create_dir(temp_dir.path().join(".git"))?;
    let mut git_file = File::create(temp_dir.path().join(".git").join("config"))?;
    writeln!(git_file, "[core]\n\trepositoryformatversion = 0")?;

    // Create a binary file
    let mut bin_file = File::create(temp_dir.path().join("binary.bin"))?;
    bin_file.write_all(&[0u8, 1u8, 2u8, 3u8])?;

    // Create a symlink if not on Windows
    #[cfg(not(target_os = "windows"))]
    std::os::unix::fs::symlink(
        temp_dir.path().join("file1.txt"),
        temp_dir.path().join("symlink.txt"),
    )?;

    Ok(temp_dir)
}

// Helper function to create a test directory with a .gitignore file
fn setup_gitignore_test_directory() -> io::Result<tempfile::TempDir> {
    let temp_dir = setup_test_directory()?;

    // Create a .gitignore file
    let mut gitignore = File::create(temp_dir.path().join(".gitignore"))?;
    writeln!(gitignore, "# Ignore all .txt files")?;
    writeln!(gitignore, "*.txt")?;
    writeln!(gitignore, "# Ignore binary.bin")?;
    writeln!(gitignore, "binary.bin")?;

    // Create some additional files that aren't explicitly ignored
    let mut not_ignored = File::create(temp_dir.path().join("not_ignored.md"))?;
    writeln!(not_ignored, "# This file shouldn't be ignored")?;

    Ok(temp_dir)
}

// Helper function to create a large file (>1MB)
fn create_large_file(dir: &Path) -> io::Result<()> {
    let path = dir.join("large_file.txt");
    let mut file = File::create(path)?;

    // Write over 1MB of data
    let line = "This is a line of text that will be repeated many times to create a large file.\n";
    for _ in 0..20000 {
        file.write_all(line.as_bytes())?;
    }

    Ok(())
}

// Test basic scanning functionality
#[test]
fn test_basic_scan() -> io::Result<()> {
    let temp_dir = setup_test_directory()?;
    let output_file = temp_dir.path().join("output.xml");

    let config = Config {
        target_dir: temp_dir.path().to_path_buf(),
        output_file: output_file.clone(),
        ignore_patterns: vec![],
        include_patterns: vec![],
        num_threads: 1,
        respect_gitignore: false,
        gitignore_path: None,
        model: None,
        repo_url: None,
        git_repo: None,
        git_cache_policy: GitCachePolicy::AlwaysPull,
    };

    let progress = Arc::new(ProgressBar::hidden());
    let scanner = Scanner::new(config.clone(), Arc::clone(&progress));
    let writer = XmlWriter::new(config);

    let root_node = scanner.scan()?;
    writer.write(&root_node)?;

    // Check that the output file exists
    assert!(output_file.exists());

    // Read the XML file to verify structure
    let xml_content = fs::read_to_string(&output_file)?;

    // Check basic structure
    assert!(xml_content.contains("<directory_scan"));
    assert!(xml_content.contains("<system_info>"));
    assert!(xml_content.contains("<hostname>"));
    assert!(xml_content.contains("<directory name="));
    assert!(xml_content.contains("<file name=\"file1.txt\""));
    assert!(xml_content.contains("This is a text file with content"));

    // The .git directory should be ignored by default
    assert!(!xml_content.contains(".git"));

    Ok(())
}

// Test ignore patterns
#[test]
fn test_ignore_patterns() -> io::Result<()> {
    let temp_dir = setup_test_directory()?;
    let output_file = temp_dir.path().join("output.xml");

    let config = Config {
        target_dir: temp_dir.path().to_path_buf(),
        output_file: output_file.clone(),
        ignore_patterns: vec!["*.txt".to_string()],
        include_patterns: vec![],
        num_threads: 1,
        respect_gitignore: false,
        model: None,
        gitignore_path: None,
        repo_url: None,
        git_repo: None,
        git_cache_policy: GitCachePolicy::AlwaysPull,
    };

    let progress = Arc::new(ProgressBar::hidden());
    let scanner = Scanner::new(config.clone(), Arc::clone(&progress));
    let writer = XmlWriter::new(config);

    let root_node = scanner.scan()?;
    writer.write(&root_node)?;

    // Read the XML file
    let xml_content = fs::read_to_string(&output_file)?;

    // All .txt files should be ignored
    assert!(!xml_content.contains("file1.txt"));
    assert!(!xml_content.contains("file2.txt"));
    assert!(!xml_content.contains("file3.txt"));

    // The binary file should still be included
    assert!(xml_content.contains("binary.bin"));

    Ok(())
}

// Test include patterns
#[test]
fn test_include_patterns() -> io::Result<()> {
    let temp_dir = setup_test_directory()?;
    let output_file = temp_dir.path().join("output.xml");

    let config = Config {
        target_dir: temp_dir.path().to_path_buf(),
        output_file: output_file.clone(),
        ignore_patterns: vec![],
        include_patterns: vec!["*.bin".to_string()],
        num_threads: 1,
        respect_gitignore: false,
        model: None,
        gitignore_path: None,
        repo_url: None,
        git_repo: None,
        git_cache_policy: GitCachePolicy::AlwaysPull,
    };

    let progress = Arc::new(ProgressBar::hidden());
    let scanner = Scanner::new(config.clone(), Arc::clone(&progress));
    let writer = XmlWriter::new(config);

    let root_node = scanner.scan()?;
    writer.write(&root_node)?;

    // Read the XML file
    let xml_content = fs::read_to_string(&output_file)?;

    // Only .bin files should be included
    assert!(!xml_content.contains("file1.txt"));
    assert!(!xml_content.contains("file2.txt"));
    assert!(!xml_content.contains("file3.txt"));
    assert!(xml_content.contains("binary.bin"));

    Ok(())
}

// Test handling of large files
#[test]
fn test_large_file_handling() -> io::Result<()> {
    let temp_dir = setup_test_directory()?;
    create_large_file(temp_dir.path())?;

    let output_file = temp_dir.path().join("output.xml");

    let config = Config {
        target_dir: temp_dir.path().to_path_buf(),
        output_file: output_file.clone(),
        ignore_patterns: vec![],
        include_patterns: vec![],
        num_threads: 1,
        respect_gitignore: false,
        model: None,
        gitignore_path: None,
        repo_url: None,
        git_repo: None,
        git_cache_policy: GitCachePolicy::AlwaysPull,
    };

    let progress = Arc::new(ProgressBar::hidden());
    let scanner = Scanner::new(config.clone(), Arc::clone(&progress));
    let writer = XmlWriter::new(config);

    let root_node = scanner.scan()?;
    writer.write(&root_node)?;

    // Read the XML file
    let xml_content = fs::read_to_string(&output_file)?;

    // Large file should be mentioned but content should be truncated
    assert!(xml_content.contains("large_file.txt"));
    assert!(xml_content.contains("File too large to include content"));

    Ok(())
}

// Test XML structure validity
#[test]
fn test_xml_validity() -> io::Result<()> {
    let temp_dir = setup_test_directory()?;
    let output_file = temp_dir.path().join("output.xml");

    let config = Config {
        target_dir: temp_dir.path().to_path_buf(),
        output_file: output_file.clone(),
        ignore_patterns: vec![],
        include_patterns: vec![],
        num_threads: 1,
        model: None,
        respect_gitignore: false,
        gitignore_path: None,
        repo_url: None,
        git_repo: None,
        git_cache_policy: GitCachePolicy::AlwaysPull,
    };

    let progress = Arc::new(ProgressBar::hidden());
    let scanner = Scanner::new(config.clone(), Arc::clone(&progress));
    let writer = XmlWriter::new(config);

    let root_node = scanner.scan()?;
    writer.write(&root_node)?;

    // Parse the XML file to verify it's well-formed
    let file_content = fs::read_to_string(&output_file)?;
    let mut reader = Reader::from_str(&file_content);

    let mut depth = 0;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => depth -= 1,
            Ok(Event::Eof) => break,
            Err(e) => panic!("Error parsing XML: {}", e),
            _ => (),
        }
        buf.clear();
    }

    // If XML is well-formed, depth should be 0 at the end
    assert_eq!(depth, 0, "XML structure is not well-balanced");

    Ok(())
}

// Test respecting .gitignore files
#[test]
fn test_respect_gitignore() -> io::Result<()> {
    let temp_dir = setup_gitignore_test_directory()?;
    let output_file = temp_dir.path().join("output.xml");

    let config = Config {
        target_dir: temp_dir.path().to_path_buf(),
        output_file: output_file.clone(),
        ignore_patterns: vec![],
        include_patterns: vec![],
        num_threads: 1,
        respect_gitignore: true,
        model: None,
        gitignore_path: None,
        repo_url: None,
        git_repo: None,
        git_cache_policy: GitCachePolicy::AlwaysPull,
    };

    let progress = Arc::new(ProgressBar::hidden());
    let scanner = Scanner::new(config.clone(), Arc::clone(&progress));
    let writer = XmlWriter::new(config);

    let root_node = scanner.scan()?;
    writer.write(&root_node)?;

    // Read the XML file
    let xml_content = fs::read_to_string(&output_file)?;

    // Files excluded by .gitignore should not be present
    assert!(!xml_content.contains("file1.txt"));
    assert!(!xml_content.contains("file2.txt"));
    assert!(!xml_content.contains("file3.txt"));
    assert!(!xml_content.contains("binary.bin"));

    // Files not excluded by .gitignore should be present
    assert!(xml_content.contains("not_ignored.md"));

    Ok(())
}

#[test]
fn test_output_file_path_for_git_repo() {
    // Create a mock GitRepoInfo
    let repo_path = PathBuf::from("/tmp/cache/dumpfs/github/username/repo");
    let git_repo = GitRepoInfo {
        url: "https://github.com/username/repo".to_string(),
        host: GitHost::GitHub,
        owner: "username".to_string(),
        name: "repo".to_string(),
        cache_path: repo_path.clone(),
    };

    // Test cases for output file paths
    let test_cases = vec![
        // Relative file with no path component -> save to repo dir
        (".dumpfs.context.xml", repo_path.join(".dumpfs.context.xml")),
        // Relative file with path component -> keep as is
        ("output/file.xml", PathBuf::from("output/file.xml")),
        // Absolute path -> keep as is
        ("/tmp/output.xml", PathBuf::from("/tmp/output.xml")),
    ];

    for (input, expected) in test_cases {
        // Create a config with the test input
        let mut config = Config {
            target_dir: repo_path.clone(),
            output_file: PathBuf::from(input),
            ignore_patterns: vec![],
            include_patterns: vec![],
            num_threads: 1,
            respect_gitignore: false,
            gitignore_path: None,
            model: None,
            repo_url: Some("https://github.com/username/repo".to_string()),
            git_repo: Some(git_repo.clone()),
            git_cache_policy: GitCachePolicy::AlwaysPull,
        };

        // Apply output file path logic (simplified from main.rs)
        if let Some(repo) = &config.git_repo {
            let output_path = PathBuf::from(input);
            if !output_path.is_absolute()
                && (output_path.parent().is_none()
                    || output_path.parent().unwrap() == Path::new(""))
            {
                config.output_file = repo.cache_path.join(output_path);
            }
        }

        // Check if the path was adjusted correctly
        assert_eq!(config.output_file, expected);
    }
}
