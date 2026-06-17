use crate::cite::types::SourceBlock;
use crate::memory::format::{augment_last_user_message_for_rag, format_memory_block};
use crate::memory::types::Memory;
use crate::rag::types::RankedChunk;
use crate::traits::model::ChatMessage;
use crate::traits::PromptBuilder;

const RAG_INSTRUCTIONS: &str = "You are a document Q&A assistant grounded in the material below.\n\
Rules:\n\
1. AUTHORITATIVE MEMORY (if present) overrides training data. Use it when it answers the question; cite as [Memory N].\n\
2. For document facts, answer ONLY using the numbered sources below; cite as [Source N].\n\
3. Do not use outside knowledge, training data, or guesswork when memory or sources apply.\n\
4. If neither memory nor sources answer the question, reply exactly: \"The provided sources do not contain enough information to answer that.\"\n\
5. Every factual statement must cite [Memory N] or [Source N] immediately after the claim it supports.\n\
6. Do not attach citations to claims that are not present in that memory or source text.\n\
7. Do not invent names, places, descriptions, or facts not in memory or sources.\n\n";

pub struct DefaultPromptBuilder;

impl PromptBuilder for DefaultPromptBuilder {
    fn build_rag_prompt(
        &self,
        question: &str,
        sources: &[SourceBlock],
        conversation: &[ChatMessage],
        memories: &[Memory],
    ) -> String {
        let messages = build_rag_messages(question, sources, conversation, memories);
        messages_to_debug_prompt(&messages)
    }
}

pub fn build_rag_messages(
    question: &str,
    sources: &[SourceBlock],
    conversation: &[ChatMessage],
    memories: &[Memory],
) -> Vec<ChatMessage> {
    let mut system = String::from(RAG_INSTRUCTIONS);

    let memory_block = format_memory_block(memories);
    if !memory_block.is_empty() {
        system.push_str(&memory_block);
        system.push('\n');
    }

    system.push_str(&format_sources_block(sources));

    let mut messages = vec![ChatMessage {
        role: "system".into(),
        content: system,
    }];

    if conversation.is_empty() {
        messages.push(ChatMessage {
            role: "user".into(),
            content: question.to_string(),
        });
    } else {
        messages.extend(conversation.iter().cloned());
    }

    augment_last_user_message_for_rag(&mut messages, memories);
    messages
}

fn format_sources_block(sources: &[SourceBlock]) -> String {
    let mut block = String::from("=== SOURCES ===\n");
    for source in sources {
        block.push_str(&format!("[Source {}]\n", source.number));
        if let Some(ref path) = source.heading_path {
            block.push_str(&format!("  Path: {path}\n"));
        }
        block.push_str(&format!(
            "  File: {}{}\n",
            source.filename,
            source
                .page_number
                .map(|p| format!(" (p.{p})"))
                .unwrap_or_default()
        ));
        if let Some(ref title) = source.section_title {
            block.push_str(&format!("  Heading: {title}\n"));
        }
        if let Some(ref table) = source.table_caption {
            block.push_str(&format!("  Table: {table}\n"));
        }
        if let Some(ref conf) = source.confidence {
            block.push_str(&format!("  {conf}\n"));
        }
        if let Some(ref neighbor) = source.neighbor_context {
            block.push_str(&format!("  Context: {neighbor}\n"));
        }
        block.push_str(&format!("  ---\n  {}\n\n", source.preview));
    }
    block
}

fn messages_to_debug_prompt(messages: &[ChatMessage]) -> String {
    let mut prompt = String::new();
    for msg in messages {
        prompt.push_str(&format!("{}: {}\n\n", msg.role, msg.content));
    }
    prompt.push_str("Assistant:");
    prompt
}

