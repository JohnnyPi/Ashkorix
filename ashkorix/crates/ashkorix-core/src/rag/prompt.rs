use crate::cite::types::SourceBlock;
use crate::rag::types::RankedChunk;
use crate::traits::model::ChatMessage;
use crate::traits::PromptBuilder;

pub struct DefaultPromptBuilder;

impl PromptBuilder for DefaultPromptBuilder {
    fn build_rag_prompt(
        &self,
        question: &str,
        sources: &[SourceBlock],
        conversation: &[ChatMessage],
    ) -> String {
        let mut prompt = String::from(
            "You answer questions using only the provided sources.\n\
             Cite claims with [Source N]. If the sources do not contain the answer, say so clearly.\n\
             Do not invent citations.\n\n",
        );

        if !conversation.is_empty() {
            prompt.push_str("Previous conversation:\n");
            for msg in conversation {
                prompt.push_str(&format!("{}: {}\n", msg.role, msg.content));
            }
            prompt.push('\n');
        }

        prompt.push_str("Sources:\n");
        for source in sources {
            prompt.push_str(&format!("[Source {}]\n", source.number));
            if let Some(ref path) = source.heading_path {
                prompt.push_str(&format!("  Path: {path}\n"));
            }
            prompt.push_str(&format!(
                "  File: {}{}\n",
                source.filename,
                source
                    .page_number
                    .map(|p| format!(" (p.{p})"))
                    .unwrap_or_default()
            ));
            if let Some(ref title) = source.section_title {
                prompt.push_str(&format!("  Heading: {title}\n"));
            }
            if let Some(ref table) = source.table_caption {
                prompt.push_str(&format!("  Table: {table}\n"));
            }
            if let Some(ref conf) = source.confidence {
                prompt.push_str(&format!("  {conf}\n"));
            }
            if let Some(ref neighbor) = source.neighbor_context {
                prompt.push_str(&format!("  Context: {neighbor}\n"));
            }
            prompt.push_str(&format!("  ---\n  {}\n\n", source.preview));
        }

        prompt.push_str(&format!("User question: {question}\n\nAssistant:"));
        prompt
    }
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
                    format!(
                        "rerank_score={s:.3}, matched={}",
                        rc.source_type
                    )
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
        let prompt = builder.build_rag_prompt("What?", &sources, &[]);
        assert!(prompt.contains("[Source 1]"));
        assert!(prompt.contains("Hello world"));
        assert!(prompt.contains("Chapter 1"));
    }
}
