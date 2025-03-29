/*!
 * Integration test for clipboard functionality
 */

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::process::Command;

use tempfile::tempdir;

#[test]
#[ignore] // This test requires tmux to be running and is ignored by default
          // To run this test manually use: cargo test --test clipboard_integration -- --ignored
fn test_clip_flag() {
    // Skip if not in a tmux session
    if env::var("TMUX").is_err() {
        return;
    }

    // Create a temporary directory with some test files
    let temp_dir = tempdir().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    let output_file = temp_dir.path().join("output.xml");

    // Write some content to the test file
    let mut file = File::create(&test_file).unwrap();
    writeln!(file, "Test content for clipboard integration").unwrap();

    // Build the project first to ensure binary is available
    assert!(Command::new("cargo")
        .args(["build"])
        .status()
        .unwrap()
        .success());

    // Run dumpfs with --clip flag
    // The command format is: dumpfs [OPTIONS] [DIRECTORY_PATH] [OUTPUT_FILE]
    let status = Command::new("cargo")
        .args([
            "run",
            "--",
            "--clip",
            &temp_dir.path().to_string_lossy(), // Directory path (positional)
            &output_file.to_string_lossy(),     // Output file (positional)
        ])
        .status()
        .unwrap();

    // Check that the command succeeded
    assert!(status.success());

    // Verify that the output file exists
    assert!(output_file.exists());

    // Get the content from the output file
    let xml_content = fs::read_to_string(&output_file).unwrap();

    // Get the content from the tmux clipboard
    let clipboard_output = Command::new("tmux").args(["show-buffer"]).output().unwrap();

    let clipboard_content = String::from_utf8_lossy(&clipboard_output.stdout);

    // Verify that the clipboard contains the XML content
    assert_eq!(xml_content, clipboard_content);
}
