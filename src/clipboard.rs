/*!
 * Clipboard support for DumpFS
 *
 * Provides functionality for copying output to system clipboard
 * with automatic detection of available clipboard mechanisms.
 */

use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use thiserror::Error;

/// Error type for clipboard operations
#[derive(Error, Debug)]
pub enum ClipboardError {
    /// The command is not available on the system
    #[error("Command not found: {0}")]
    CommandNotFound(String),

    /// Failed to execute the command
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// No suitable clipboard mechanism was found
    #[error("No suitable clipboard mechanism found")]
    NoClipboardFound,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Result type for clipboard operations
pub type Result<T> = std::result::Result<T, ClipboardError>;

/// Trait for clipboard operations
pub trait Clipboard {
    /// Copy text to the clipboard
    fn copy_to_clipboard(&self, text: &str) -> Result<()>;
}

/// Available clipboard providers
#[derive(Debug, Clone, Copy)]
enum ClipboardProvider {
    /// tmux clipboard
    Tmux,
    /// X11 clipboard with xclip
    Xclip,
    /// X11 clipboard with xsel
    Xsel,
    /// Wayland clipboard
    Wayland,
    /// macOS clipboard
    MacOS,
    /// Windows clipboard (via WSL)
    Wsl,
    /// Termux clipboard
    Termux,
}

impl Clipboard for ClipboardProvider {
    fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        let (cmd, args) = match self {
            Self::Tmux => {
                // Check tmux version to determine if we should use the -w flag
                let tmux_args = vec!["load-buffer", "-w", "-"];
                ("tmux", tmux_args)
            }
            Self::Xclip => ("xclip", vec!["-selection", "clipboard", "-in"]),
            Self::Xsel => ("xsel", vec!["-b", "-i"]),
            Self::Wayland => ("wl-copy", vec![]),
            Self::MacOS => ("pbcopy", vec![]),
            Self::Wsl => ("clip.exe", vec![]),
            Self::Termux => ("termux-clipboard-set", vec![]),
        };

        execute_clipboard_command(cmd, &args, text)
    }
}

//--------------------------------------------------------------------
// Public API
//--------------------------------------------------------------------

/// Copy text to the clipboard
///
/// Automatically detects the most appropriate clipboard mechanism
/// and uses it to copy text to the system clipboard.
///
/// # Arguments
/// * `text` - The text to copy to the clipboard
///
/// # Returns
/// * `Ok(())` - If the text was successfully copied
/// * `Err(ClipboardError)` - If the text could not be copied
///
/// # Examples
/// ```
/// use dumpfs::clipboard::copy_to_clipboard;
///
/// let result = copy_to_clipboard("Hello, clipboard!");
/// if let Err(e) = result {
///     eprintln!("Failed to copy to clipboard: {}", e);
/// }
/// ```
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let clipboard = get_clipboard()?;
    clipboard.copy_to_clipboard(text)
}

/// Check if a command exists on the system
///
/// # Arguments
/// * `command` - The command to check
///
/// # Returns
/// * `true` - If the command exists and can be executed
/// * `false` - Otherwise
pub fn command_exists(command: &str) -> bool {
    // First check if the command exists in the PATH
    if let Ok(paths) = env::var("PATH") {
        for path in paths.split(':') {
            let p = Path::new(path).join(command);
            if p.exists() {
                return true;
            }
        }
    }

    // Try to run the command with '--version' flag as fallback
    Command::new(command)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

//--------------------------------------------------------------------
// Internal Implementation
//--------------------------------------------------------------------

/// Get the appropriate clipboard implementation based on the system
fn get_clipboard() -> Result<Box<dyn Clipboard>> {
    // Define a list of providers to try in order of preference
    let providers = determine_clipboard_providers();

    // Try each provider in order
    for provider in providers {
        if let Some(clipboard) = try_clipboard_provider(provider) {
            return Ok(clipboard);
        }
    }

    // No clipboard mechanism found
    Err(ClipboardError::NoClipboardFound)
}

/// Execute a command to copy text to clipboard
///
/// This centralizes all the common command execution logic:
/// - Spawning the process
/// - Writing to stdin
/// - Waiting for completion
/// - Error handling
fn execute_clipboard_command(cmd: &str, args: &[&str], text: &str) -> Result<()> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| ClipboardError::CommandFailed(format!("Failed to spawn {}", cmd)))?;

    let stdin = child.stdin.as_mut().ok_or_else(|| {
        ClipboardError::CommandFailed(format!("Failed to open stdin for {}", cmd))
    })?;

    stdin
        .write_all(text.as_bytes())
        .map_err(|_| ClipboardError::CommandFailed(format!("Failed to write to {}", cmd)))?;

    let status = child
        .wait()
        .map_err(|_| ClipboardError::CommandFailed(format!("Failed to wait for {}", cmd)))?;

    if status.success() {
        Ok(())
    } else {
        Err(ClipboardError::CommandFailed(format!(
            "{} exited with status: {}",
            cmd, status
        )))
    }
}

