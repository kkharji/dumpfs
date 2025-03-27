/*!
 * Tokenizer module for token counting with different LLM models
 *
 * Includes caching of already tokenized content to improve performance
 * when the same content is processed multiple times.
 */

use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use clap::ValueEnum;
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json;
use strum::{Display, EnumIter, EnumProperty, EnumString};

/// Supported LLM models for tokenization
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    EnumIter,
    Display,
    ValueEnum,
    Serialize,
    Deserialize,
    EnumProperty,
)]
pub enum Model {
    #[strum(props(
        model_id = "claude-3-5-sonnet-latest",
        context_window = 200000,
        provider = "anthropic"
    ))]
    Sonnet35,

    #[strum(props(
        model_id = "claude-3-7-sonnet-latest",
        context_window = 200000,
        provider = "anthropic"
    ))]
    Sonnet37,

    // OpenAI models
    #[strum(props(model_id = "gpt-4", context_window = 8192, provider = "openai"))]
    Gpt4,

    #[strum(props(
        model_id = "gpt-4-0125-preview",
        context_window = 128000,
        provider = "openai"
    ))]
    Gpt4Turbo,

    #[strum(props(model_id = "gpt-4o", context_window = 8192, provider = "openai"))]
    Gpt4o,

    // HuggingFace models
    #[strum(props(
        model_id = "meta-llama/Llama-2-7b-hf",
        context_window = 4096,
        provider = "huggingface"
    ))]
    Llama2_7b,

    #[strum(props(
        model_id = "meta-llama/Llama-3-8b-hf",
        context_window = 8192,
        provider = "huggingface"
    ))]
    Llama3_8b,

    #[strum(props(
        model_id = "mistralai/Mistral-Small-3.1-24B-Base-2503",
        context_window = 128000,
        provider = "huggingface"
    ))]
    MistralSmall24B,

    #[strum(props(
        model_id = "mistralai/Mistral-Large-Instruct-2411",
        context_window = 128000,
        provider = "huggingface"
    ))]
    MistralLargeInstruct,

    #[strum(props(
        model_id = "mistralai/Pixtral-12B-Base-2409",
        context_window = 128000,
        provider = "huggingface"
    ))]
    Pixtral12B,

    #[strum(props(
        model_id = "mistralai/Mistral-Small-Instruct-2409",
        context_window = 32000,
        provider = "huggingface"
    ))]
    MistralSmall,
}

impl Model {
    /// Get the context window size for this model
    pub fn context_window(&self) -> usize {
        self.get_int("context_window").unwrap() as usize
    }

    /// Get the provider of this model
    pub fn provider(&self) -> ModelProvider {
        let provider = self.get_str("provider").unwrap();
        ModelProvider::from_str(provider).unwrap()
    }

    /// Get the model identifier as used by the provider's API
    pub fn model_id(&self) -> &'static str {
        self.get_str("model_id").unwrap()
    }
}

/// Model providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum ModelProvider {
    /// Anthropic (Claude models)
    Anthropic,
    /// OpenAI (GPT models)
    OpenAI,
    /// HuggingFace models
    HuggingFace,
}

