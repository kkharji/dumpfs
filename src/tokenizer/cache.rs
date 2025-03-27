//! Token cache implementation

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::tokenizer::error::{TokenizerError, TokenizerResult};

// Global cache statistics for easier access
static CACHE_HITS: AtomicUsize = AtomicUsize::new(0);
static CACHE_MISSES: AtomicUsize = AtomicUsize::new(0);

/// Cache entry with token count and model identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCacheEntry {
    /// Hash of the content
    hash: u64,
    /// Model used for tokenization
    model: String,
    /// Token count
    tokens: usize,
    /// Timestamp when the entry was created
    timestamp: u64,
}

/// Statistics for token cache
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
}

impl CacheStats {
    /// Get global cache statistics
    pub fn global() -> Self {
        Self {
            hits: CACHE_HITS.load(Ordering::Relaxed),
            misses: CACHE_MISSES.load(Ordering::Relaxed),
        }
    }
}

/// Cache for token counts to avoid redundant processing
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenCache {
    /// Cached token entries
    entries: Vec<TokenCacheEntry>,

    #[serde(skip)]
    local_hits: usize,

    #[serde(skip)]
    local_misses: usize,
}

impl TokenCache {
    /// Create a new token cache
    pub fn new(project_dir: &str) -> TokenizerResult<Self> {
        // Try to load from disk, fall back to empty cache
        let cache = Self::load(project_dir).unwrap_or_else(|_| Self {
            entries: Vec::new(),
            local_hits: 0,
            local_misses: 0,
        });

        // Clean old entries
        cache.clean_old_entries(project_dir).ok();

        Ok(cache)
    }

    /// Calculate hash for content
    fn hash_content(&self, content: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Get token count from cache if available
    pub fn get(&mut self, content: &str, model_id: &str) -> Option<usize> {
        let hash = self.hash_content(content);

        // Find matching entry
        let result = self
            .entries
            .iter()
            .find(|entry| entry.hash == hash && entry.model == model_id)
            .map(|entry| entry.tokens);

        // Update statistics
        if result.is_some() {
            self.local_hits += 1;
            CACHE_HITS.fetch_add(1, Ordering::Relaxed);
        } else {
            self.local_misses += 1;
            CACHE_MISSES.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    /// Insert token count into cache
    pub fn insert(
        &mut self,
        content: &str,
        model_id: &str,
        count: usize,
        project_dir: &str,
    ) -> TokenizerResult<()> {
        let hash = self.hash_content(content);

        // Remove existing entry with same hash and model
        self.entries
            .retain(|entry| !(entry.hash == hash && entry.model == model_id));

        // Add new entry
        self.entries.push(TokenCacheEntry {
            hash,
            model: model_id.to_string(),
            tokens: count,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        // Save cache to disk
        self.save(project_dir)?;

        Ok(())
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStats {
        CacheStats {
            hits: self.local_hits,
            misses: self.local_misses,
        }
    }

    /// Clean old cache entries (older than 7 days)
    fn clean_old_entries(&self, project_dir: &str) -> TokenizerResult<()> {
        let mut cache = self.clone();

        // Current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // 7 days in seconds
        const WEEK_IN_SECS: u64 = 7 * 24 * 60 * 60;

        // Remove entries older than a week
        let old_len = cache.entries.len();
        cache
            .entries
            .retain(|entry| now - entry.timestamp < WEEK_IN_SECS);

        // If we removed any entries, save the file
        if cache.entries.len() < old_len {
            cache.save(project_dir)?;
        }

        Ok(())
    }

    /// Load cache from disk
    pub fn load(project_dir: &str) -> TokenizerResult<Self> {
        let path = get_cache_path(project_dir)?;

        if !path.exists() {
            return Err(TokenizerError::CacheError(
                "Cache file not found".to_string(),
            ));
        }

        let content = fs::read_to_string(&path)?;
        let mut cache: Self = serde_json::from_str(&content)?;

        // Initialize counters
        cache.local_hits = 0;
        cache.local_misses = 0;

        Ok(cache)
    }

    /// Save cache to disk
    pub fn save(&self, project_dir: &str) -> TokenizerResult<()> {
        let content = serde_json::to_string(self)?;
        let path = get_cache_path(project_dir)?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, content)?;

        Ok(())
    }

    /// Clone the cache for internal use
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
            local_hits: self.local_hits,
            local_misses: self.local_misses,
        }
    }
}

/// Get the path to the token cache file for a specific project directory
pub fn get_cache_path(project_dir: &str) -> TokenizerResult<PathBuf> {
    // Get home directory
    let home_dir = dirs::home_dir().ok_or_else(|| {
        TokenizerError::CacheError("Could not determine home directory".to_string())
    })?;

    // Create cache directory path
    let cache_dir = home_dir.join(".cache").join("dumpfs");

    // Create a sanitized filename based on the project directory path
    let canonical_path = fs::canonicalize(project_dir)
        .map_err(|e| TokenizerError::CacheError(format!("Invalid project directory: {}", e)))?;

    // Convert the path to a string, removing any invalid characters
    let path_str = canonical_path.to_string_lossy().to_string();
    let sanitized_path = path_str.replace(
        |c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.',
        "_",
    );

    // Create the cache file path
    let cache_file = cache_dir.join(format!("{}.token_cache.json", sanitized_path));

    Ok(cache_file)
}