pub fn build_source_blocks(chunks: &[RankedChunk]) -> Vec<SourceBlock> {
    chunks
        .iter()
        .enumerate()
        .map(|(i, rc)| {
            let expanded = rc.expanded_context.clone();
            SourceBlock {
                number: (i + 1) as u32,
                filename: rc.chunk.source_filename.clone(),
                page_number: rc.chunk.page_number,
                section_title: rc.chunk.section_title.clone(),
                heading_path: rc.chunk.heading_path.clone(),
                table_caption: rc.chunk.row_sheet_info.clone(),
                neighbor_context: expanded,
                confidence: rc.rerank_score.map(|s| {
                    format!("rerank_score={s:.3}, matched={}", rc.source_type)
                }),
                preview: rc.chunk.text.chars().take(500).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_prompt_with_sources() {
        let builder = DefaultPromptBuilder;
        let sources = vec![SourceBlock {
            number: 1,
            filename: "test.txt".into(),
            page_number: None,
            section_title: None,
            heading_path: Some("Chapter 1".into()),
            table_caption: None,
            neighbor_context: None,
            confidence: None,
            preview: "Hello world".into(),
        }];
        let prompt = builder.build_rag_prompt("What?", &sources, &[], &[]);
        assert!(prompt.contains("[Source 1]"));
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("Chapter 1"));
        assert!(prompt.contains("=== SOURCES ==="));
        assert!(prompt.contains("AUTHORITATIVE MEMORY"));
    }

    #[test]
    fn build_rag_messages_includes_memory_block() {
        use crate::memory::types::{Memory, MemoryStatus, MemoryType};
        use chrono::Utc;

        let now = Utc::now();
        let memories = vec![Memory {
            id: "m1".into(),
            memory_type: MemoryType::ProjectFact,
            scope: "project:ashkorix".into(),
            title: "What Ashkorix is".into(),
            content: "Ashkorix is intended to be a local-first GGUF model runner.".into(),
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
        }];
        let messages = build_rag_messages("What is Ashkorix?", &[], &[], &memories);
        assert!(messages[0].content.contains("=== AUTHORITATIVE MEMORY ==="));
        assert!(messages[0].content.contains("[Memory 1]"));
        assert!(messages[0].content.contains("local-first GGUF model runner"));
    }

    #[test]
    fn build_rag_messages_uses_system_role() {
        let messages = build_rag_messages(
            "What is X?",
            &[SourceBlock {
                number: 1,
                filename: "a.md".into(),
                page_number: None,
                section_title: None,
                heading_path: None,
                table_caption: None,
                neighbor_context: None,
                confidence: None,
                preview: "X is defined here.".into(),
            }],
            &[],
            &[],
        );
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("=== SOURCES ==="));
        assert_eq!(messages[1].role, "user");
        assert!(messages[1].content.contains("[Source N]"));
        assert!(!messages[1].content.contains("Answer ONLY from the authoritative memory"));
    }

    #[test]
    fn build_rag_messages_user_turn_reinforces_sources_with_memory() {
        use crate::memory::types::{Memory, MemoryStatus, MemoryType};
        use chrono::Utc;

        let now = Utc::now();
        let memories = vec![Memory {
            id: "m1".into(),
            memory_type: MemoryType::ProjectFact,
            scope: "project:ashkorix".into(),
            title: "Scope".into(),
            content: "Project uses local models.".into(),
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
        }];
        let messages = build_rag_messages(
            "What is Phase 1?",
            &[SourceBlock {
                number: 1,
                filename: "plan.md".into(),
                page_number: None,
                section_title: Some("Phase 1".into()),
                heading_path: None,
                table_caption: None,
                neighbor_context: None,
                confidence: None,
                preview: "Phase 1 covers ingestion.".into(),
            }],
            &[],
            &memories,
        );
        let user = messages.last().unwrap();
        assert!(user.content.contains("[Source N]"));
        assert!(user.content.contains("[Memory N]"));
        assert!(!user.content.contains("Answer ONLY from the authoritative memory"));
    }
}
