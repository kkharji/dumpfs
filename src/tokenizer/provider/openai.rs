//! OpenAI tokenizer implementation using tiktoken

use tiktoken_rs::CoreBPE;

use super::Provider;
use crate::tokenizer::error::{TokenizerError, TokenizerResult};
use crate::tokenizer::model::Model;

/// OpenAI tokenizer implementation
pub struct OpenAIProvider {
    model: Model,
    encoding: CoreBPE,
}

impl OpenAIProvider {
    /// Create a new OpenAI tokenizer
    pub fn new(model: Model) -> TokenizerResult<Self> {
        let encoding = tiktoken_rs::get_bpe_from_model(model.model_id())
            .map_err(|e| TokenizerError::TokenizerError(e.to_string()))?;

        Ok(Self { model, encoding })
    }
}

impl Provider for OpenAIProvider {
    fn count_tokens(&self, text: &str) -> TokenizerResult<usize> {
        let tokens = self.encoding.encode_ordinary(text);
        Ok(tokens.len())
    }

    fn model_context_window(&self) -> usize {
        self.model.context_window()
    }
}
