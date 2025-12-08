// Tantivy-based FTS Engine
//
// Core full-text search engine using Tantivy for indexing and searching.

use super::{FtsConfig, IndexStats, SearchResult};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy};
use tokio::sync::RwLock;

/// Main FTS engine
pub struct FtsEngine {
    config: FtsConfig,
    indexes: Arc<RwLock<HashMap<String, IndexHandle>>>,
}

/// Handle to a Tantivy index
struct IndexHandle {
    index: Index,
    reader: IndexReader,
    writer: Arc<RwLock<IndexWriter>>,
    schema: Schema,
}

impl FtsEngine {
    /// Create a new FTS engine
    pub fn new(config: FtsConfig) -> Result<Self> {
        // Create index directory if it doesn't exist
        std::fs::create_dir_all(&config.index_dir)
            .context("Failed to create FTS index directory")?;

        Ok(Self {
            config,
            indexes: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create a new index
    pub async fn create_index(
        &self,
        name: &str,
        schema: Schema,
    ) -> Result<()> {
        let index_path = self.config.index_dir.join(name);
        std::fs::create_dir_all(&index_path)?;

        // Create Tantivy index
        let index = Index::create_in_dir(&index_path, schema.clone())?;

        // Create index writer
        let writer = index.writer(self.config.max_memory)?;
        writer.set_num_threads(self.config.num_threads)?;

        // Create index reader
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let handle = IndexHandle {
            index,
            reader,
            writer: Arc::new(RwLock::new(writer)),
            schema,
        };

        self.indexes.write().await.insert(name.to_string(), handle);

        Ok(())
    }

    /// Drop an index
    pub async fn drop_index(&self, name: &str) -> Result<()> {
        self.indexes.write().await.remove(name);

        let index_path = self.config.index_dir.join(name);
        if index_path.exists() {
            std::fs::remove_dir_all(&index_path)?;
        }

        Ok(())
    }

    /// Add a document to an index
    pub async fn add_document(
        &self,
        index_name: &str,
        doc: tantivy::Document,
    ) -> Result<()> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .context("Index not found")?;

        let mut writer = handle.writer.write().await;
        writer.add_document(doc)?;

        Ok(())
    }

    /// Commit pending changes
    pub async fn commit(&self, index_name: &str) -> Result<()> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .context("Index not found")?;

        let mut writer = handle.writer.write().await;
        writer.commit()?;

        Ok(())
    }

    /// Search an index
    pub async fn search(
        &self,
        index_name: &str,
        query: Box<dyn tantivy::query::Query>,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .context("Index not found")?;

        let searcher = handle.reader.searcher();
        let top_docs = searcher.search(
            &query,
            &tantivy::collector::TopDocs::with_limit(limit),
        )?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;
            
            // Extract fields from document
            let mut fields = Vec::new();
            for (field, field_values) in doc.get_all_sorted() {
                if let Some(field_entry) = handle.schema.get_field_entry(field) {
                    let field_name = field_entry.name().to_string();
                    for value in field_values {
                        if let Some(text) = value.as_str() {
                            fields.push((field_name.clone(), text.to_string()));
                        }
                    }
                }
            }

            // Generate doc ID from first field
            let doc_id = fields
                .first()
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| format!("{:?}", doc_address));

            results.push(SearchResult {
                doc_id,
                score,
                fields,
                highlights: Vec::new(), // TODO: Implement highlighting
            });
        }

        Ok(results)
    }

    /// Get index statistics
    pub async fn get_stats(&self, index_name: &str) -> Result<IndexStats> {
        let indexes = self.indexes.read().await;
        let handle = indexes
            .get(index_name)
            .context("Index not found")?;

        let searcher = handle.reader.searcher();
        let num_docs = searcher.num_docs();

        // Calculate index size
        let index_path = self.config.index_dir.join(index_name);
        let size_bytes = calculate_dir_size(&index_path)?;

        Ok(IndexStats {
            num_docs,
            size_bytes,
            num_fields: handle.schema.fields().count(),
            last_updated: chrono::Utc::now(),
        })
    }

    /// List all indexes
    pub async fn list_indexes(&self) -> Vec<String> {
        self.indexes.read().await.keys().cloned().collect()
    }
}

/// Calculate directory size recursively
fn calculate_dir_size(path: &PathBuf) -> Result<u64> {
    let mut size = 0u64;
    
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                size += calculate_dir_size(&entry.path())?;
            } else {
                size += metadata.len();
            }
        }
    }
    
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_index() {
        let config = FtsConfig {
            index_dir: PathBuf::from("/tmp/fts_test"),
            ..Default::default()
        };

        let engine = FtsEngine::new(config).unwrap();

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("body", TEXT);
        let schema = schema_builder.build();

        engine.create_index("test_index", schema).await.unwrap();

        let indexes = engine.list_indexes().await;
        assert!(indexes.contains(&"test_index".to_string()));

        // Cleanup
        engine.drop_index("test_index").await.unwrap();
    }
}
