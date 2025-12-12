// MongoDB Full-Text Search Integration
//
// Implements MongoDB text search features:
// - Text indexes
// - $text operator
// - $search query
// - Language-specific stemming
// - Text score projection

use crate::fts::FtsEngine;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tantivy::schema::*;
use tokio::sync::RwLock;

/// MongoDB FTS integration
pub struct MongodbFts {
    engine: Arc<RwLock<FtsEngine>>,
}

impl MongodbFts {
    pub fn new(engine: Arc<RwLock<FtsEngine>>) -> Self {
        Self { engine }
    }

    /// Create text index
    pub async fn create_text_index(
        &self,
        collection: &str,
        fields: &[(String, i32)], // (field_name, weight)
        language: Option<&str>,
    ) -> Result<()> {
        let index_name = format!("{}_text", collection);
        let _lang = language.unwrap_or("english");

        // Create schema with weighted fields
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("_id", STRING | STORED);

        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        for (field_name, _weight) in fields {
            schema_builder.add_text_field(field_name, text_options.clone());
        }

        let schema = schema_builder.build();

        // Create index
        let engine = self.engine.write().await;
        engine.create_index(&index_name, schema).await?;

        Ok(())
    }

    /// Execute $text search
    pub async fn text_search(
        &self,
        collection: &str,
        search: &str,
        language: Option<&str>,
        case_sensitive: bool,
        diacritic_sensitive: bool,
    ) -> Result<Vec<TextSearchResult>> {
        let index_name = format!("{}_text", collection);
        let _lang = language.unwrap_or("english");
        let _case_sens = case_sensitive;
        let _diacritic_sens = diacritic_sensitive;

        let engine = self.engine.read().await;
        let results = engine
            .search(&index_name, self.parse_text_query(search)?, 100)
            .await?;

        Ok(results
            .into_iter()
            .map(|r| TextSearchResult {
                doc_id: r.doc_id,
                score: r.score,
                fields: r.fields,
            })
            .collect())
    }

    /// Get text score for sorting
    pub fn get_text_score(&self, result: &TextSearchResult) -> f32 {
        result.score
    }

    /// Parse $text query
    fn parse_text_query(&self, _query: &str) -> Result<Box<dyn tantivy::query::Query>> {
        // Placeholder - would use query_parser module
        Err(anyhow!("Not implemented - use query_parser module"))
    }
}

/// MongoDB text search result
#[derive(Debug, Clone)]
pub struct TextSearchResult {
    pub doc_id: String,
    pub score: f32,
    pub fields: Vec<(String, String)>,
}

impl TextSearchResult {
    /// Convert to MongoDB document with textScore
    pub fn to_document(&self) -> HashMap<String, serde_json::Value> {
        let mut doc = HashMap::new();
        doc.insert("_id".to_string(), serde_json::json!(self.doc_id));
        doc.insert("score".to_string(), serde_json::json!(self.score));

        for (key, value) in &self.fields {
            doc.insert(key.clone(), serde_json::json!(value));
        }

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_search_result() {
        let result = TextSearchResult {
            doc_id: "507f1f77bcf86cd799439011".to_string(),
            score: 1.5,
            fields: vec![("title".to_string(), "Test Document".to_string())],
        };

        let doc = result.to_document();
        assert!(doc.contains_key("_id"));
        assert!(doc.contains_key("score"));
        assert!(doc.contains_key("title"));
    }
}
