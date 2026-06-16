use crate::cite::types::{Citation, CitationParseResult};
use crate::rag::types::RankedChunk;
use crate::traits::CitationService;

pub struct DefaultCitationService;

impl CitationService for DefaultCitationService {
    fn assemble_sources(&self, chunks: &[RankedChunk], collection_name: &str) -> Vec<Citation> {
        chunks
            .iter()
            .enumerate()
            .map(|(i, rc)| Citation {
                source_number: (i + 1) as u32,
                document_id: rc.chunk.document_id.0.clone(),
                original_filename: rc.chunk.source_filename.clone(),
                page_number: rc.chunk.page_number,
                section_title: rc.chunk.section_title.clone(),
                chunk_preview: rc.chunk.text.chars().take(500).collect(),
                score: rc.score,
                collection_name: collection_name.to_string(),
            })
            .collect()
    }

    fn parse_markers(&self, response: &str, citations: &[Citation]) -> CitationParseResult {
        let re = regex::Regex::new(r"\[Source\s+(\d+)\]").unwrap();
        let mut found_numbers = std::collections::HashSet::new();
        for cap in re.captures_iter(response) {
            if let Some(m) = cap.get(1) {
                if let Ok(n) = m.as_str().parse::<u32>() {
                    found_numbers.insert(n);
                }
            }
        }

        let resolved: Vec<Citation> = citations
            .iter()
            .filter(|c| found_numbers.contains(&c.source_number))
            .cloned()
            .collect();

        let valid_numbers: std::collections::HashSet<u32> =
            citations.iter().map(|c| c.source_number).collect();
        let dangling: Vec<u32> = found_numbers
            .iter()
            .filter(|n| !valid_numbers.contains(n))
            .copied()
            .collect();

        let uncited_warning = !citations.is_empty() && found_numbers.is_empty();

        CitationParseResult {
            resolved,
            dangling,
            uncited_warning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_citation_markers() {
        let svc = DefaultCitationService;
        let citations = vec![Citation {
            source_number: 1,
            document_id: "doc1".into(),
            original_filename: "a.txt".into(),
            page_number: None,
            section_title: None,
            chunk_preview: "text".into(),
            score: 1.0,
            collection_name: "default".into(),
        }];
        let result = svc.parse_markers("Answer based on [Source 1].", &citations);
        assert_eq!(result.resolved.len(), 1);
        assert!(result.dangling.is_empty());
    }
}
