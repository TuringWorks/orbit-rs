// Comprehensive tests for AQL data modification operations
// Tests INSERT, UPDATE, REMOVE, REPLACE, and UPSERT

#[cfg(test)]
mod aql_data_modification_tests {
    use crate::protocols::aql::{AqlCollection, AqlQueryEngine, AqlStorage, AqlValue};
    use crate::protocols::aql::data_model::{CollectionStatus, CollectionType};
    use std::sync::Arc;
    use tempfile::TempDir;

    async fn create_test_storage() -> (Arc<AqlStorage>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(AqlStorage::new(temp_dir.path()));
        storage.initialize().await.unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_insert_document() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        // Create a collection first
        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // Test INSERT with explicit key
        let query = r#"INSERT { _key: "user1", name: "Alice", age: 30 } INTO users"#;
        let result = engine.execute_query(query).await;

        assert!(result.is_ok(), "INSERT should succeed: {:?}", result.err());
        let result = result.unwrap();
        assert_eq!(result.data.len(), 1, "Should return 1 inserted document");

        // Verify the document was stored
        let doc = storage
            .get_document("users", "user1")
            .await
            .expect("Failed to get document");
        assert!(doc.is_some(), "Document should exist");

        let doc = doc.unwrap();
        assert_eq!(doc.key, "user1");
        if let Some(AqlValue::String(name)) = doc.data.get("name") {
            assert_eq!(name, "Alice");
        } else {
            panic!("Name field should be a string");
        }
    }

    #[tokio::test]
    async fn test_insert_auto_generated_key() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "products".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // INSERT without explicit key - should auto-generate
        let query = r#"INSERT { name: "Widget", price: 19.99 } INTO products"#;
        let result = engine.execute_query(query).await;

        assert!(result.is_ok(), "INSERT with auto-key should succeed");
        let result = result.unwrap();
        assert_eq!(result.data.len(), 1);

