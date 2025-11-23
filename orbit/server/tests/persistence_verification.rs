//! Verification tests to ensure data persistence across server restarts
//!
//! These tests verify that:
//! 1. Existing data files are NOT deleted on server restart
//! 2. Data persists across restarts
//! 3. RocksDB properly opens existing databases instead of recreating them

#[cfg(test)]
mod tests {
    use std::fs;
    use rocksdb::{DB, Options};
    use tempfile::TempDir;

    /// Test that RocksDB opens existing databases instead of deleting them
    #[test]
    fn test_rocksdb_reuses_existing_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // Create initial database and write data
        {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);

            let cf_descriptors = vec![
                rocksdb::ColumnFamilyDescriptor::new("test_cf", Options::default()),
            ];

            let db = DB::open_cf_descriptors(&opts, &db_path, cf_descriptors).unwrap();
            let cf = db.cf_handle("test_cf").unwrap();
            db.put_cf(cf, b"key1", b"value1").unwrap();
            db.put_cf(cf, b"key2", b"value2").unwrap();
        }

        // Verify files exist
        assert!(db_path.exists(), "Database directory should exist");
        let files_before: Vec<_> = fs::read_dir(&db_path).unwrap().collect();

        // Reopen the same database (simulating server restart)
        {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);

            let cf_descriptors = vec![
                rocksdb::ColumnFamilyDescriptor::new("test_cf", Options::default()),
            ];

            let db = DB::open_cf_descriptors(&opts, &db_path, cf_descriptors).unwrap();
            let cf = db.cf_handle("test_cf").unwrap();

            // Verify existing data is still there
            assert_eq!(db.get_cf(cf, b"key1").unwrap().unwrap(), b"value1");
            assert_eq!(db.get_cf(cf, b"key2").unwrap().unwrap(), b"value2");

            // Add new data
            db.put_cf(cf, b"key3", b"value3").unwrap();
        }

        // Verify files still exist (not deleted)
        assert!(db_path.exists(), "Database directory should still exist");
        let files_after: Vec<_> = fs::read_dir(&db_path).unwrap().collect();
        
        // Files should still be there (may have more due to WAL, but shouldn't be fewer)
        assert!(
            files_after.len() >= files_before.len(),
            "Files should not be deleted on reopen"
        );
    }

    /// Test that create_dir_all doesn't delete existing directories
    #[test]
    fn test_create_dir_all_reuses_existing() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("test_dir");

        // Create directory and a file in it
        fs::create_dir_all(&dir_path).unwrap();
        let test_file = dir_path.join("test.txt");
        fs::write(&test_file, "test data").unwrap();

        assert!(test_file.exists(), "Test file should exist");

        // Call create_dir_all again (simulating server restart)
        fs::create_dir_all(&dir_path).unwrap();

        // Verify file still exists (not deleted)
        assert!(
            test_file.exists(),
            "Existing file should not be deleted by create_dir_all"
        );
        assert_eq!(
            fs::read_to_string(&test_file).unwrap(),
            "test data",
            "File content should be preserved"
        );
    }

    /// Test AQL storage persistence
    #[tokio::test]
    async fn test_aql_storage_persistence() {
        use orbit_server::protocols::aql::storage::AqlStorage;
        use orbit_server::protocols::aql::data_model::{AqlDocument, AqlValue, AqlCollection, CollectionType, CollectionStatus};
        use std::collections::HashMap;

        let temp_dir = TempDir::new().unwrap();
        let storage1 = AqlStorage::new(temp_dir.path());
        storage1.initialize().await.unwrap();

        // Create collection and document
        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage1.store_collection(collection).await.unwrap();

        let mut data = HashMap::new();
        data.insert("name".to_string(), AqlValue::String("Alice".to_string()));
        let doc = AqlDocument::new("users", "alice".to_string(), data);
        storage1.store_document(doc).await.unwrap();

        // Verify files exist
        let rocksdb_path = temp_dir.path().join("rocksdb");
        assert!(rocksdb_path.exists(), "RocksDB directory should exist");

        // Explicitly shutdown storage1 to release RocksDB lock
        storage1.shutdown().await.unwrap();
        // Drop storage1 to ensure it's fully cleaned up
        drop(storage1);

        // Create new storage instance (simulating server restart)
        let storage2 = AqlStorage::new(temp_dir.path());
        storage2.initialize().await.unwrap();

        // Verify data persists
        let retrieved = storage2.get_document("users", "alice").await.unwrap();
        assert!(retrieved.is_some(), "Document should persist across restarts");
        assert_eq!(retrieved.unwrap().key, "alice");
    }

    /// Test Cypher storage persistence
    #[tokio::test]
    async fn test_cypher_storage_persistence() {
        use orbit_server::protocols::cypher::storage::{CypherGraphStorage, GraphNode};

        let temp_dir = TempDir::new().unwrap();
        let storage1 = CypherGraphStorage::new(temp_dir.path());
        storage1.initialize().await.unwrap();

        // Create a node
        let mut properties = std::collections::HashMap::new();
        properties.insert("name".to_string(), serde_json::json!("Alice"));
        let node = GraphNode {
            id: "alice".to_string(),
            labels: vec!["Person".to_string()],
            properties: properties.clone(),
        };
        storage1.store_node(node).await.unwrap();

        // Verify files exist
        let rocksdb_path = temp_dir.path().join("rocksdb");
        assert!(rocksdb_path.exists(), "RocksDB directory should exist");

        // Explicitly shutdown storage1 to release RocksDB lock
        storage1.shutdown().await.unwrap();
        // Drop storage1 to ensure it's fully cleaned up
        drop(storage1);

        // Create new storage instance (simulating server restart)
        let storage2 = CypherGraphStorage::new(temp_dir.path());
        storage2.initialize().await.unwrap();

        // Verify data persists
        let node = storage2.get_node("alice").await.unwrap();
        assert!(node.is_some(), "Node should persist across restarts");
        assert_eq!(node.unwrap().id, "alice");
    }

    /// Test that server initialization doesn't delete existing data directories
    #[test]
    fn test_server_initialization_preserves_data() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");

        // Create data directory structure with existing files
        let postgres_dir = data_dir.join("postgresql").join("rocksdb");
        let redis_dir = data_dir.join("redis").join("rocksdb");
        let mysql_dir = data_dir.join("mysql").join("rocksdb");
        let cql_dir = data_dir.join("cql").join("rocksdb");
        let cypher_dir = data_dir.join("cypher").join("rocksdb");
        let aql_dir = data_dir.join("aql").join("rocksdb");
        let graphrag_dir = data_dir.join("graphrag").join("rocksdb");

        for dir in &[&postgres_dir, &redis_dir, &mysql_dir, &cql_dir, &cypher_dir, &aql_dir, &graphrag_dir] {
            fs::create_dir_all(dir).unwrap();
            let test_file = dir.join("test.txt");
            fs::write(&test_file, "existing data").unwrap();
        }

        // Simulate server initialization (create_dir_all for each directory)
        // This is what the server does - it calls create_dir_all which doesn't delete
        for dir in &[&postgres_dir, &redis_dir, &mysql_dir, &cql_dir, &cypher_dir, &aql_dir, &graphrag_dir] {
            fs::create_dir_all(dir).unwrap();
        }

        // Verify all test files still exist
        for dir in &[&postgres_dir, &redis_dir, &mysql_dir, &cql_dir, &cypher_dir, &aql_dir, &graphrag_dir] {
            let test_file = dir.join("test.txt");
            assert!(
                test_file.exists(),
                "Existing file in {:?} should not be deleted",
                dir
            );
            assert_eq!(
                fs::read_to_string(&test_file).unwrap(),
                "existing data",
                "File content in {:?} should be preserved",
                dir
            );
        }
    }

    /// Test RocksDB with create_if_missing behavior
    #[test]
    fn test_rocksdb_create_if_missing_behavior() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");

        // First open: creates database
        {
            let mut opts = Options::default();
            opts.create_if_missing(true);
            opts.create_missing_column_families(true);

            let cf_descriptors = vec![
                rocksdb::ColumnFamilyDescriptor::new("cf1", Options::default()),
            ];

            let db = DB::open_cf_descriptors(&opts, &db_path, cf_descriptors).unwrap();
            let cf = db.cf_handle("cf1").unwrap();
            db.put_cf(cf, b"test", b"data").unwrap();
        }

        // Verify database exists
        assert!(db_path.exists());

        // Second open: should reuse existing database, not recreate
        {
            let mut opts = Options::default();
            opts.create_if_missing(true); // This is safe - only creates if missing
            opts.create_missing_column_families(true);

            let cf_descriptors = vec![
                rocksdb::ColumnFamilyDescriptor::new("cf1", Options::default()),
            ];

            let db = DB::open_cf_descriptors(&opts, &db_path, cf_descriptors).unwrap();
            let cf = db.cf_handle("cf1").unwrap();

            // Verify existing data is preserved
            let value = db.get_cf(cf, b"test").unwrap();
            assert_eq!(value.unwrap(), b"data", "Existing data should be preserved");
        }
    }
}

