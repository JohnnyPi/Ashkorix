use crate::documents::graph_types::{Entity, EntityKind};
use crate::types::{hash_text, short_id_from_hash};

pub fn extract_entities_from_text(document_id: &str, chunk_id: &str, text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();
    let patterns: &[(&str, EntityKind)] = &[
        (r"\b[A-Z]{2,6}\b", EntityKind::Acronym),
        (r"\bfield\s+\d+\b", EntityKind::FieldId),
        (r"\bField\s+\d+\b", EntityKind::FieldId),
        (r"\berror\s+\d+\b", EntityKind::ErrorCode),
        (r"\bError\s+\d+\b", EntityKind::ErrorCode),
        (r"\b\d{5,}\b", EntityKind::PartNumber),
    ];

    for (pattern, kind) in patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            for m in re.find_iter(text) {
                let value = m.as_str().to_string();
                let id = format!(
                    "ent-{}",
                    short_id_from_hash(&hash_text(&format!("{document_id}{chunk_id}{value}")))
                );
                if !entities.iter().any(|e: &Entity| e.value == value) {
                    entities.push(Entity {
                        id,
                        document_id: document_id.to_string(),
                        chunk_id: Some(chunk_id.to_string()),
                        kind: kind.clone(),
                        value,
                    });
                }
            }
        }
    }
    entities
}
