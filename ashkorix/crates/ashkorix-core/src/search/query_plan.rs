use crate::documents::storage::DocumentStore;
use crate::error::Result;
use crate::rag::types::RetrievalMode;

#[derive(Debug, Clone)]
pub struct QueryPlan {
    pub variants: Vec<String>,
}

pub struct QueryPlanner {
    store: std::sync::Arc<DocumentStore>,
}

impl QueryPlanner {
    pub fn new(store: std::sync::Arc<DocumentStore>) -> Self {
        Self { store }
    }

    pub fn plan(&self, query: &str, mode: RetrievalMode) -> Result<QueryPlan> {
        let (_, _, _, _, max_variants, _) = mode.params();
        let mut variants = vec![query.to_string()];

        if let Some(keyword) = extract_keyword_query(query) {
            if !variants.contains(&keyword) {
                variants.push(keyword);
            }
        }

        if let Ok(expansions) = self.store.lookup_entity_expansions(query) {
            for exp in expansions.into_iter().take(3) {
                if !variants.contains(&exp) {
                    variants.push(exp);
                }
            }
        }

        if max_variants >= 3 {
            if let Some(acronym) = expand_acronyms(query) {
                if !variants.contains(&acronym) {
                    variants.push(acronym);
                }
            }
        }

        if max_variants >= 5 {
            for sub in decompose_subquestions(query) {
                if !variants.contains(&sub) {
                    variants.push(sub);
                }
            }
        }

        variants.truncate(max_variants);
        Ok(QueryPlan { variants })
    }
}

fn extract_keyword_query(query: &str) -> Option<String> {
    let mut parts = Vec::new();
    if let Ok(re) = regex::Regex::new(r#""([^"]+)""#) {
        for cap in re.captures_iter(query) {
            parts.push(cap.get(1)?.as_str().to_string());
        }
    }
    for pattern in [
        r"(?i)\bphase\s+\d+\b",
        r"\bfield\s+\d+\b",
        r"\bField\s+\d+\b",
        r"\berror\s+\d+\b",
        r"\bError\s+\d+\b",
        r"\b[A-Z]{2,6}\b",
        r"\b\d{5,}\b",
    ] {
        if let Ok(re) = regex::Regex::new(pattern) {
            for m in re.find_iter(query) {
                parts.push(m.as_str().to_string());
            }
        }
    }
    let stopwords = [
        "the", "a", "an", "is", "are", "what", "does", "how", "where", "when", "this", "that",
        "about", "say", "says",
    ];
    for word in query.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if w.len() > 2 && !stopwords.contains(&w.as_str()) {
            parts.push(w);
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn expand_acronyms(query: &str) -> Option<String> {
    let expansions: &[(&str, &str)] = &[
        ("compliance", "verification assessment traceability control objective shall"),
        ("security", "authentication authorization encryption access control"),
        ("error", "fault failure exception code"),
    ];
    let lower = query.to_lowercase();
    for (term, expansion) in expansions {
        if lower.contains(term) {
            return Some(format!("{query} {expansion}"));
        }
    }
    None
}

fn decompose_subquestions(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();
    let mut subs = Vec::new();
    if lower.contains(" and ") {
        for part in query.split(" and ") {
            let p = part.trim();
            if p.len() > 5 {
                subs.push(p.to_string());
            }
        }
    }
    if subs.is_empty() && query.len() > 10 {
        subs.push(format!("What is {query}?"));
        subs.push(format!("Where is {query} documented?"));
    }
    subs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_query_includes_phase_number() {
        let kw = extract_keyword_query("What is Phase 1?").expect("keywords");
        assert!(kw.to_lowercase().contains("phase 1"));
    }
}