/// Get the path to the token cache file for a specific project directory
pub fn get_cache_path(project_dir: &str) -> Result<PathBuf, TokenizerError> {
    // Get home directory
    let home_dir = dirs::home_dir().ok_or_else(|| {
        TokenizerError::ApiError("Could not determine home directory".to_string())
    })?;

    // Create ~/.cache/dumpfs directory if it doesn't exist
    let cache_dir = home_dir.join(".cache").join("dumpfs");
    fs::create_dir_all(&cache_dir).map_err(|e| {
        TokenizerError::ApiError(format!("Failed to create cache directory: {}", e))
    })?;

    // Create a sanitized filename based on the project directory path
    let canonical_path = fs::canonicalize(project_dir)
        .map_err(|e| TokenizerError::ApiError(format!("Invalid project directory: {}", e)))?;

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

/// Cache for token counts to avoid redundant processing
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TokenCache {
    /// Cached token entries
    entries: Vec<TokenCacheEntry>,
    /// Number of cache hits
    #[serde(skip)]
    pub hits: usize,
    /// Number of cache misses
    #[serde(skip)]
    pub misses: usize,
}

impl TokenCache {
    /// Create a new empty token cache
    pub fn new(project_dir: &str) -> Self {
        // Try to load cache from disk, otherwise create new
        Self::load(project_dir).unwrap_or_else(|_| Self {
            entries: Vec::new(),
            hits: 0,
            misses: 0,
        })
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

        // Find matching entry by hash and model
        let result = self
            .entries
            .iter()
            .find(|entry| entry.hash == hash && entry.model == model_id)
            .map(|entry| entry.tokens);

        if result.is_some() {
            self.hits += 1;
            CACHE_HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.misses += 1;
            CACHE_MISSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        result
    }

    /// Insert token count into cache
    pub fn insert(&mut self, content: &str, model_id: &str, count: usize, project_dir: &str) {
        let hash = self.hash_content(content);

        // Remove existing entry with same hash and model if present
        self.entries
            .retain(|entry| !(entry.hash == hash && entry.model == model_id));

        // Add new entry
        self.entries.push(TokenCacheEntry {
            hash,
            model: model_id.to_string(),
            tokens: count,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });

        // Save cache to disk
        if let Err(e) = self.save(project_dir) {
            eprintln!("Failed to save token cache: {}", e);
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> (usize, usize) {
        (self.hits, self.misses)
    }

    /// Load cache from disk
    pub fn load(project_dir: &str) -> Result<Self, TokenizerError> {
        let path = get_cache_path(project_dir)?;

        if !path.exists() {
            return Err(TokenizerError::ApiError("Cache file not found".to_string()));
        }

        let content = std::fs::read_to_string(&path)
            .map_err(|e| TokenizerError::ApiError(format!("Failed to read cache file: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| TokenizerError::ApiError(format!("Failed to parse cache file: {}", e)))
    }

    /// Save cache to disk
    pub fn save(&self, project_dir: &str) -> Result<(), TokenizerError> {
        let content = serde_json::to_string(self)
            .map_err(|e| TokenizerError::ApiError(format!("Failed to serialize cache: {}", e)))?;

        let path = get_cache_path(project_dir)?;
        std::fs::write(&path, content)
            .map_err(|e| TokenizerError::ApiError(format!("Failed to write cache file: {}", e)))?;

        Ok(())
    }
}

/// TokenCount represents the result of token counting
#[derive(Debug, Clone, Copy)]
pub struct TokenCount {
    /// Number of tokens in the text
    pub tokens: usize,
    /// Whether this was a cache hit (if caching is enabled)
    pub cached: Option<bool>,
}

/// TokenizerError represents errors from tokenizer operations
#[derive(Debug)]
pub enum TokenizerError {
    /// Error calling API
    ApiError(String),

    /// Error with the tokenizer library
    TokenizerError(String),

    /// Model is not supported
    UnsupportedModel(String),

    /// Environment variable not set
    EnvVarError(String),
}

impl Display for TokenizerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizerError::ApiError(msg) => write!(f, "API error: {}", msg),
            TokenizerError::TokenizerError(msg) => write!(f, "Tokenizer error: {}", msg),
            TokenizerError::UnsupportedModel(msg) => write!(f, "Unsupported model: {}", msg),
            TokenizerError::EnvVarError(msg) => write!(f, "Environment variable error: {}", msg),
        }
    }
}

impl Error for TokenizerError {}

impl From<std::io::Error> for TokenizerError {
    fn from(error: std::io::Error) -> Self {
        TokenizerError::ApiError(format!("IO error: {}", error))
    }
}

impl From<serde_json::Error> for TokenizerError {
    fn from(error: serde_json::Error) -> Self {
        TokenizerError::ApiError(format!("JSON error: {}", error))
    }
}

/// Tokenizer trait defines the interface for all tokenizers
pub trait Tokenizer: Send + Sync {
    /// Count tokens in the given text
    fn count_tokens(&self, text: &str) -> Result<TokenCount, TokenizerError>;

    /// Get the context window size for this model
    fn model_context_window(&self) -> usize;
}

/// Cached tokenizer that wraps another tokenizer and caches results
pub struct CachedTokenizer {
    /// Inner tokenizer that does the actual tokenization
    inner: Box<dyn Tokenizer>,
    /// Cache for token counts
    cache: Arc<Mutex<TokenCache>>,
    /// Model used for tokenization
    model: Model,
    /// Project directory for cache storage
    project_dir: String,
}

// Global cache statistics for easier access
static CACHE_HITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CACHE_MISSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl CachedTokenizer {
    /// Create a new cached tokenizer wrapping another tokenizer
    pub fn new(inner: Box<dyn Tokenizer>, model: Model, project_dir: &str) -> Self {
        // Clean up and optimize cache on creation
        Self::clean_old_cache_entries(project_dir).ok();

        Self {
            inner,
            cache: Arc::new(Mutex::new(TokenCache::new(project_dir))),
            model,
            project_dir: project_dir.to_string(),
        }
    }

    /// Clean old cache entries (older than 7 days)
    fn clean_old_cache_entries(project_dir: &str) -> Result<(), TokenizerError> {
        let path = get_cache_path(project_dir)?;
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&path)?;
        let mut cache: TokenCache = serde_json::from_str(&content)?;

        // Current timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
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

    /// Get cache statistics (hits, misses)
    pub fn get_cache_stats(&self) -> (usize, usize) {
        if let Ok(cache) = self.cache.lock() {
            cache.get_stats()
        } else {
            (0, 0) // Return zeros if mutex is poisoned
        }
    }

    /// Get global cache statistics
    pub fn get_global_cache_stats() -> (usize, usize) {
        let hits = CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed);
        let misses = CACHE_MISSES.load(std::sync::atomic::Ordering::Relaxed);
        (hits, misses)
    }
}

impl Tokenizer for CachedTokenizer {
    fn count_tokens(&self, text: &str) -> Result<TokenCount, TokenizerError> {
        let model_id = self.model.model_id();

        // Try to get from cache first
        let cached = if let Ok(mut cache) = self.cache.lock() {
            cache.get(text, model_id)
        } else {
            None
        };

        // If found in cache, return it
        if let Some(count) = cached {
            return Ok(TokenCount {
                tokens: count,
                cached: Some(true),
            });
        }

        // Otherwise, delegate to inner tokenizer
        let result = self.inner.count_tokens(text)?;

        // Update cache with the result
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(text, model_id, result.tokens, &self.project_dir);
        }

        // Return result with cache flag
        Ok(TokenCount {
            tokens: result.tokens,
            cached: Some(false),
        })
    }

    fn model_context_window(&self) -> usize {
        self.inner.model_context_window()
    }
}

/// Create a tokenizer for the specified model
pub fn create_tokenizer(
    model: Model,
    project_dir: &str,
) -> Result<Box<dyn Tokenizer>, TokenizerError> {
    let inner: Box<dyn Tokenizer> = match model.provider() {
        ModelProvider::Anthropic => Box::new(ClaudeTokenizer::new(model)),
        ModelProvider::OpenAI => Box::new(OpenAITokenizer::new(model)?),
        ModelProvider::HuggingFace => Box::new(HuggingFaceTokenizer::new(model)),
    };

    // Wrap with cached tokenizer
    Ok(Box::new(CachedTokenizer::new(inner, model, project_dir)))
}

/// Claude tokenizer implementation
pub struct ClaudeTokenizer {
    model: Model,
}

impl ClaudeTokenizer {
    pub fn new(model: Model) -> Self {
        Self { model }
    }
}

impl Tokenizer for ClaudeTokenizer {
    fn count_tokens(&self, text: &str) -> Result<TokenCount, TokenizerError> {
        // Check if API key is set
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            TokenizerError::EnvVarError(
                "ANTHROPIC_API_KEY environment variable not set".to_string(),
            )
        })?;

        // Create client and send request to token counting endpoint
        let client = reqwest::blocking::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages/count_tokens")
            .header("x-api-key", api_key)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": self.model.model_id(),
                "messages": [{
                    "role": "user",
                    "content": text
                }]
            }))
            .send()
            .map_err(|e| TokenizerError::ApiError(format!("Failed to send request: {}", e)))?;

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unable to read error message".to_string());
            return Err(TokenizerError::ApiError(format!(
                "Claude API returned error status {}: {}",
                status, error_text
            )));
        }

        // Parse the response
        #[derive(Deserialize)]
        struct TokenResponse {
            input_tokens: usize,
        }

        let token_response: TokenResponse = response
            .json()
            .map_err(|e| TokenizerError::ApiError(format!("Failed to parse response: {}", e)))?;

        Ok(TokenCount {
            tokens: token_response.input_tokens,
            cached: None,
        })
    }

    fn model_context_window(&self) -> usize {
        self.model.context_window()
    }
}

