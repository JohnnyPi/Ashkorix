use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoadOptions {
    pub n_ctx: u32,
    pub n_gpu_layers: u32,
    pub threads: u32,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            n_ctx: 8192,
            n_gpu_layers: 0,
            threads: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub path: PathBuf,
    pub filename: String,
    pub n_ctx: u32,
    pub architecture: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateParams {
    pub prompt: String,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    pub seed: i32,
    pub stop_sequences: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEvent {
    pub token: String,
    pub finished: bool,
    pub tokens_generated: u32,
}
