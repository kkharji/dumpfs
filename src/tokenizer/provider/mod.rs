//! Provider implementations for different tokenizer backends

pub mod anthropic;
pub mod huggingface;
pub mod openai;

use crate::tokenizer::error::TokenizerResult;

/// Trait for tokenizer provider implementations
pub trait Provider: Send + Sync {
    /// Count tokens in the given text
    fn count_tokens(&self, text: &str) -> TokenizerResult<usize>;

    /// Get the context window size for this model
    fn model_context_window(&self) -> usize;
}