/// OpenAITokenizer encapsulates tiktoken-based tokenization for OpenAI models
pub struct OpenAITokenizer {
    model: Model,
    encoding: tiktoken_rs::CoreBPE,
}

impl OpenAITokenizer {
    pub fn new(model: Model) -> Result<Self, TokenizerError> {
        let encoding = tiktoken_rs::get_bpe_from_model(model.model_id())
            .map_err(|e| TokenizerError::TokenizerError(e.to_string()))?;

        Ok(Self { model, encoding })
    }
}

impl Tokenizer for OpenAITokenizer {
    fn count_tokens(&self, text: &str) -> Result<TokenCount, TokenizerError> {
        let tokens = self.encoding.encode_ordinary(text);
        Ok(TokenCount {
            tokens: tokens.len(),
            cached: None,
        })
    }

    fn model_context_window(&self) -> usize {
        self.model.context_window()
    }
}

/// HuggingFace tokenizer implementation using the tokenizers crate
pub struct HuggingFaceTokenizer {
    model: Model,
    repo_id: &'static str,
    tokenizer: Option<tokenizers::Tokenizer>,
}

impl HuggingFaceTokenizer {
    pub fn new(model: Model) -> Self {
        let repo_id = model.model_id();

        // Don't initialize tokenizer here - lazy load on first use
        Self {
            model,
            repo_id,
            tokenizer: None,
        }
    }

