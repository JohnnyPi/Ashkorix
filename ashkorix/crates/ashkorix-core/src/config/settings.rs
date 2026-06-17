use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    pub context_size: u32,
    pub gpu_layers: u32,
    pub threads: u32,
    pub seed: i32,
    pub stop_sequences: Vec<String>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            max_tokens: 1024,
            context_size: 8192,
            gpu_layers: 0,
            threads: 4,
            seed: -1,
            stop_sequences: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingConfig {
    pub max_tokens: u32,
    pub overlap_tokens: u32,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            overlap_tokens: 64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalConfig {
    pub max_query_variants: u32,
    pub retrieval_context_budget_pct: u32,
    pub candidate_pool_size: usize,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            max_query_variants: 3,
            retrieval_context_budget_pct: 60,
            candidate_pool_size: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub active_project: String,
    pub max_injected: usize,
    pub min_confidence: f64,
    pub min_importance: f64,
    pub extraction_min_confidence: f64,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            active_project: "ashkorix".into(),
            max_injected: 8,
            min_confidence: 0.75,
            min_importance: 0.0,
            extraction_min_confidence: 0.75,
        }
    }
}

impl MemoryConfig {
    pub fn project_scope(&self) -> String {
        format!("project:{}", self.active_project)
    }

    pub fn active_scopes(&self, session_id: &str) -> Vec<String> {
        vec![
            "global".into(),
            self.project_scope(),
            format!("conversation:{session_id}"),
        ]
    }
}
