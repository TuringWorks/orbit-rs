// Batch insert optimization for MySQL protocol
//
// Provides optimized batch insert operations to improve throughput
// for bulk data loading scenarios.

use bytes::Bytes;
use std::collections::HashMap;

/// Batch insert configuration
#[derive(Debug, Clone)]
pub struct BatchInsertConfig {
    /// Maximum batch size (number of rows)
    pub max_batch_size: usize,
    /// Maximum batch bytes
    pub max_batch_bytes: usize,
    /// Flush interval in milliseconds
    pub flush_interval_ms: u64,
}

impl Default for BatchInsertConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1000,
            max_batch_bytes: 16 * 1024 * 1024, // 16MB
            flush_interval_ms: 100,
        }
    }
}

/// Batch insert optimizer
pub struct BatchInsertOptimizer {
    config: BatchInsertConfig,
    pending_batches: HashMap<String, Vec<String>>,
}

impl BatchInsertOptimizer {
    /// Create a new batch insert optimizer
    pub fn new(config: BatchInsertConfig) -> Self {
        Self {
            config,
            pending_batches: HashMap::new(),
        }
    }

    /// Check if query is an INSERT statement
    pub fn is_insert_query(query: &str) -> bool {
        query.trim_start().to_uppercase().starts_with("INSERT")
    }

    /// Extract table name from INSERT query
    pub fn extract_table_name(query: &str) -> Option<String> {
        let upper = query.to_uppercase();
        if let Some(into_pos) = upper.find("INTO") {
            let after_into = &query[into_pos + 4..].trim_start();
            if let Some(space_pos) = after_into.find(|c: char| c.is_whitespace() || c == '(') {
                return Some(after_into[..space_pos].trim().to_string());
            }
        }
        None
    }

    /// Add query to batch
    pub fn add_to_batch(&mut self, query: String) -> Option<Vec<String>> {
        if let Some(table_name) = Self::extract_table_name(&query) {
            let batch = self.pending_batches.entry(table_name.clone()).or_default();
            batch.push(query);

            // Check if we should flush
            if batch.len() >= self.config.max_batch_size {
                return Some(self.flush_batch(&table_name));
            }
        }
        None
    }

    /// Flush batch for a specific table
    pub fn flush_batch(&mut self, table_name: &str) -> Vec<String> {
        self.pending_batches.remove(table_name).unwrap_or_default()
    }

    /// Flush all pending batches
    pub fn flush_all(&mut self) -> Vec<Vec<String>> {
        let mut all_batches = Vec::new();
        for (_, batch) in self.pending_batches.drain() {
            if !batch.is_empty() {
                all_batches.push(batch);
            }
        }
        all_batches
    }

    /// Combine multiple INSERT statements into a single multi-value INSERT
    pub fn combine_inserts(inserts: &[String]) -> Option<String> {
        if inserts.is_empty() {
            return None;
        }

        // Parse first insert to get table and columns
        let first = &inserts[0];
        let upper = first.to_uppercase();
        
        if let Some(values_pos) = upper.find("VALUES") {
            let prefix = &first[..values_pos + 6].trim();
            
            // Extract all value clauses
            let mut all_values = Vec::new();
            for insert in inserts {
                if let Some(val_pos) = insert.to_uppercase().find("VALUES") {
                    let values_part = insert[val_pos + 6..].trim();
                    all_values.push(values_part.to_string());
                }
            }

            if !all_values.is_empty() {
                return Some(format!("{} {}", prefix, all_values.join(", ")));
            }
        }

        None
    }

    /// Get statistics
    pub fn stats(&self) -> BatchStats {
        BatchStats {
            pending_batches: self.pending_batches.len(),
            total_pending_rows: self.pending_batches.values().map(|b| b.len()).sum(),
        }
    }
}

/// Batch statistics
#[derive(Debug, Clone)]
pub struct BatchStats {
    pub pending_batches: usize,
    pub total_pending_rows: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_insert_query() {
        assert!(BatchInsertOptimizer::is_insert_query("INSERT INTO users VALUES (1, 'test')"));
        assert!(BatchInsertOptimizer::is_insert_query("  insert into users values (1, 'test')"));
        assert!(!BatchInsertOptimizer::is_insert_query("SELECT * FROM users"));
    }

    #[test]
    fn test_extract_table_name() {
        let table = BatchInsertOptimizer::extract_table_name("INSERT INTO users (id, name) VALUES (1, 'test')");
        assert_eq!(table, Some("users".to_string()));

        let table = BatchInsertOptimizer::extract_table_name("INSERT INTO my_table VALUES (1)");
        assert_eq!(table, Some("my_table".to_string()));
    }

    #[test]
    fn test_batch_accumulation() {
        let config = BatchInsertConfig {
            max_batch_size: 3,
            ..Default::default()
        };
        let mut optimizer = BatchInsertOptimizer::new(config);

        optimizer.add_to_batch("INSERT INTO users VALUES (1, 'a')".to_string());
        optimizer.add_to_batch("INSERT INTO users VALUES (2, 'b')".to_string());
        
        let stats = optimizer.stats();
        assert_eq!(stats.pending_batches, 1);
        assert_eq!(stats.total_pending_rows, 2);

        // Third insert should trigger flush
        let flushed = optimizer.add_to_batch("INSERT INTO users VALUES (3, 'c')".to_string());
        assert!(flushed.is_some());
        assert_eq!(flushed.unwrap().len(), 3);
    }

    #[test]
    fn test_combine_inserts() {
        let inserts = vec![
            "INSERT INTO users (id, name) VALUES (1, 'Alice')".to_string(),
            "INSERT INTO users (id, name) VALUES (2, 'Bob')".to_string(),
            "INSERT INTO users (id, name) VALUES (3, 'Charlie')".to_string(),
        ];

        let combined = BatchInsertOptimizer::combine_inserts(&inserts);
        assert!(combined.is_some());
        
        let result = combined.unwrap();
        assert!(result.contains("VALUES"));
        assert!(result.contains("(1, 'Alice')"));
        assert!(result.contains("(2, 'Bob')"));
        assert!(result.contains("(3, 'Charlie')"));
    }

    #[test]
    fn test_flush_all() {
        let mut optimizer = BatchInsertOptimizer::new(BatchInsertConfig::default());

        optimizer.add_to_batch("INSERT INTO users VALUES (1)".to_string());
        optimizer.add_to_batch("INSERT INTO products VALUES (1)".to_string());

        let all_batches = optimizer.flush_all();
        assert_eq!(all_batches.len(), 2);
        
        let stats = optimizer.stats();
        assert_eq!(stats.pending_batches, 0);
    }
}