    /// Lazily initialize the tokenizer on first use
    fn get_or_initialize_tokenizer(&mut self) -> Result<&tokenizers::Tokenizer, TokenizerError> {
        if self.tokenizer.is_none() {
            // Load tokenizer from HuggingFace Hub
            let tokenizer = match tokenizers::Tokenizer::from_pretrained(self.repo_id, None) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("{e}");
                    let mut tokenizer =
                        tokenizers::Tokenizer::new(tokenizers::models::bpe::BPE::default());

                    // Configure for LLaMA-like tokenization
                    tokenizer.with_pre_tokenizer(Some(
                        tokenizers::pre_tokenizers::whitespace::Whitespace,
                    ));

                    tokenizer
                }
            };

            self.tokenizer = Some(tokenizer);
        }

        Ok(self.tokenizer.as_ref().unwrap())
    }
}

impl Tokenizer for HuggingFaceTokenizer {
    fn count_tokens(&self, text: &str) -> Result<TokenCount, TokenizerError> {
        // We need to make self mutable for lazy initialization
        let mut mutable_self = Self {
            model: self.model,
            repo_id: self.repo_id,
            tokenizer: self.tokenizer.clone(),
        };

        // Get or initialize the tokenizer
        let tokenizer = mutable_self.get_or_initialize_tokenizer()?;

        // Encode the text
        let encoding = tokenizer
            .encode(text, false)
            .map_err(|e| TokenizerError::TokenizerError(format!("Failed to encode text: {}", e)))?;

        // Get the token count
        let tokens = encoding.get_ids().len();

        eprintln!("HuggingFace tokenizer using model: {}", self.repo_id);
        eprintln!("Token count for text of length {}: {}", text.len(), tokens);

        Ok(TokenCount {
            tokens,
            cached: None,
        })
    }

