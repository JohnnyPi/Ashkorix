use crate::memory::types::{Memory, MemoryType};

const MEMORY_BLOCK_HEADER: &str = "=== AUTHORITATIVE MEMORY ===\n\
These are verified facts about this user/project. They override your training data when they conflict.\n\
When your answer uses a memory, cite it as [Memory N].\n\n";

const CHAT_MEMORY_INSTRUCTIONS: &str = "You are Ashkorix, a local assistant.\n\
Rules:\n\
1. When authoritative memory is provided below, treat it as ground truth about this user/project.\n\
2. Do not contradict memory with training data or general world knowledge.\n\
3. If memory answers the question, use it and cite as [Memory N].\n\
4. If you lack relevant memory and the user asks about their project, say you do not have that information.\n\n";

pub fn format_memory_block(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut block = String::from(MEMORY_BLOCK_HEADER);
    for (i, mem) in memories.iter().enumerate() {
        block.push_str(&format!(
            "[Memory {}] [{}] {}\n",
            i + 1,
            mem.memory_type.label(),
            mem.content
        ));
    }
    block
}

pub fn build_chat_memory_system_prompt(memories: &[Memory]) -> String {
    let memory_block = format_memory_block(memories);
    if memory_block.is_empty() {
        return String::new();
    }
    format!("{CHAT_MEMORY_INSTRUCTIONS}{memory_block}")
}

pub fn memory_for_number(memories: &[Memory], num: u32) -> Option<&Memory> {
    memories.get(num as usize - 1)
}

/// Local GGUF models often weight the latest user turn more than system prompts.
pub fn augment_user_message_with_memory(query: &str, memories: &[Memory]) -> String {
    let memory_block = format_memory_block(memories);
    if memory_block.is_empty() {
        return query.to_string();
    }
    format!(
        "{memory_block}\
         CRITICAL: Answer ONLY from the authoritative memory above. \
         Do NOT use training data or general world knowledge.\n\n\
         Question: {query}"
    )
}

pub fn augment_last_user_message(messages: &mut [crate::traits::model::ChatMessage], memories: &[Memory]) {
    if memories.is_empty() {
        return;
    }
    if let Some(last) = messages.iter_mut().rev().find(|m| m.role == "user") {
        last.content = augment_user_message_with_memory(&last.content, memories);
    }
}

/// RAG prompts already include memory and sources in the system message; reinforce both
/// in the user turn without the chat-only "memory only" restriction.
pub fn augment_user_message_for_rag(query: &str, memories: &[Memory]) -> String {
    let memory_line = if memories.is_empty() {
        String::new()
    } else {
        "Use authoritative memory when it applies; cite as [Memory N].\n".to_string()
    };
    format!(
        "{memory_line}\
         Use the numbered SOURCES from the system message for document facts; cite as [Source N].\n\
         Do not answer from training data or guesswork when memory or sources apply.\n\
         If neither memory nor sources answer the question, reply exactly: \
         \"The provided sources do not contain enough information to answer that.\"\n\n\
         Question: {query}"
    )
}

pub fn augment_last_user_message_for_rag(
    messages: &mut [crate::traits::model::ChatMessage],
    memories: &[Memory],
) {
    if let Some(last) = messages.iter_mut().rev().find(|m| m.role == "user") {
        last.content = augment_user_message_for_rag(&last.content, memories);
    }
}

pub fn type_relevance_score(memory_type: MemoryType, query: &str) -> f64 {
    let q = query.to_lowercase();
    match memory_type {
        MemoryType::Procedure => {
            if q.contains("plan")
                || q.contains("implement")
                || q.contains("phase")
                || q.contains("build")
                || q.contains("architecture")
            {
                1.0
            } else {
                0.5
            }
        }
        MemoryType::Decision => {
            if q.contains("decid")
                || q.contains("chose")
                || q.contains("chosen")
                || q.contains("should we")
            {
                1.0
            } else {
                0.55
            }
        }
        MemoryType::ProjectFact => {
            if q.contains("project")
                || q.contains("ashkorix")
                || q.contains("what is")
                || q.contains("about")
            {
                0.9
            } else {
                0.6
            }
        }
        MemoryType::UserPreference => 0.65,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::{Memory, MemoryStatus, MemoryType};
    use chrono::Utc;

    fn sample_memory(content: &str, memory_type: MemoryType) -> Memory {
        let now = Utc::now();
        Memory {
            id: "mem_test".into(),
            memory_type,
            scope: "global".into(),
            title: "Test".into(),
            content: content.into(),
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
        }
    }

    #[test]
    fn formats_memory_block() {
        let memories = vec![sample_memory(
            "Ashkorix is local-first.",
            MemoryType::ProjectFact,
        )];
        let block = format_memory_block(&memories);
        assert!(block.contains("=== AUTHORITATIVE MEMORY ==="));
        assert!(block.contains("[Memory 1]"));
        assert!(block.contains("[Project Fact]"));
        assert!(block.contains("Ashkorix is local-first."));
    }

    #[test]
    fn empty_memories_returns_empty_string() {
        assert!(format_memory_block(&[]).is_empty());
    }

    #[test]
    fn chat_prompt_includes_grounding_rules() {
        let memories = vec![sample_memory(
            "Ashkorix is local-first.",
            MemoryType::ProjectFact,
        )];
        let prompt = build_chat_memory_system_prompt(&memories);
        assert!(prompt.contains("Do not contradict memory"));
        assert!(prompt.contains("[Memory 1]"));
    }
}
