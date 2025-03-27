//! HuggingFace tokenizer implementation

use once_cell::sync::OnceCell;
use std::sync::Mutex;
use tokenizers::Tokenizer as HfTokenizer;

use super::Provider;
use crate::tokenizer::error::{TokenizerError, TokenizerResult};
use crate::tokenizer::model::Model;

/// HuggingFace tokenizer implementation
pub struct HuggingFaceProvider {
    model: Model,
    repo_id: &'static str,
    tokenizer: OnceCell<Mutex<HfTokenizer>>,
}

impl HuggingFaceProvider {
    /// Create a new HuggingFace tokenizer
    pub fn new(model: Model) -> Self {
        Self {
            model,
            repo_id: model.model_id(),
            tokenizer: OnceCell::new(),
        }
    }

    /// Get or initialize the tokenizer
    fn get_tokenizer(&self) -> TokenizerResult<&Mutex<HfTokenizer>> {
        self.tokenizer.get_or_try_init(|| {
            // Try to load the tokenizer from HuggingFace
            let tokenizer = match HfTokenizer::from_pretrained(self.repo_id, None) {
                Ok(t) => t,
                Err(e) => {
                    // Fall back to a basic BPE tokenizer
                    eprintln!("Error loading tokenizer: {}, using fallback", e);
                    let mut tokenizer = HfTokenizer::new(tokenizers::models::bpe::BPE::default());

                    // Configure for LLaMA-like tokenization
                    tokenizer.with_pre_tokenizer(Some(
                        tokenizers::pre_tokenizers::whitespace::Whitespace,
                    ));

                    tokenizer
                }
            };

            Ok(Mutex::new(tokenizer))
        })
    }
}

impl Provider for HuggingFaceProvider {
    fn count_tokens(&self, text: &str) -> TokenizerResult<usize> {
        // Get the tokenizer
        let tokenizer_mutex = self.get_tokenizer()?;

        // Acquire the lock
        let tokenizer = tokenizer_mutex
            .lock()
            .map_err(|_| TokenizerError::TokenizerError("Failed to lock tokenizer".to_string()))?;

        // Encode the text
        let encoding = tokenizer
            .encode(text, false)
            .map_err(|e| TokenizerError::TokenizerError(format!("Failed to encode text: {}", e)))?;

        // Get the token count
        let tokens = encoding.get_ids().len();

        Ok(tokens)
    }

    fn model_context_window(&self) -> usize {
        self.model.context_window()
    }
}
