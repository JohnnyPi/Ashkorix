use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    Vector,
    Keyword,
    Both,
}

pub fn reciprocal_rank_fusion(
    vector_results: &[(String, f32)],
    lexical_results: &[(String, f32)],
    k: f64,
) -> Vec<(String, f64, SourceType)> {
    multi_reciprocal_rank_fusion(&[vector_results, lexical_results], k)
}

pub fn multi_reciprocal_rank_fusion(
    result_lists: &[&[(String, f32)]],
    k: f64,
) -> Vec<(String, f64, SourceType)> {
    let mut scores: HashMap<String, (f64, SourceType)> = HashMap::new();

    for (list_idx, list) in result_lists.iter().enumerate() {
        let is_vector = list_idx % 2 == 0;
        for (rank, (id, _)) in list.iter().enumerate() {
            let rrf = 1.0 / (k + rank as f64 + 1.0);
            scores
                .entry(id.clone())
                .and_modify(|(s, t)| {
                    *s += rrf;
                    if is_vector && *t == SourceType::Keyword {
                        *t = SourceType::Both;
                    } else if !is_vector && *t == SourceType::Vector {
                        *t = SourceType::Both;
                    }
                })
                .or_insert((
                    rrf,
                    if is_vector {
                        SourceType::Vector
                    } else {
                        SourceType::Keyword
                    },
                ));
        }
    }

    let mut fused: Vec<(String, f64, SourceType)> = scores
        .into_iter()
        .map(|(id, (score, src))| (id, score, src))
        .collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuses_multiple_lists() {
        let v = vec![("a".into(), 1.0), ("b".into(), 0.9)];
        let l = vec![("b".into(), 1.0), ("c".into(), 0.8)];
        let fused = multi_reciprocal_rank_fusion(&[&v, &l], 60.0);
        assert!(!fused.is_empty());
        assert_eq!(fused[0].0, "b");
    }
}
