use crate::cite::types::Citation;
use crate::memory::format::memory_for_number;
use crate::memory::types::Memory;
use crate::rag::types::{RankedChunk, UnsupportedClaim};

pub struct CitationVerifier;

impl CitationVerifier {
    pub fn verify(
        answer_text: &str,
        citations: &[Citation],
        chunks: &[RankedChunk],
        memories: &[Memory],
    ) -> Vec<UnsupportedClaim> {
        let mut unsupported = Self::verify_source_claims(answer_text, citations, chunks);
        unsupported.extend(Self::verify_memory_claims(answer_text, memories));
        unsupported
    }

    fn verify_source_claims(
        answer_text: &str,
        citations: &[Citation],
        chunks: &[RankedChunk],
    ) -> Vec<UnsupportedClaim> {
        let mut unsupported = Vec::new();
        let cited_claims = Self::extract_marked_claims(answer_text, r"\[Source\s+(\d+)\]");

        for (claim, num) in cited_claims {
            if !citations.iter().any(|c| c.source_number == num) {
                unsupported.push(UnsupportedClaim {
                    sentence: claim.clone(),
                    cited_source: Some(num),
                    reason: "Citation number not in resolved list".into(),
                });
                continue;
            }

            let Some(chunk) = Self::chunk_for_source(chunks, num) else {
                unsupported.push(UnsupportedClaim {
                    sentence: claim.clone(),
                    cited_source: Some(num),
                    reason: "Source chunk not found in retrieval set".into(),
                });
                continue;
            };

            if !Self::claim_supported(&claim, &Self::source_corpus(chunk)) {
                unsupported.push(UnsupportedClaim {
                    sentence: claim,
                    cited_source: Some(num),
                    reason: "Claim lacks sufficient lexical overlap with cited source".into(),
                });
            }
        }

        unsupported
    }

    fn verify_memory_claims(answer_text: &str, memories: &[Memory]) -> Vec<UnsupportedClaim> {
        let mut unsupported = Vec::new();
        let cited_claims = Self::extract_marked_claims(answer_text, r"\[Memory\s+(\d+)\]");

        for (claim, num) in cited_claims {
            let Some(memory) = memory_for_number(memories, num) else {
                unsupported.push(UnsupportedClaim {
                    sentence: claim.clone(),
                    cited_source: Some(num),
                    reason: "Memory number not in injected set".into(),
                });
                continue;
            };

            if !Self::claim_supported(&claim, &memory.content) {
                unsupported.push(UnsupportedClaim {
                    sentence: claim,
                    cited_source: Some(num),
                    reason: "Claim lacks sufficient lexical overlap with cited memory".into(),
                });
            }
        }

        unsupported
    }

    fn extract_marked_claims(text: &str, marker_pattern: &str) -> Vec<(String, u32)> {
        let re = match regex::Regex::new(marker_pattern) {
            Ok(re) => re,
            Err(_) => return Vec::new(),
        };

        let mut claims = Vec::new();
        let mut last_end = 0;
        for cap in re.captures_iter(text) {
            let Some(full) = cap.get(0) else {
                continue;
            };
            let Ok(num) = cap[1].parse::<u32>() else {
                continue;
            };
            let claim = text[last_end..full.start()].trim();
            if claim.len() >= 10 {
                claims.push((claim.to_string(), num));
            }
            last_end = full.end();
        }
        claims
    }

    fn chunk_for_source(chunks: &[RankedChunk], num: u32) -> Option<&RankedChunk> {
        chunks
            .iter()
            .find(|c| c.source_number == Some(num))
            .or_else(|| chunks.get(num as usize - 1))
    }

    fn source_corpus(chunk: &RankedChunk) -> String {
        let mut parts = vec![chunk.chunk.text.clone()];
        if let Some(ref ctx) = chunk.expanded_context {
            parts.push(ctx.clone());
        }
        parts.join("\n")
    }

    fn claim_supported(claim: &str, source_text: &str) -> bool {
        let claim_lower = claim.to_lowercase();
        let source_lower = source_text.to_lowercase();
        let terms: Vec<&str> = claim_lower
            .split_whitespace()
            .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|t| t.len() > 4)
            .collect();
        if terms.is_empty() {
            return true;
        }
        let matched = terms.iter().filter(|t| source_lower.contains(*t)).count();
        matched as f64 / terms.len() as f64 >= 0.35
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::types::Chunk;
    use crate::types::{ChunkId, CollectionId, DocumentId};

    fn scope_chunk() -> RankedChunk {
        RankedChunk {
            chunk: Chunk {
                id: ChunkId("c1".into()),
                document_id: DocumentId("doc1".into()),
                collection_id: CollectionId("pool".into()),
                text: "The one sentence that defines done: upload a file, ask a local model questions, get answers with [Source N] citations, accurately.".into(),
                start_offset: 0,
                end_offset: 100,
                page_number: None,
                section_title: Some("1. Scope for this milestone".into()),
                row_sheet_info: None,
                source_filename: "plan.md".into(),
                content_hash: "h1".into(),
                token_count: 20,
                parent_section_id: None,
                heading_path: Some("1. Scope for this milestone".into()),
                chunk_index: 0,
                prev_chunk_id: None,
                next_chunk_id: None,
                contextual_text: None,
                table_id: None,
                entity_tokens: None,
            },
            score: 1.0,
            source_type: "both".into(),
            source_number: Some(1),
            rerank_score: Some(-6.345),
            expanded_context: None,
        }
    }

    #[test]
    fn flags_hallucination_with_trailing_citation() {
        let chunks = vec![scope_chunk()];
        let answer = "Ashkorix is a fictional land mentioned in the document you uploaded. \
                      It is described as a lush green valley with rolling hills. [Source 1]";
        let unsupported = CitationVerifier::verify(answer, &[], &chunks, &[]);
        assert_eq!(unsupported.len(), 1);
        assert!(unsupported[0].sentence.contains("Ashkorix"));
    }

    #[test]
    fn accepts_claim_grounded_in_source() {
        let chunks = vec![scope_chunk()];
        let citations = vec![Citation {
            source_number: 1,
            document_id: "doc1".into(),
            original_filename: "plan.md".into(),
            page_number: None,
            section_title: None,
            chunk_preview: chunks[0].chunk.text.chars().take(200).collect(),
            score: 1.0,
            collection_name: "default".into(),
        }];
        let answer = "Done means uploading a file and getting cited answers from a local model. [Source 1]";
        let unsupported = CitationVerifier::verify(answer, &citations, &chunks, &[]);
        assert!(unsupported.is_empty());
    }
}
