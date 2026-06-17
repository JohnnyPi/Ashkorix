use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    UserPreference,
    ProjectFact,
    Decision,
    Procedure,
}

impl MemoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UserPreference => "user_preference",
            Self::ProjectFact => "project_fact",
            Self::Decision => "decision",
            Self::Procedure => "procedure",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user_preference" => Some(Self::UserPreference),
            "project_fact" => Some(Self::ProjectFact),
            "decision" => Some(Self::Decision),
            "procedure" => Some(Self::Procedure),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::UserPreference => "User Preference",
            Self::ProjectFact => "Project Fact",
            Self::Decision => "Decision",
            Self::Procedure => "Procedure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Active,
    Inactive,
    Superseded,
    Deleted,
}

impl MemoryStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Superseded => "superseded",
            Self::Deleted => "deleted",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "inactive" => Some(Self::Inactive),
            "superseded" => Some(Self::Superseded),
            "deleted" => Some(Self::Deleted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateStatus {
    Pending,
    Approved,
    Rejected,
    Edited,
}

impl CandidateStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Edited => "edited",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "edited" => Some(Self::Edited),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub memory_type: MemoryType,
    pub scope: String,
    pub title: String,
    pub content: String,
    pub importance: f64,
    pub confidence: f64,
    pub status: MemoryStatus,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub supersedes_id: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub id: String,
    pub proposed_type: MemoryType,
    pub proposed_scope: String,
    pub proposed_title: String,
    pub proposed_content: String,
    pub importance: f64,
    pub confidence: f64,
    pub reason: Option<String>,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub created_at: DateTime<Utc>,
    pub status: CandidateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMemoryInput {
    pub memory_type: MemoryType,
    pub scope: String,
    pub title: String,
    pub content: String,
    pub importance: f64,
    pub confidence: f64,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub supersedes_id: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMemoryInput {
    pub memory_type: Option<MemoryType>,
    pub scope: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub importance: Option<f64>,
    pub confidence: Option<f64>,
    pub status: Option<MemoryStatus>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditCandidateInput {
    pub proposed_type: Option<MemoryType>,
    pub proposed_scope: Option<String>,
    pub proposed_title: Option<String>,
    pub proposed_content: Option<String>,
    pub importance: Option<f64>,
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCandidate {
    pub proposed_type: MemoryType,
    pub proposed_scope: String,
    pub proposed_title: String,
    pub proposed_content: String,
    pub importance: f64,
    pub confidence: f64,
    pub reason: Option<String>,
}

pub fn normalize_content(content: &str) -> String {
    content.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn memory_cache_key(content: &str) -> String {
    format!("mem:{}", crate::types::hash_text(content))
}
