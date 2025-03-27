//! Model definitions and metadata

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
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
        context_window = "200000",
        provider = "anthropic"
    ))]
    Sonnet35,

    #[strum(props(
        model_id = "claude-3-7-sonnet-latest",
        context_window = "200000",
        provider = "anthropic"
    ))]
    Sonnet37,

    // OpenAI models
    #[strum(props(model_id = "gpt-4", context_window = "8192", provider = "openai"))]
    Gpt4,

    #[strum(props(
        model_id = "gpt-4-0125-preview",
        context_window = "128000",
        provider = "openai"
    ))]
    Gpt4Turbo,

    #[strum(props(model_id = "gpt-4o", context_window = "8192", provider = "openai"))]
    Gpt4o,

    // HuggingFace models
    #[strum(props(
        model_id = "meta-llama/Llama-2-7b-hf",
        context_window = "4096",
        provider = "huggingface"
    ))]
    Llama2_7b,

    #[strum(props(
        model_id = "meta-llama/Llama-3-8b-hf",
        context_window = "8192",
        provider = "huggingface"
    ))]
    Llama3_8b,

    #[strum(props(
        model_id = "mistralai/Mistral-Small-3.1-24B-Base-2503",
        context_window = "128000",
        provider = "huggingface"
    ))]
    MistralSmall24B,

    #[strum(props(
        model_id = "mistralai/Mistral-Large-Instruct-2411",
        context_window = "128000",
        provider = "huggingface"
    ))]
    MistralLargeInstruct,

    #[strum(props(
        model_id = "mistralai/Pixtral-12B-Base-2409",
        context_window = "128000",
        provider = "huggingface"
    ))]
    Pixtral12B,

    #[strum(props(
        model_id = "mistralai/Mistral-Small-Instruct-2409",
        context_window = "32000",
        provider = "huggingface"
    ))]
    MistralSmall,
}

impl Model {
    /// Get the context window size for this model
    pub fn context_window(&self) -> usize {
        self.get_int("context_window").unwrap_or(0) as usize
    }

    /// Get the provider of this model
    pub fn provider(&self) -> ModelProvider {
        let provider = self.get_str("provider").unwrap_or("unknown");
        ModelProvider::from_str(provider).unwrap_or(ModelProvider::HuggingFace)
    }

    /// Get the model identifier as used by the provider's API
    pub fn model_id(&self) -> &'static str {
        self.get_str("model_id").unwrap_or("unknown")
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
