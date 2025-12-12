// Redis Full-Text Search Integration
//
// Implements RedisSearch module features:
// - FT.CREATE - Create search index
// - FT.SEARCH - Search index
// - FT.AGGREGATE - Aggregate search results
// - FT.INFO - Index information
// - FT.DROPINDEX - Drop index

use crate::fts::FtsEngine;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tantivy::schema::*;
use tokio::sync::RwLock;

/// Redis FTS integration (RedisSearch compatibility)
pub struct RedisFts {
    engine: Arc<RwLock<FtsEngine>>,
}

impl RedisFts {
    pub fn new(engine: Arc<RwLock<FtsEngine>>) -> Self {
        Self { engine }
    }

    /// FT.CREATE - Create a search index
    pub async fn ft_create(
        &self,
        index_name: &str,
        schema_fields: &[(String, RedisFieldType)],
    ) -> Result<String> {
        // Create Tantivy schema from Redis schema
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("_id", STRING | STORED);

        for (field_name, field_type) in schema_fields {
            match field_type {
                RedisFieldType::Text => {
                    let text_options = TextOptions::default()
                        .set_indexing_options(
                            TextFieldIndexing::default()
                                .set_tokenizer("default")
                                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                        )
                        .set_stored();
                    schema_builder.add_text_field(field_name, text_options);
                }
                RedisFieldType::Tag => {
                    schema_builder.add_text_field(field_name, STRING | STORED);
                }
                RedisFieldType::Numeric => {
                    schema_builder.add_i64_field(field_name, INDEXED | STORED);
                }
                RedisFieldType::Geo => {
                    // TODO: Implement geo field support
                    schema_builder.add_text_field(field_name, STRING | STORED);
                }
                RedisFieldType::Vector => {
                    // TODO: Implement vector field support
                    schema_builder.add_bytes_field(field_name, STORED);
                }
            }
        }

        let schema = schema_builder.build();

        // Create index
        let engine = self.engine.write().await;
        engine.create_index(index_name, schema).await?;

        Ok("OK".to_string())
    }

    /// FT.SEARCH - Search the index
    pub async fn ft_search(
        &self,
        index_name: &str,
        query: &str,
        options: &SearchOptions,
    ) -> Result<Vec<RedisSearchResult>> {
        let engine = self.engine.read().await;

        let limit = options.limit.unwrap_or(10);
        let results = engine
            .search(index_name, self.parse_redis_query(query)?, limit)
            .await?;

        Ok(results
            .into_iter()
            .skip(options.offset.unwrap_or(0))
            .map(|r| RedisSearchResult {
                key: r.doc_id,
                score: r.score,
                fields: r.fields.into_iter().collect(),
            })
            .collect())
    }

    /// FT.AGGREGATE - Aggregate search results
    pub async fn ft_aggregate(
        &self,
        index_name: &str,
        query: &str,
        _pipeline: &[AggregateOp],
    ) -> Result<Vec<HashMap<String, String>>> {
        // Simplified aggregation - just search for now
        let results = self
            .ft_search(index_name, query, &SearchOptions::default())
            .await?;

        Ok(results.into_iter().map(|r| r.fields).collect())
    }

    /// FT.INFO - Get index information
    pub async fn ft_info(&self, index_name: &str) -> Result<IndexInfo> {
        let engine = self.engine.read().await;
        let stats = engine.get_stats(index_name).await?;

        Ok(IndexInfo {
            index_name: index_name.to_string(),
            num_docs: stats.num_docs,
            num_terms: 0, // TODO: Calculate from index
            num_records: stats.num_docs,
            inverted_sz_mb: (stats.size_bytes as f64) / (1024.0 * 1024.0),
            total_inverted_index_blocks: 0,
            offset_vectors_sz_mb: 0.0,
            doc_table_size_mb: 0.0,
            sortable_values_size_mb: 0.0,
            key_table_size_mb: 0.0,
        })
    }

    /// FT.DROPINDEX - Drop an index
    pub async fn ft_dropindex(&self, index_name: &str) -> Result<String> {
        let engine = self.engine.write().await;
        engine.drop_index(index_name).await?;
        Ok("OK".to_string())
    }

    /// Parse Redis query syntax
    fn parse_redis_query(&self, _query: &str) -> Result<Box<dyn tantivy::query::Query>> {
        // Placeholder - would use query_parser module
        Err(anyhow!("Not implemented - use query_parser module"))
    }
}

/// Redis field types
#[derive(Debug, Clone)]
pub enum RedisFieldType {
    Text,
    Tag,
    Numeric,
    Geo,
    Vector,
}

/// Search options
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort_by: Option<String>,
    pub return_fields: Option<Vec<String>>,
}

/// Aggregate operations
#[derive(Debug, Clone)]
pub enum AggregateOp {
    GroupBy { fields: Vec<String> },
    Reduce { function: String, args: Vec<String> },
    SortBy { field: String, order: SortOrder },
    Limit { offset: usize, num: usize },
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Asc,
    Desc,
}

/// Redis search result
#[derive(Debug, Clone)]
pub struct RedisSearchResult {
    pub key: String,
    pub score: f32,
    pub fields: HashMap<String, String>,
}

/// Index information
#[derive(Debug, Clone)]
pub struct IndexInfo {
    pub index_name: String,
    pub num_docs: u64,
    pub num_terms: u64,
    pub num_records: u64,
    pub inverted_sz_mb: f64,
    pub total_inverted_index_blocks: u64,
    pub offset_vectors_sz_mb: f64,
    pub doc_table_size_mb: f64,
    pub sortable_values_size_mb: f64,
    pub key_table_size_mb: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_search_result() {
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), "Test".to_string());

        let result = RedisSearchResult {
            key: "doc:1".to_string(),
            score: 1.0,
            fields,
        };

        assert_eq!(result.key, "doc:1");
        assert_eq!(result.score, 1.0);
        assert!(result.fields.contains_key("title"));
    }

    #[test]
    fn test_search_options() {
        let opts = SearchOptions {
            limit: Some(10),
            offset: Some(0),
            sort_by: None,
            return_fields: None,
        };

        assert_eq!(opts.limit, Some(10));
        assert_eq!(opts.offset, Some(0));
    }
}
