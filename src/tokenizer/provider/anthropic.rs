//! Anthropic Claude tokenizer implementation

use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::json;
use std::env;

use super::Provider;
use crate::tokenizer::error::{TokenizerError, TokenizerResult};
use crate::tokenizer::model::Model;

/// Claude tokenizer implementation
pub struct ClaudeProvider {
    model: Model,
    client: Client,
}

impl ClaudeProvider {
    /// Create a new Claude tokenizer
    pub fn new(model: Model) -> Self {
        Self {
            model,
            client: Client::new(),
        }
    }
}

impl Provider for ClaudeProvider {
    fn count_tokens(&self, text: &str) -> TokenizerResult<usize> {
        // Get API key from environment
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            TokenizerError::EnvVarError(
                "ANTHROPIC_API_KEY environment variable not set".to_string(),
            )
        })?;

        // Send request to token counting endpoint
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages/count_tokens")
            .header("x-api-key", api_key)
            .header("content-type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.model.model_id(),
                "messages": [{
                    "role": "user",
                    "content": text
                }]
            }))
            .send()?;

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

        let token_response: TokenResponse = response.json()?;

        Ok(token_response.input_tokens)
    }

    fn model_context_window(&self) -> usize {
        self.model.context_window()
    }
}