        // Verify a document was created
        let docs = storage
            .get_collection_documents("products")
            .await
            .expect("Failed to get documents");
        assert_eq!(docs.len(), 1, "Should have 1 document");
        assert!(!docs[0].key.is_empty(), "Key should be auto-generated");
    }

    #[tokio::test]
    async fn test_update_document() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // Insert a document first
        let insert_query = r#"INSERT { _key: "user1", name: "Alice", age: 30 } INTO users"#;
        engine
            .execute_query(insert_query)
            .await
            .expect("INSERT failed");

        // Update the document
        let update_query = r#"UPDATE "user1" WITH { age: 31, city: "NYC" } IN users"#;
        let result = engine.execute_query(update_query).await;

        assert!(result.is_ok(), "UPDATE should succeed: {:?}", result.err());

        // Verify the update
        let doc = storage
            .get_document("users", "user1")
            .await
            .expect("Failed to get document")
            .expect("Document should exist");

        if let Some(AqlValue::Number(age)) = doc.data.get("age") {
            assert_eq!(age.as_u64(), Some(31), "Age should be updated to 31");
        } else {
            panic!("Age should be a number");
        }

        if let Some(AqlValue::String(city)) = doc.data.get("city") {
            assert_eq!(city, "NYC", "City should be added");
        } else {
            panic!("City should be a string");
        }

        // Original name should still be there
        assert!(doc.data.contains_key("name"), "Name should still exist");
    }

    #[tokio::test]
    async fn test_replace_document() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // Insert a document
        let insert_query =
            r#"INSERT { _key: "user1", name: "Alice", age: 30, city: "SF" } INTO users"#;
        engine
            .execute_query(insert_query)
            .await
            .expect("INSERT failed");

        // Replace the document (should remove old fields)
        let replace_query = r#"REPLACE "user1" WITH { name: "Alice Smith", email: "alice@example.com" } IN users"#;
        let result = engine.execute_query(replace_query).await;

        assert!(
            result.is_ok(),
            "REPLACE should succeed: {:?}",
            result.err()
        );

        // Verify the replacement
        let doc = storage
            .get_document("users", "user1")
            .await
            .expect("Failed to get document")
            .expect("Document should exist");

        assert_eq!(doc.data.len(), 2, "Should only have 2 fields");
        assert!(doc.data.contains_key("name"), "Name should exist");
        assert!(doc.data.contains_key("email"), "Email should exist");
        assert!(!doc.data.contains_key("age"), "Age should be removed");
        assert!(!doc.data.contains_key("city"), "City should be removed");
    }

    #[tokio::test]
    async fn test_remove_document() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // Insert a document
        let insert_query = r#"INSERT { _key: "user1", name: "Alice" } INTO users"#;
        engine
            .execute_query(insert_query)
            .await
            .expect("INSERT failed");

        // Verify it exists
        assert!(
            storage
                .document_exists("users", "user1")
                .await,
            "Document should exist before removal"
        );

        // Remove the document
        let remove_query = r#"REMOVE "user1" IN users"#;
        let result = engine.execute_query(remove_query).await;

        assert!(
            result.is_ok(),
            "REMOVE should succeed: {:?}",
            result.err()
        );

        // Verify it's gone
        assert!(
            !storage.document_exists("users", "user1").await,
            "Document should not exist after removal"
        );
    }

    #[tokio::test]
    async fn test_upsert_insert() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // UPSERT when document doesn't exist - should INSERT
        let upsert_query = r#"
            UPSERT { _key: "user1" }
            INSERT { _key: "user1", name: "Alice", age: 30 }
            UPDATE { age: 31 }
            IN users
        "#;
        let result = engine.execute_query(upsert_query).await;

        assert!(
            result.is_ok(),
            "UPSERT (insert) should succeed: {:?}",
            result.err()
        );

        // Verify document was inserted
        let doc = storage
            .get_document("users", "user1")
            .await
            .expect("Failed to get document")
            .expect("Document should exist");

        if let Some(AqlValue::Number(age)) = doc.data.get("age") {
            assert_eq!(age.as_u64(), Some(30), "Age should be 30 (from INSERT)");
        }
    }

    #[tokio::test]
    async fn test_upsert_update() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // Insert a document first
        let insert_query = r#"INSERT { _key: "user1", name: "Alice", age: 30 } INTO users"#;
        engine
            .execute_query(insert_query)
            .await
            .expect("INSERT failed");

        // UPSERT when document exists - should UPDATE
        let upsert_query = r#"
            UPSERT { _key: "user1" }
            INSERT { _key: "user1", name: "Bob", age: 25 }
            UPDATE { age: 31 }
            IN users
        "#;
        let result = engine.execute_query(upsert_query).await;

        assert!(
            result.is_ok(),
            "UPSERT (update) should succeed: {:?}",
            result.err()
        );

        // Verify document was updated, not replaced
        let doc = storage
            .get_document("users", "user1")
            .await
            .expect("Failed to get document")
            .expect("Document should exist");

        if let Some(AqlValue::String(name)) = doc.data.get("name") {
            assert_eq!(name, "Alice", "Name should still be Alice (not replaced)");
        }

        if let Some(AqlValue::Number(age)) = doc.data.get("age") {
            assert_eq!(age.as_u64(), Some(31), "Age should be updated to 31");
        }
    }

    #[tokio::test]
    async fn test_for_update_loop() {
        let (storage, _temp_dir) = create_test_storage().await;
        let engine = AqlQueryEngine::with_storage(storage.clone());

        let collection = AqlCollection {
            name: "users".to_string(),
            collection_type: CollectionType::Document,
            status: CollectionStatus::Loaded,
            count: 0,
            indexes: vec![],
        };
        storage
            .store_collection(collection)
            .await
            .expect("Failed to create collection");

        // Insert multiple documents
        for i in 1..=3 {
            let query = format!(
                r#"INSERT {{ _key: "user{}", name: "User{}", score: {} }} INTO users"#,
                i,
                i,
                i * 10
            );
            engine.execute_query(&query).await.expect("INSERT failed");
        }

        // Update all documents in a loop
        let update_query = r#"
            FOR u IN users
            UPDATE u._key WITH { score: u.score + 5 } IN users
        "#;
        let result = engine.execute_query(update_query).await;

        assert!(
            result.is_ok(),
            "FOR UPDATE should succeed: {:?}",
            result.err()
        );

        // Verify all documents were updated
        for i in 1..=3 {
            let doc = storage
                .get_document("users", &format!("user{}", i))
                .await
                .expect("Failed to get document")
                .expect("Document should exist");

            if let Some(AqlValue::Number(score)) = doc.data.get("score") {
                assert_eq!(
                    score.as_u64(),
                    Some((i * 10 + 5) as u64),
                    "Score should be incremented by 5"
                );
            }
        }
    }
}