    fn model_context_window(&self) -> usize {
        self.model.context_window()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // OpenAI Tests
    mod openai_tests {
        use super::*;

        #[test]
        fn test_openai_tokenizer_basic() {
            let tokenizer = OpenAITokenizer::new(Model::Gpt4).expect("Intialize openai tokenizer");
            let result = tokenizer.count_tokens("Hello, world!");
            assert!(result.is_ok());

            let tokens = result.unwrap().tokens;
            assert!(tokens > 0);
        }

        #[test]
        fn test_openai_tokenizer_code_content() {
            let tokenizer = OpenAITokenizer::new(Model::Gpt4o).expect("Intialize openai tokenizer");
            let code = r#"
                fn main() {
                    println!("Hello, world!");
                    let x = 42;
                    let y = x * 2;
                    println!("x: {}, y: {}", x, y);
                }
            "#;

            let result = tokenizer.count_tokens(code);
            assert!(result.is_ok());

            let tokens = result.unwrap().tokens;
            // Code should be tokenized differently than plain text
            assert!(tokens > code.len() / 10); // Very rough estimate
        }

        #[test]
        fn test_openai_tokenizer_special_tokens() {
            let tokenizer =
                OpenAITokenizer::new(Model::Gpt4Turbo).expect("Intialize openai tokenizer");
            // Test with various special tokens and character sets
            let special_text = "Hello\n\nworld! <|endoftext|> User: Assistant: $$$";

            let result = tokenizer.count_tokens(special_text);
            assert!(result.is_ok());

            let tokens = result.unwrap().tokens;
            // Special tokens should be counted separately
            assert!(tokens >= 10); // Rough minimum based on content
        }
    }

    // HuggingFace Tests
    mod huggingface_tests {
        use super::*;

        #[test]
        fn test_huggingface_tokenizer_basic() {
            let tokenizer = HuggingFaceTokenizer::new(Model::Llama2_7b);
            let result = tokenizer.count_tokens("Hello, world!");
            assert!(result.is_ok());

            let tokens = result.unwrap().tokens;
            assert!(tokens > 0);
        }

        #[test]
        fn test_huggingface_tokenizer_long_text() {
            let tokenizer = HuggingFaceTokenizer::new(Model::Llama3_8b);
            let long_text = "This is a much longer text that should be tokenized into many tokens. It includes various sentences with different structures and vocabulary to test the tokenizer's behavior with more complex content. The goal is to ensure that the tokenizer can handle real-world text correctly.";

            let result = tokenizer.count_tokens(long_text);
            assert!(result.is_ok());

            let tokens = result.unwrap().tokens;
            assert!(tokens >= long_text.split_whitespace().count());
        }

        #[test]
        fn test_huggingface_tokenizer_fallback() {
            // Test the fallback mechanism when model loading fails
            let tokenizer = HuggingFaceTokenizer::new(Model::MistralSmall);

            // Use intentionally difficult text
            let text = "🤗 This includes emoji and unusual characters: →☺←";

            let result = tokenizer.count_tokens(text);
            // The result should be Ok even if the model couldn't be loaded
            assert!(result.is_ok());
        }
    }

    // Claude Tests (only run when API key is available)
    mod claude_tests {
        use super::*;

        #[test]
        #[ignore] // Skip this test by default since it requires an API key
        fn test_claude_tokenizer() {
            // Only run this test if ANTHROPIC_API_KEY is set
            match env::var("ANTHROPIC_API_KEY") {
                Ok(api_key) if !api_key.is_empty() => {
                    let tokenizer = ClaudeTokenizer::new(Model::Sonnet37);
                    let result = tokenizer.count_tokens("Hello, Claude! How are you today?");

                    assert!(result.is_ok());
                    let tokens = result.unwrap().tokens;
                    assert!(tokens > 0);

                    // When API key is available, test different content types
                    let code_result = tokenizer
                        .count_tokens("def hello():\n    print('Hello, world!')\n\nhello()");
                    assert!(code_result.is_ok());
                }
                _ => {
                    // Skip test when API key is not available
                    println!("Skipping Claude tokenizer test (no API key)");
                }
            }
        }

        #[test]
        fn test_claude_tokenizer_error_handling() {
            // Test error handling when API key is not set
            // Temporarily unset the API key if it exists
            let api_key = env::var("ANTHROPIC_API_KEY").ok();
            env::remove_var("ANTHROPIC_API_KEY");

            let tokenizer = ClaudeTokenizer::new(Model::Sonnet35);
            let result = tokenizer.count_tokens("Hello, Claude!");

            // Should return an EnvVarError
            assert!(result.is_err());
            match result {
                Err(TokenizerError::EnvVarError(_)) => (), // Expected error
                _ => panic!("Expected EnvVarError when API key is not set"),
            }

            // Restore API key if it was set
            if let Some(key) = api_key {
                env::set_var("ANTHROPIC_API_KEY", key);
            }
        }
    }
}
