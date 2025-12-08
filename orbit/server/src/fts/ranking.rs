// Ranking Engine
//
// Provides BM25 ranking and relevance scoring for search results.

use super::SearchResult;
use anyhow::Result;

/// Ranking engine for search results
pub struct RankingEngine {
    /// BM25 k1 parameter (term frequency saturation)
    k1: f32,
    /// BM25 b parameter (length normalization)
    b: f32,
}

impl Default for RankingEngine {
    fn default() -> Self {
        Self {
            k1: 1.2,  // Standard BM25 k1
            b: 0.75,  // Standard BM25 b
        }
    }
}

impl RankingEngine {
    pub fn new(k1: f32, b: f32) -> Self {
        Self { k1, b }
    }

    /// Re-rank search results (Tantivy already uses BM25)
    pub fn rank(&self, mut results: Vec<SearchResult>) -> Vec<SearchResult> {
        // Tantivy already provides BM25 scores, so we just sort
        results.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        results
    }

    /// Normalize scores to [0, 1] range
    pub fn normalize_scores(&self, results: &mut [SearchResult]) {
        if results.is_empty() {
            return;
        }

        let max_score = results
            .iter()
            .map(|r| r.score)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(1.0);

        if max_score > 0.0 {
            for result in results.iter_mut() {
                result.score /= max_score;
            }
        }
    }

    /// Apply custom scoring function
    pub fn apply_custom_scoring<F>(&self, results: &mut [SearchResult], f: F)
    where
        F: Fn(&SearchResult) -> f32,
    {
        for result in results.iter_mut() {
            result.score = f(result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranking() {
        let engine = RankingEngine::default();
        
        let results = vec![
            SearchResult {
                doc_id: "1".to_string(),
                score: 0.5,
                fields: vec![],
                highlights: vec![],
            },
            SearchResult {
                doc_id: "2".to_string(),
                score: 0.9,
                fields: vec![],
                highlights: vec![],
            },
            SearchResult {
                doc_id: "3".to_string(),
                score: 0.3,
                fields: vec![],
                highlights: vec![],
            },
        ];

        let ranked = engine.rank(results);
        assert_eq!(ranked[0].doc_id, "2");
        assert_eq!(ranked[1].doc_id, "1");
        assert_eq!(ranked[2].doc_id, "3");
    }

    #[test]
    fn test_normalize_scores() {
        let engine = RankingEngine::default();
        
        let mut results = vec![
            SearchResult {
                doc_id: "1".to_string(),
                score: 5.0,
                fields: vec![],
                highlights: vec![],
            },
            SearchResult {
                doc_id: "2".to_string(),
                score: 10.0,
                fields: vec![],
                highlights: vec![],
            },
        ];

        engine.normalize_scores(&mut results);
        assert_eq!(results[0].score, 0.5);
        assert_eq!(results[1].score, 1.0);
    }
}
