// Index Manager
//
// Manages FTS index lifecycle, schema, and metadata.

use super::FtsConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use tantivy::schema::*;

/// Index metadata
#[derive(Debug, Clone)]
pub struct IndexMetadata {
    pub name: String,
    pub schema: Schema,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub num_docs: u64,
}

/// Index manager
pub struct IndexManager {
    config: FtsConfig,
    metadata: HashMap<String, IndexMetadata>,
}

impl IndexManager {
    pub fn new(config: FtsConfig) -> Self {
        Self {
            config,
            metadata: HashMap::new(),
        }
    }

    /// Register a new index
    pub fn register_index(&mut self, name: String, schema: Schema) {
        let metadata = IndexMetadata {
            name: name.clone(),
            schema,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            num_docs: 0,
        };
        self.metadata.insert(name, metadata);
    }

    /// Unregister an index
    pub fn unregister_index(&mut self, name: &str) {
        self.metadata.remove(name);
    }

    /// Get index metadata
    pub fn get_metadata(&self, name: &str) -> Option<&IndexMetadata> {
        self.metadata.get(name)
    }

    /// Update index statistics
    pub fn update_stats(&mut self, name: &str, num_docs: u64) {
        if let Some(metadata) = self.metadata.get_mut(name) {
            metadata.num_docs = num_docs;
            metadata.updated_at = chrono::Utc::now();
        }
    }

    /// List all indexes
    pub fn list_indexes(&self) -> Vec<String> {
        self.metadata.keys().cloned().collect()
    }

    /// Get index path
    pub fn get_index_path(&self, name: &str) -> PathBuf {
        self.config.index_dir.join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_manager() {
        let config = FtsConfig::default();
        let mut manager = IndexManager::new(config);

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("title", TEXT);
        let schema = schema_builder.build();

        manager.register_index("test".to_string(), schema);
        assert!(manager.get_metadata("test").is_some());

        manager.unregister_index("test");
        assert!(manager.get_metadata("test").is_none());
    }
}