/// Platform detection cache (using thread-safe lazy initialization)
static PLATFORM: OnceLock<&'static str> = OnceLock::new();

/// Determine the platform (cached)
fn get_platform() -> &'static str {
    PLATFORM.get_or_init(|| {
        if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            if env::var("WSL_DISTRO_NAME").is_ok() {
                "wsl"
            } else {
                "linux"
            }
        } else if cfg!(target_os = "android") {
            "android"
        } else {
            "unknown"
        }
    })
}

/// Determine which clipboard providers to try based on platform and preference
fn determine_clipboard_providers() -> Vec<ClipboardProvider> {
    let mut providers = Vec::with_capacity(3); // Pre-allocate space for typical number of providers

    // Always try tmux first if available and running (user preference)
    if command_exists("tmux") && is_tmux_running() {
        providers.push(ClipboardProvider::Tmux);
    }

    // Add platform-specific providers
    match get_platform() {
        "macos" => {
            if command_exists("pbcopy") {
                providers.push(ClipboardProvider::MacOS);
            }
        }
        "windows" | "wsl" => {
            if command_exists("clip.exe") {
                providers.push(ClipboardProvider::Wsl);
            }
        }
        "linux" => {
            // Try Wayland first
            if command_exists("wl-copy") {
                providers.push(ClipboardProvider::Wayland);
            }

            // Then X11 mechanisms
            if command_exists("xsel") {
                providers.push(ClipboardProvider::Xsel);
            }

            if command_exists("xclip") {
                providers.push(ClipboardProvider::Xclip);
            }
        }
        "android" => {
            if command_exists("termux-clipboard-set") {
                providers.push(ClipboardProvider::Termux);
            }
        }
        _ => {}
    }

    providers
}

/// Try to create a clipboard provider
fn try_clipboard_provider(provider: ClipboardProvider) -> Option<Box<dyn Clipboard>> {
    Some(Box::new(provider))
}

/// Check if tmux is running and available for clipboard operations
fn is_tmux_running() -> bool {
    // Check if TMUX environment variable is set (inside tmux session)
    if env::var("TMUX").is_ok() {
        return true;
    }

    // Try running tmux list-buffers as a fallback check
    let status = Command::new("tmux")
        .args(["list-buffers"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    // If the command succeeds, tmux is running and can be used
    status.map(|s| s.success()).unwrap_or(false)
}

//--------------------------------------------------------------------
// Tmux Version Handling
//--------------------------------------------------------------------

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_command_exists() {
        // These commands should exist on most systems
        assert!(command_exists("ls"));
        assert!(command_exists("echo"));

        // This command should not exist
        assert!(!command_exists("nonexistentcommandxyz"));
    }

    #[test]
    fn test_get_platform() {
        let platform = get_platform();

        // Just check that it's one of the known platforms
        assert!(
            platform == "macos"
                || platform == "windows"
                || platform == "wsl"
                || platform == "linux"
                || platform == "android"
                || platform == "unknown"
        );

        // Check that caching works (call again and verify it's the same result)
        let platform2 = get_platform();
        assert_eq!(platform, platform2);
    }

    #[test]
    #[ignore] // This test requires tmux to be installed and running
    fn test_tmux_clipboard() {
        // Skip the test if tmux is not available
        if !command_exists("tmux") {
            return;
        }

        // Check if we're in a tmux session
        let in_tmux = env::var("TMUX").is_ok();
        if !in_tmux {
            return;
        }

        let clipboard = ClipboardProvider::Tmux;
        let test_text = "Test text for tmux clipboard";

        // Copy to clipboard
        clipboard
            .copy_to_clipboard(test_text)
            .expect("Failed to copy to tmux clipboard");

        // Verify by reading from clipboard
        let output = Command::new("tmux")
            .args(["show-buffer"])
            .output()
            .expect("Failed to execute tmux show-buffer");

        let clipboard_content = String::from_utf8_lossy(&output.stdout);
        assert_eq!(clipboard_content.trim(), test_text);
    }
}
