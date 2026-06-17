use crate::memory::types::{Memory, MemoryType};
use crate::traits::model::TokenEvent;
use futures::stream::{self, Stream};
use std::pin::Pin;

const LOOKUP_STOP_WORDS: &[&str] = &[
    "what", "is", "the", "a", "an", "about", "tell", "me", "who", "define", "describe", "this",
    "that", "project", "please", "explain",
];

/// When injected memories directly answer a lookup question, return a deterministic
/// response instead of calling the LLM (local models often ignore system prompts).
pub fn try_direct_memory_answer(query: &str, memories: &[Memory]) -> Option<String> {
    if memories.is_empty() || !is_lookup_query(query) {
        return None;
    }

    for (i, mem) in memories.iter().enumerate() {
        if !matches!(mem.memory_type, MemoryType::ProjectFact | MemoryType::Decision) {
            continue;
        }
        if memory_answers_lookup(query, mem) {
            return Some(format!("{} [Memory {}]", mem.content.trim(), i + 1));
        }
    }

    None
}

pub fn instant_text_stream(text: String) -> Pin<Box<dyn Stream<Item = crate::error::Result<TokenEvent>> + Send>> {
    let events = vec![
        Ok(TokenEvent {
            token: text,
            finished: false,
            tokens_generated: 0,
        }),
        Ok(TokenEvent {
            token: String::new(),
            finished: true,
            tokens_generated: 1,
        }),
    ];
    Box::pin(stream::iter(events))
}

fn is_lookup_query(query: &str) -> bool {
    let q = query.to_lowercase();
    q.contains("what is")
        || q.contains("what's")
        || q.contains("who is")
        || q.contains("tell me about")
        || q.contains("describe ")
        || q.starts_with("define ")
}

fn memory_answers_lookup(query: &str, memory: &Memory) -> bool {
    let q = query.to_lowercase();
    let content = memory.content.to_lowercase();
    let title = memory.title.to_lowercase();

    if let Some(subject) = lookup_subject(&q) {
        if subject.len() >= 3 && (content.contains(&subject) || title.contains(&subject)) {
            return true;
        }
    }

    let terms = significant_terms(&q);
    if terms.is_empty() {
        return false;
    }

    terms
        .iter()
        .any(|term| content.contains(term) || title.contains(term))
}

fn lookup_subject(q: &str) -> Option<String> {
    for prefix in [
        "what is ",
        "what's ",
        "who is ",
        "tell me about ",
        "define ",
        "describe ",
    ] {
        if let Some(rest) = q.strip_prefix(prefix) {
            return Some(normalize_subject(rest));
        }
        if let Some(pos) = q.find(prefix) {
            let rest = &q[pos + prefix.len()..];
            return Some(normalize_subject(rest));
        }
    }
    None
}

fn normalize_subject(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c == '?' || c == '.' || c == '!')
        .to_lowercase()
}

fn significant_terms(q: &str) -> Vec<String> {
    q.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| w.len() > 2 && !LOOKUP_STOP_WORDS.contains(&w.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::{Memory, MemoryStatus};
    use chrono::Utc;

    fn fact_memory(content: &str) -> Memory {
        let now = Utc::now();
        Memory {
            id: "m1".into(),
            memory_type: MemoryType::ProjectFact,
            scope: "project:ashkorix".into(),
            title: "Ashkorix is local-first".into(),
            content: content.into(),
            importance: 0.9,
            confidence: 1.0,
            status: MemoryStatus::Active,
            source_type: None,
            source_ref: None,
            created_at: now,
            updated_at: now,
            last_used_at: None,
            supersedes_id: None,
            metadata_json: None,
        }
    }

    #[test]
    fn answers_what_is_from_project_fact() {
        let memories = vec![fact_memory(
            "Ashkorix is intended to be a local-first GGUF model runner.",
        )];
        let answer = try_direct_memory_answer("What is Ashkorix?", &memories);
        assert!(answer.is_some());
        let answer = answer.unwrap();
        assert!(answer.contains("local-first GGUF model runner"));
        assert!(answer.contains("[Memory 1]"));
    }

    #[test]
    fn skips_non_lookup_questions() {
        let memories = vec![fact_memory("Ashkorix is local-first.")];
        assert!(try_direct_memory_answer("How do I build the index?", &memories).is_none());
    }

    #[test]
    fn skips_preference_memories_for_lookup() {
        let now = Utc::now();
        let memories = vec![Memory {
            id: "m2".into(),
            memory_type: MemoryType::UserPreference,
            scope: "global".into(),
            title: "Style".into(),
            content: "The user prefers practical details.".into(),
            importance: 0.8,
            confidence: 1.0,
            status: MemoryStatus::Active,
            source_type: None,
            source_ref: None,
            created_at: now,
            updated_at: now,
            last_used_at: None,
            supersedes_id: None,
            metadata_json: None,
        }];
        assert!(try_direct_memory_answer("What is Ashkorix?", &memories).is_none());
    }
}
