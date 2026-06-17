use crate::error::{AshkorixError, Result};
use crate::memory::types::{ExtractedCandidate, MemoryType};

pub fn parse_extraction_response(text: &str) -> Result<Vec<ExtractedCandidate>> {
    let trimmed = strip_code_fences(text.trim());
    let value: serde_json::Value = serde_json::from_str(&trimmed).map_err(|e| {
        AshkorixError::Config(format!("failed to parse memory extraction JSON: {e}"))
    })?;
    let array = value
        .as_array()
        .ok_or_else(|| AshkorixError::Config("memory extraction response must be a JSON array".into()))?;

    let mut candidates = Vec::new();
    for item in array {
        let obj = item.as_object().ok_or_else(|| {
            AshkorixError::Config("each memory candidate must be a JSON object".into())
        })?;
        let type_str = obj
            .get("proposed_type")
            .or_else(|| obj.get("type"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| AshkorixError::Config("candidate missing proposed_type".into()))?;
        let memory_type = MemoryType::from_str(type_str).ok_or_else(|| {
            AshkorixError::Config(format!("invalid memory type: {type_str}"))
        })?;
        candidates.push(ExtractedCandidate {
            proposed_type: memory_type,
            proposed_scope: get_string(obj, "proposed_scope")
                .or_else(|| get_string(obj, "scope"))
                .ok_or_else(|| AshkorixError::Config("candidate missing proposed_scope".into()))?,
            proposed_title: get_string(obj, "proposed_title")
                .or_else(|| get_string(obj, "title"))
                .ok_or_else(|| AshkorixError::Config("candidate missing proposed_title".into()))?,
            proposed_content: get_string(obj, "proposed_content")
                .or_else(|| get_string(obj, "content"))
                .ok_or_else(|| AshkorixError::Config("candidate missing proposed_content".into()))?,
            importance: obj
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5),
            confidence: obj
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.8),
            reason: get_string(obj, "reason"),
        });
    }
    Ok(candidates)
}

pub fn build_extraction_prompt(
    conversation: &str,
    project_scope: &str,
    min_confidence: f64,
) -> String {
    format!(
        "Extract only durable memories from this conversation.\n\
         Return a JSON array of objects with fields:\n\
         proposed_type, proposed_scope, proposed_title, proposed_content, importance, confidence, reason\n\n\
         Allowed types: user_preference, project_fact, decision, procedure\n\
         Reject: temporary details, random facts, one-off questions, sensitive personal information, \
         facts already known, low-confidence guesses (below {min_confidence}).\n\
         Active project scope: {project_scope}\n\
         Also consider global scope for cross-project preferences and procedures.\n\
         Each memory should contain one idea only. Keep content short.\n\
         Return JSON only, no markdown fences.\n\n\
         Conversation:\n{conversation}"
    )
}

fn strip_code_fences(text: &str) -> String {
    if text.starts_with("```") {
        let without_start = text.trim_start_matches('`');
        let inner = without_start
            .trim_start_matches("json")
            .trim_start_matches('\n');
        if let Some(end) = inner.rfind("```") {
            return inner[..end].trim().to_string();
        }
    }
    text.to_string()
}

fn get_string(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    obj.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::MemoryType;

    #[test]
    fn parses_json_array() {
        let json = r#"[
            {
                "proposed_type": "project_fact",
                "proposed_scope": "project:ashkorix",
                "proposed_title": "Local-first",
                "proposed_content": "Ashkorix is local-first.",
                "importance": 0.9,
                "confidence": 1.0
            }
        ]"#;
        let parsed = parse_extraction_response(json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].proposed_type, MemoryType::ProjectFact);
    }

    #[test]
    fn strips_markdown_fences() {
        let json = "```json\n[{\"proposed_type\":\"decision\",\"proposed_scope\":\"global\",\"proposed_title\":\"T\",\"proposed_content\":\"C\",\"importance\":0.8,\"confidence\":0.9}]\n```";
        let parsed = parse_extraction_response(json).unwrap();
        assert_eq!(parsed.len(), 1);
    }
}
