/*!
 * Progress reporting for Git operations
 */

use crate::utils::format_file_size;

/// Trait for reporting Git operation progress
pub trait ProgressReporter {
    /// Called with progress information during Git operations
    fn report(&self, progress: &GitProgress);
}

/// Progress information for Git operations
#[derive(Debug, Clone)]
pub struct GitProgress {
    /// Total number of objects to download
    pub total_objects: usize,
    /// Number of received objects
    pub received_objects: usize,
    /// Number of indexed objects
    pub indexed_objects: usize,
    /// Number of local objects
    pub local_objects: usize,
    /// Total number of deltas
    pub total_deltas: usize,
    /// Number of indexed deltas
    pub indexed_deltas: usize,
    /// Number of bytes received
    pub received_bytes: usize,
}

impl GitProgress {
    /// Get the progress percentage
    pub fn percentage(&self) -> u8 {
        if self.total_objects == 0 {
            return 0;
        }

        ((self.received_objects * 100) / self.total_objects) as u8
    }

    /// Get a formatted string of received bytes
    pub fn formatted_bytes(&self) -> String {
        format_file_size(self.received_bytes as u64)
    }
}

// Implement ProgressReporter for closures
impl<F> ProgressReporter for F
where
    F: Fn(&GitProgress),
{
    fn report(&self, progress: &GitProgress) {
        self(progress)
    }
}
