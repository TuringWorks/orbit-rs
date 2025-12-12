// PostgreSQL Full-Text Search Integration
//
// Implements PostgreSQL FTS features:
// - tsvector and tsquery data types
// - to_tsvector(), to_tsquery(), plainto_tsquery() functions
// - @@ match operator
// - GIN index support

use crate::fts::{FtsEngine, SearchResult};
use crate::protocols::postgres_wire::sql::types::SqlValue;
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tantivy::schema::*;
use tokio::sync::RwLock;

/// PostgreSQL FTS integration
pub struct PostgresFts {
    engine: Arc<RwLock<FtsEngine>>,
}

impl PostgresFts {
    pub fn new(engine: Arc<RwLock<FtsEngine>>) -> Self {
        Self { engine }
    }

    /// Convert text to tsvector
    pub async fn to_tsvector(&self, config: &str, text: &str) -> Result<TsVector> {
        // Parse language config (e.g., 'english', 'simple')
        let _language = config;

        // Tokenize and create tsvector
        let tokens = self.tokenize(text);
        Ok(TsVector { tokens })
    }

    /// Convert query string to tsquery
    pub async fn to_tsquery(&self, config: &str, query: &str) -> Result<TsQuery> {
        let _language = config;
        Ok(TsQuery {
            query: query.to_string(),
        })
    }

    /// Plain text to tsquery (no operators)
    pub async fn plainto_tsquery(&self, config: &str, query: &str) -> Result<TsQuery> {
        let _language = config;
        // Convert plain text to query (words joined with &)
        let words: Vec<&str> = query.split_whitespace().collect();
        let query_str = words.join(" & ");
        Ok(TsQuery { query: query_str })
    }

    /// Execute @@ match operator
    pub async fn match_operator(&self, vector: &TsVector, query: &TsQuery) -> Result<bool> {
        // Simple matching: check if any query terms are in vector
        let query_terms: Vec<&str> = query.query.split_whitespace().collect();

        for term in query_terms {
            let clean_term = term.trim_matches(|c| c == '&' || c == '|' || c == '!');
            if vector.tokens.iter().any(|t| t.lexeme == clean_term) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Create GIN index for full-text search
    pub async fn create_gin_index(&self, table_name: &str, column_name: &str) -> Result<()> {
        let index_name = format!("{}_{}_fts", table_name, column_name);

        // Create schema for this index
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("_id", STRING | STORED);

        let text_options = TextOptions::default()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("en_stem")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_stored();

        schema_builder.add_text_field(column_name, text_options);
        let schema = schema_builder.build();

        // Create index in FTS engine
        let engine = self.engine.write().await;
        engine.create_index(&index_name, schema).await?;

        Ok(())
    }

    /// Search using GIN index
    pub async fn search_gin_index(
        &self,
        table_name: &str,
        column_name: &str,
        query: &TsQuery,
    ) -> Result<Vec<SearchResult>> {
        let index_name = format!("{}_{}_fts", table_name, column_name);

        let engine = self.engine.read().await;

        // Get index handle to create query parser
        let indexes = engine.list_indexes().await;
        if !indexes.contains(&index_name) {
            return Err(anyhow!("GIN index not found: {}", index_name));
        }

        // Parse query and search
        // Note: This is simplified - in production we'd use the query_parser module
        let tantivy_query = self.parse_tsquery_to_tantivy(&query.query)?;

        engine.search(&index_name, tantivy_query, 100).await
    }

    /// Tokenize text (simplified)
    fn tokenize(&self, text: &str) -> Vec<TsToken> {
        text.split_whitespace()
            .enumerate()
            .map(|(pos, word)| TsToken {
                lexeme: word.to_lowercase(),
                positions: vec![pos as u32],
            })
            .collect()
    }

    /// Parse tsquery to Tantivy query (placeholder)
    fn parse_tsquery_to_tantivy(&self, _query: &str) -> Result<Box<dyn tantivy::query::Query>> {
        // This would use the query_parser module in production
        Err(anyhow!("Not implemented - use query_parser module"))
    }
}

/// PostgreSQL tsvector type
#[derive(Debug, Clone)]
pub struct TsVector {
    pub tokens: Vec<TsToken>,
}

impl TsVector {
    pub fn from_string(s: &str) -> Self {
        // Parse tsvector string format: 'word1':1 'word2':2,3
        let tokens = s
            .split_whitespace()
            .filter_map(|part| {
                let parts: Vec<&str> = part.split(':').collect();
                if parts.len() == 2 {
                    let lexeme = parts[0].trim_matches('\'').to_string();
                    let positions: Vec<u32> =
                        parts[1].split(',').filter_map(|p| p.parse().ok()).collect();
                    Some(TsToken { lexeme, positions })
                } else {
                    None
                }
            })
            .collect();

        TsVector { tokens }
    }

    pub fn to_string(&self) -> String {
        self.tokens
            .iter()
            .map(|t| {
                format!(
                    "'{}':{}",
                    t.lexeme,
                    t.positions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Token in tsvector
#[derive(Debug, Clone)]
pub struct TsToken {
    pub lexeme: String,
    pub positions: Vec<u32>,
}

/// PostgreSQL tsquery type
#[derive(Debug, Clone)]
pub struct TsQuery {
    pub query: String,
}

impl TsQuery {
    pub fn from_string(s: &str) -> Self {
        TsQuery {
            query: s.to_string(),
        }
    }

    pub fn to_string(&self) -> String {
        self.query.clone()
    }
}

/// Convert SqlValue to TsVector
pub fn sqlvalue_to_tsvector(value: &SqlValue) -> Result<TsVector> {
    match value {
        SqlValue::Text(s) => Ok(TsVector::from_string(s)),
        _ => Err(anyhow!("Cannot convert {:?} to tsvector", value)),
    }
}

/// Convert SqlValue to TsQuery
pub fn sqlvalue_to_tsquery(value: &SqlValue) -> Result<TsQuery> {
    match value {
        SqlValue::Text(s) => Ok(TsQuery::from_string(s)),
        _ => Err(anyhow!("Cannot convert {:?} to tsquery", value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsvector_parsing() {
        let vec = TsVector::from_string("'cat':1 'dog':2,3");
        assert_eq!(vec.tokens.len(), 2);
        assert_eq!(vec.tokens[0].lexeme, "cat");
        assert_eq!(vec.tokens[0].positions, vec![1]);
        assert_eq!(vec.tokens[1].lexeme, "dog");
        assert_eq!(vec.tokens[1].positions, vec![2, 3]);
    }

    #[test]
    fn test_tsvector_to_string() {
        let vec = TsVector {
            tokens: vec![
                TsToken {
                    lexeme: "hello".to_string(),
                    positions: vec![1],
                },
                TsToken {
                    lexeme: "world".to_string(),
                    positions: vec![2, 3],
                },
            ],
        };
        let s = vec.to_string();
        assert!(s.contains("'hello':1"));
        assert!(s.contains("'world':2,3"));
    }

    #[test]
    fn test_tsquery() {
        let query = TsQuery::from_string("cat & dog");
        assert_eq!(query.query, "cat & dog");
    }
}
