// MySQL Full-Text Search Integration
//
// Implements MySQL FULLTEXT features:
// - FULLTEXT indexes
// - MATCH() AGAINST() function
// - Natural language mode
// - Boolean mode
// - Query expansion mode

use crate::fts::FtsEngine;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::*;
use tantivy::Term;
use tokio::sync::RwLock;

/// MySQL FTS integration
pub struct MysqlFts {
    /// Optional Tantivy FTS engine (may not be available in all configurations)
    engine: Option<Arc<RwLock<FtsEngine>>>,
    /// In-memory text storage for simple FTS (always available)
    text_store: Arc<RwLock<HashMap<String, TextIndex>>>,
}

/// Simple in-memory text index for MySQL FULLTEXT
struct TextIndex {
    /// Documents: doc_id -> field values
    documents: HashMap<String, HashMap<String, String>>,
    /// Inverted index: term -> [(doc_id, term_frequency)]
    inverted_index: HashMap<String, Vec<(String, f32)>>,
    /// Columns in this index
    columns: Vec<String>,
}

impl TextIndex {
    fn new(columns: Vec<String>) -> Self {
        Self {
            documents: HashMap::new(),
            inverted_index: HashMap::new(),
            columns,
        }
    }

    /// Tokenize text into terms
    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 1)
            .map(|s| s.to_string())
            .collect()
    }

    /// Add document to index
    fn add_document(&mut self, doc_id: &str, fields: HashMap<String, String>) {
        // Index all text columns
        for col in &self.columns {
            if let Some(value) = fields.get(col) {
                let terms = Self::tokenize(value);
                let term_count = terms.len() as f32;

                for term in terms {
                    let tf = 1.0 / term_count.max(1.0); // Normalized TF
                    self.inverted_index
                        .entry(term)
                        .or_insert_with(Vec::new)
                        .push((doc_id.to_string(), tf));
                }
            }
        }

        self.documents.insert(doc_id.to_string(), fields);
    }

    /// Search with natural language mode (simple TF-IDF)
    fn search_natural(&self, query: &str, limit: usize) -> Vec<MatchResult> {
        let query_terms = Self::tokenize(query);
        if query_terms.is_empty() {
            return vec![];
        }

        let num_docs = self.documents.len() as f32;
        let mut doc_scores: HashMap<String, f32> = HashMap::new();

        for term in &query_terms {
            if let Some(postings) = self.inverted_index.get(term) {
                // IDF = log(N / df)
                let idf = (num_docs / postings.len() as f32).ln().max(0.0) + 1.0;

                for (doc_id, tf) in postings {
                    let score = tf * idf;
                    *doc_scores.entry(doc_id.clone()).or_insert(0.0) += score;
                }
            }
        }

        // Sort by score
        let mut results: Vec<(String, f32)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results
            .into_iter()
            .take(limit)
            .filter_map(|(doc_id, score)| {
                self.documents.get(&doc_id).map(|fields| MatchResult {
                    doc_id,
                    relevance: score,
                    fields: fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                })
            })
            .collect()
    }

    /// Search with boolean mode (+must -exclude optional)
    fn search_boolean(&self, query: &str, limit: usize) -> Vec<MatchResult> {
        let mut must_terms = Vec::new();
        let mut must_not_terms = Vec::new();
        let mut should_terms = Vec::new();

        for term in query.split_whitespace() {
            let term = term.to_lowercase();
            if term.starts_with('+') {
                must_terms.push(term[1..].to_string());
            } else if term.starts_with('-') {
                must_not_terms.push(term[1..].to_string());
            } else if term.starts_with('"') && term.ends_with('"') {
                // Phrase query - treat as must for now
                must_terms.push(term.trim_matches('"').to_string());
            } else {
                should_terms.push(term);
            }
        }

        let mut doc_scores: HashMap<String, f32> = HashMap::new();
        let num_docs = self.documents.len() as f32;

        // Must terms - all must match
        for term in &must_terms {
            if let Some(postings) = self.inverted_index.get(term) {
                let idf = (num_docs / postings.len() as f32).ln().max(0.0) + 1.0;
                for (doc_id, tf) in postings {
                    *doc_scores.entry(doc_id.clone()).or_insert(0.0) += tf * idf * 2.0;
                    // Boost must terms
                }
            }
        }

        // Should terms - optional boost
        for term in &should_terms {
            if let Some(postings) = self.inverted_index.get(term) {
                let idf = (num_docs / postings.len() as f32).ln().max(0.0) + 1.0;
                for (doc_id, tf) in postings {
                    *doc_scores.entry(doc_id.clone()).or_insert(0.0) += tf * idf;
                }
            }
        }

        // Must not - exclude these documents
        let excluded: std::collections::HashSet<String> = must_not_terms
            .iter()
            .flat_map(|term| {
                self.inverted_index
                    .get(term)
                    .map(|postings| {
                        postings
                            .iter()
                            .map(|(id, _)| id.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect();

        // Filter out excluded documents
        doc_scores.retain(|id, _| !excluded.contains(id));

        // If must terms specified, filter to only docs that have all must terms
        if !must_terms.is_empty() {
            let must_docs: Vec<std::collections::HashSet<String>> = must_terms
                .iter()
                .map(|term| {
                    self.inverted_index
                        .get(term)
                        .map(|postings| postings.iter().map(|(id, _)| id.clone()).collect())
                        .unwrap_or_default()
                })
                .collect();

            if let Some(first) = must_docs.first() {
                let intersection: std::collections::HashSet<String> =
                    must_docs.iter().skip(1).fold(first.clone(), |acc, set| {
                        acc.intersection(set).cloned().collect()
                    });
                doc_scores.retain(|id, _| intersection.contains(id));
            }
        }

        // Sort by score
        let mut results: Vec<(String, f32)> = doc_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results
            .into_iter()
            .take(limit)
            .filter_map(|(doc_id, score)| {
                self.documents.get(&doc_id).map(|fields| MatchResult {
                    doc_id,
                    relevance: score,
                    fields: fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                })
            })
            .collect()
    }
}

impl MysqlFts {
    pub fn new(engine: Arc<RwLock<FtsEngine>>) -> Self {
        Self {
            engine: Some(engine),
            text_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new instance without FTS engine (uses in-memory index only)
    pub fn new_simple() -> Self {
        Self {
            engine: None,
            text_store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create FULLTEXT index
    pub async fn create_fulltext_index(&self, table_name: &str, columns: &[String]) -> Result<()> {
        let index_name = format!("{}_{}_fulltext", table_name, columns.join("_"));

        // Create in-memory text index
        let mut store = self.text_store.write().await;
        store.insert(index_name.clone(), TextIndex::new(columns.to_vec()));

        // Also try to create Tantivy index for better performance (if available)
        if let Some(engine) = &self.engine {
            let mut schema_builder = Schema::builder();
            schema_builder.add_text_field("_id", STRING | STORED);

            let text_options = TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("default")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored();

            for column in columns {
                schema_builder.add_text_field(column, text_options.clone());
            }

            let schema = schema_builder.build();

            // Create Tantivy index (best effort)
            if let Ok(engine_guard) = engine.try_write() {
                let _ = engine_guard.create_index(&index_name, schema).await;
            }
        }

        Ok(())
    }

    /// Add document to FULLTEXT index
    pub async fn add_document(
        &self,
        table_name: &str,
        columns: &[String],
        doc_id: &str,
        fields: HashMap<String, String>,
    ) -> Result<()> {
        let index_name = format!("{}_{}_fulltext", table_name, columns.join("_"));

        let mut store = self.text_store.write().await;
        if let Some(index) = store.get_mut(&index_name) {
            index.add_document(doc_id, fields);
        }

        Ok(())
    }

    /// MATCH() AGAINST() in natural language mode
    pub async fn match_against_natural(
        &self,
        table_name: &str,
        columns: &[String],
        query: &str,
    ) -> Result<Vec<MatchResult>> {
        let index_name = format!("{}_{}_fulltext", table_name, columns.join("_"));

        // Use in-memory index
        let store = self.text_store.read().await;
        if let Some(index) = store.get(&index_name) {
            return Ok(index.search_natural(query, 100));
        }

        Ok(vec![])
    }

    /// MATCH() AGAINST() in boolean mode
    pub async fn match_against_boolean(
        &self,
        table_name: &str,
        columns: &[String],
        query: &str,
    ) -> Result<Vec<MatchResult>> {
        let index_name = format!("{}_{}_fulltext", table_name, columns.join("_"));

        // Use in-memory index
        let store = self.text_store.read().await;
        if let Some(index) = store.get(&index_name) {
            return Ok(index.search_boolean(query, 100));
        }

        Ok(vec![])
    }

    /// MATCH() AGAINST() with query expansion
    pub async fn match_against_expansion(
        &self,
        table_name: &str,
        columns: &[String],
        query: &str,
    ) -> Result<Vec<MatchResult>> {
        // Query expansion: first search, then use top results to expand query
        // Query expansion: first search, then expand with related terms
        let initial_results = self
            .match_against_natural(table_name, columns, query)
            .await?;

        if initial_results.is_empty() {
            return Ok(initial_results);
        }

        // Extract terms from top 3 results for query expansion
        let mut expanded_terms: Vec<String> =
            query.split_whitespace().map(|s| s.to_lowercase()).collect();

        for result in initial_results.iter().take(3) {
            for (_, value) in &result.fields {
                for word in value.split_whitespace() {
                    let word = word.to_lowercase();
                    if word.len() > 2 && !expanded_terms.contains(&word) {
                        expanded_terms.push(word);
                    }
                }
            }
        }

        // Limit expanded terms
        expanded_terms.truncate(20);

        // Search with expanded query
        let expanded_query = expanded_terms.join(" ");
        self.match_against_natural(table_name, columns, &expanded_query)
            .await
    }

    /// Parse natural language query to Tantivy query (for Tantivy backend)
    #[allow(dead_code)]
    fn parse_natural_query(&self, query: &str, schema: &Schema) -> Result<Box<dyn Query>> {
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        for term in query.split_whitespace() {
            let term = term.to_lowercase();
            // Add term query for each text field
            for (field, entry) in schema.fields() {
                if entry.field_type().is_indexed() {
                    let term_obj = Term::from_field_text(field, &term);
                    clauses.push((
                        Occur::Should,
                        Box::new(TermQuery::new(term_obj, IndexRecordOption::Basic)),
                    ));
                }
            }
        }

        Ok(Box::new(BooleanQuery::from(clauses)))
    }

    /// Parse boolean mode query to Tantivy query (for Tantivy backend)
    #[allow(dead_code)]
    fn parse_boolean_query(&self, query: &str, schema: &Schema) -> Result<Box<dyn Query>> {
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        for term in query.split_whitespace() {
            let (occur, word) = if term.starts_with('+') {
                (Occur::Must, &term[1..])
            } else if term.starts_with('-') {
                (Occur::MustNot, &term[1..])
            } else {
                (Occur::Should, term)
            };

            let word = word.to_lowercase();

            // Add term query for each text field
            for (field, entry) in schema.fields() {
                if entry.field_type().is_indexed() {
                    let term_obj = Term::from_field_text(field, &word);
                    clauses.push((
                        occur,
                        Box::new(TermQuery::new(term_obj, IndexRecordOption::Basic)),
                    ));
                }
            }
        }

        Ok(Box::new(BooleanQuery::from(clauses)))
    }
}

/// MySQL MATCH() result
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub doc_id: String,
    pub relevance: f32,
    pub fields: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_result() {
        let result = MatchResult {
            doc_id: "1".to_string(),
            relevance: 0.95,
            fields: vec![("title".to_string(), "Test".to_string())],
        };
        assert_eq!(result.relevance, 0.95);
    }
}
