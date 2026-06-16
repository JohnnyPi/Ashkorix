use crate::rag::types::{UnsupportedClaim, RankedChunk};
use crate::cite::types::Citation;

pub struct CitationVerifier;

impl CitationVerifier {
    pub fn verify(
        answer_text: &str,
        citations: &[Citation],
        chunks: &[RankedChunk],
    ) -> Vec<UnsupportedClaim> {
        let mut unsupported = Vec::new();
        let sentences: Vec<&str> = answer_text
            .split(|c| c == '.' || c == '!' || c == '?')
            .map(str::trim)
            .filter(|s| s.len() > 15)
            .collect();

        for sentence in sentences {
            if !sentence.contains("[Source") {
                continue;
            }
            let cited_nums: Vec<u32> = regex::Regex::new(r"\[Source\s+(\d+)\]")
                .ok()
                .map(|re| {
                    re.captures_iter(sentence)
                        .filter_map(|c| c.get(1)?.as_str().parse().ok())
                        .collect()
                })
                .unwrap_or_default();

            for num in cited_nums {
                let claim = sentence
                    .replace(&format!("[Source {num}]"), "")
                    .trim()
                    .to_string();
                if claim.len() < 10 {
                    continue;
                }
                if !citations.iter().any(|c| c.source_number == num) {
                    unsupported.push(UnsupportedClaim {
                        sentence: claim.clone(),
                        cited_source: Some(num),
                        reason: "Citation number not in resolved list".into(),
                    });
                    continue;
                }
                let Some(chunk) = chunks
                    .iter()
                    .find(|c| c.source_number == Some(num))
                else {
                    unsupported.push(UnsupportedClaim {
                        sentence: claim.clone(),
                        cited_source: Some(num),
                        reason: "Source chunk not found in retrieval set".into(),
                    });
                    continue;
                };

                if !Self::claim_supported(&claim, &chunk.chunk.text) {
                    unsupported.push(UnsupportedClaim {
                        sentence: claim,
                        cited_source: Some(num),
                        reason: "Claim lacks sufficient lexical overlap with cited source".into(),
                    });
                }
            }
        }
        unsupported
    }

    fn claim_supported(claim: &str, source_text: &str) -> bool {
        let claim_lower = claim.to_lowercase();
        let source_lower = source_text.to_lowercase();
        let terms: Vec<&str> = claim_lower
            .split_whitespace()
            .filter(|t| t.len() > 4)
            .collect();
        if terms.is_empty() {
            return true;
        }
        let matched = terms.iter().filter(|t| source_lower.contains(*t)).count();
        matched as f64 / terms.len() as f64 >= 0.3
    }
}
