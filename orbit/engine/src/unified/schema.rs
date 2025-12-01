//! Schema Registry for Unified Storage
//!
//! This module provides schema management for cross-protocol data sharing.
//! It tracks namespace schemas, field definitions, and provides protocol-specific
//! projections of the unified data model.
//!
//! # Key Responsibilities
//!
//! - **Namespace Management**: Create, modify, and drop namespaces with schemas
//! - **Schema Evolution**: Version-controlled schema changes with migration support
//! - **Protocol Projections**: How each protocol views the unified data
//! - **Type Mappings**: Convert between protocol-native types and UniversalValue
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Schema Registry                             │
//! │                                                                  │
//! │  ┌──────────────────┐  ┌──────────────────┐  ┌───────────────┐  │
//! │  │    Namespace     │  │     Protocol     │  │    Schema     │  │
//! │  │    Definitions   │  │   Projections    │  │   Versions    │  │
//! │  └──────────────────┘  └──────────────────┘  └───────────────┘  │
//! │                                                                  │
//! │  Stores:                                                         │
//! │  - schema:{namespace} -> NamespaceSchema                         │
//! │  - projection:{protocol}:{namespace} -> ProtocolProjection       │
//! │  - version:{namespace}:{version} -> SchemaVersion                │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use super::operations::{FieldDefinition, FieldType, IndexDefinition, NamespaceSchema};
use super::storage::{UnifiedStorage, UnifiedStorageError, UnifiedStorageResult};
use super::types::UniversalValue;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Supported protocols for projections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    /// Redis/RESP protocol
    Redis,
    /// PostgreSQL wire protocol
    PostgreSQL,
    /// MySQL protocol
    MySQL,
    /// Cassandra Query Language
    CQL,
    /// Neo4j Cypher
    Cypher,
    /// ArangoDB Query Language
    AQL,
    /// HTTP REST API
    REST,
    /// gRPC
    GRPC,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Redis => write!(f, "redis"),
            Protocol::PostgreSQL => write!(f, "postgresql"),
            Protocol::MySQL => write!(f, "mysql"),
            Protocol::CQL => write!(f, "cql"),
            Protocol::Cypher => write!(f, "cypher"),
            Protocol::AQL => write!(f, "aql"),
            Protocol::REST => write!(f, "rest"),
            Protocol::GRPC => write!(f, "grpc"),
        }
    }
}

/// Schema Registry for managing namespace schemas
pub struct SchemaRegistry {
    /// In-memory schema cache for fast lookups
    schemas: Arc<RwLock<HashMap<String, NamespaceSchema>>>,
    /// Protocol-specific projections
    projections: Arc<RwLock<HashMap<(Protocol, String), ProtocolProjection>>>,
    /// Optional backing storage for persistence
    storage: Option<Arc<UnifiedStorage>>,
}

/// How a specific protocol views a namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolProjection {
    /// The namespace this projection is for
    pub namespace: String,
    /// The protocol this projection is for
    pub protocol: Protocol,
    /// Field mappings: protocol field name -> universal field name
    pub field_mappings: HashMap<String, String>,
    /// Type overrides: field name -> protocol-specific type hint
    pub type_hints: HashMap<String, String>,
    /// Whether this namespace is visible to this protocol
    pub visible: bool,
    /// Protocol-specific metadata
    pub metadata: BTreeMap<String, String>,
}

impl ProtocolProjection {
    /// Create a default projection (all fields visible, direct mapping)
    pub fn default_for(namespace: &str, protocol: Protocol, schema: &NamespaceSchema) -> Self {
        let field_mappings: HashMap<String, String> = schema
            .fields
            .iter()
            .map(|f| (f.name.clone(), f.name.clone()))
            .collect();

        Self {
            namespace: namespace.to_string(),
            protocol,
            field_mappings,
            type_hints: HashMap::new(),
            visible: true,
            metadata: BTreeMap::new(),
        }
    }
}

/// Schema version for tracking changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersion {
    /// Version number
    pub version: u32,
    /// The schema at this version
    pub schema: NamespaceSchema,
    /// Timestamp when this version was created
    pub created_at: i64,
    /// Optional migration script from previous version
    pub migration: Option<String>,
    /// Description of changes
    pub description: String,
}

impl SchemaRegistry {
    /// Create a new schema registry (in-memory only)
    pub fn new() -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            projections: Arc::new(RwLock::new(HashMap::new())),
            storage: None,
        }
    }

    /// Create a schema registry backed by unified storage
    pub fn with_storage(storage: Arc<UnifiedStorage>) -> Self {
        Self {
            schemas: Arc::new(RwLock::new(HashMap::new())),
            projections: Arc::new(RwLock::new(HashMap::new())),
            storage: Some(storage),
        }
    }

    /// Initialize the registry, loading schemas from storage if available
    pub async fn initialize(&self) -> UnifiedStorageResult<()> {
        if let Some(ref storage) = self.storage {
            // Load all schemas from storage
            let result = storage.scan_keys("__schema__", None, None).await?;

            if let super::types::UniversalResult::Values(keys) = result {
                for key in keys {
                    if let UniversalValue::String(schema_key) = key {
                        let result = storage.get("__schema__", &schema_key).await?;
                        if let super::types::UniversalResult::Record(record) = result {
                            if let UniversalValue::String(json) = record.value {
                                if let Ok(schema) = serde_json::from_str::<NamespaceSchema>(&json) {
                                    let mut schemas = self.schemas.write().await;
                                    schemas.insert(schema_key.clone(), schema.clone());

                                    // Create default projections for all protocols
                                    self.create_default_projections(&schema_key, &schema).await;
                                }
                            }
                        }
                    }
                }
            }
        }

        tracing::info!("Schema registry initialized");
        Ok(())
    }

    // ============================================================================
    // Namespace Management
    // ============================================================================

    /// Create a new namespace with the given schema
    pub async fn create_namespace(
        &self,
        name: &str,
        schema: NamespaceSchema,
    ) -> UnifiedStorageResult<()> {
        // Check if namespace already exists
        {
            let schemas = self.schemas.read().await;
            if schemas.contains_key(name) {
                return Err(UnifiedStorageError::NamespaceExists(name.to_string()));
            }
        }

        // Store in memory
        {
            let mut schemas = self.schemas.write().await;
            schemas.insert(name.to_string(), schema.clone());
        }

        // Persist to storage if available
        if let Some(ref storage) = self.storage {
            let json = serde_json::to_string(&schema)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;
            storage.put(
                "__schema__",
                name,
                UniversalValue::String(json),
                None,
                false,
                None,
            ).await?;
        }

        // Create default projections for all protocols
        self.create_default_projections(name, &schema).await;

        tracing::info!("Created namespace: {}", name);
        Ok(())
    }

    /// Drop a namespace
    pub async fn drop_namespace(&self, name: &str) -> UnifiedStorageResult<bool> {
        let removed = {
            let mut schemas = self.schemas.write().await;
            schemas.remove(name).is_some()
        };

        if removed {
            // Remove from storage
            if let Some(ref storage) = self.storage {
                storage.delete("__schema__", name).await?;
            }

            // Remove all projections for this namespace
            {
                let mut projections = self.projections.write().await;
                projections.retain(|(_, ns), _| ns != name);
            }

            tracing::info!("Dropped namespace: {}", name);
        }

        Ok(removed)
    }

    /// Get a namespace schema
    pub async fn get_schema(&self, name: &str) -> Option<NamespaceSchema> {
        let schemas = self.schemas.read().await;
        schemas.get(name).cloned()
    }

    /// List all namespaces
    pub async fn list_namespaces(&self) -> Vec<String> {
        let schemas = self.schemas.read().await;
        schemas.keys().cloned().collect()
    }

    /// Check if a namespace exists
    pub async fn namespace_exists(&self, name: &str) -> bool {
        let schemas = self.schemas.read().await;
        schemas.contains_key(name)
    }

    // ============================================================================
    // Schema Updates
    // ============================================================================

    /// Add a field to a namespace schema
    pub async fn add_field(
        &self,
        namespace: &str,
        field: FieldDefinition,
    ) -> UnifiedStorageResult<()> {
        let mut schemas = self.schemas.write().await;

        let schema = schemas.get_mut(namespace).ok_or_else(|| {
            UnifiedStorageError::NamespaceNotFound(namespace.to_string())
        })?;

        // Check for duplicate field
        if schema.fields.iter().any(|f| f.name == field.name) {
            return Err(UnifiedStorageError::InvalidOperation(format!(
                "Field '{}' already exists in namespace '{}'",
                field.name, namespace
            )));
        }

        schema.fields.push(field);

        // Persist to storage
        if let Some(ref storage) = self.storage {
            let json = serde_json::to_string(&*schema)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;
            storage.put(
                "__schema__",
                namespace,
                UniversalValue::String(json),
                None,
                false,
                None,
            ).await?;
        }

        Ok(())
    }

    /// Remove a field from a namespace schema
    pub async fn remove_field(
        &self,
        namespace: &str,
        field_name: &str,
    ) -> UnifiedStorageResult<bool> {
        let mut schemas = self.schemas.write().await;

        let schema = schemas.get_mut(namespace).ok_or_else(|| {
            UnifiedStorageError::NamespaceNotFound(namespace.to_string())
        })?;

        // Check if field is part of primary key
        if schema.primary_key.contains(&field_name.to_string()) {
            return Err(UnifiedStorageError::InvalidOperation(format!(
                "Cannot remove field '{}' - it is part of the primary key",
                field_name
            )));
        }

        let original_len = schema.fields.len();
        schema.fields.retain(|f| f.name != field_name);
        let removed = schema.fields.len() < original_len;

        // Persist to storage
        if removed {
            if let Some(ref storage) = self.storage {
                let json = serde_json::to_string(&*schema)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;
                storage.put(
                    "__schema__",
                    namespace,
                    UniversalValue::String(json),
                    None,
                    false,
                    None,
                ).await?;
            }
        }

        Ok(removed)
    }

    /// Add an index to a namespace
    pub async fn add_index(
        &self,
        namespace: &str,
        index: IndexDefinition,
    ) -> UnifiedStorageResult<()> {
        let mut schemas = self.schemas.write().await;

        let schema = schemas.get_mut(namespace).ok_or_else(|| {
            UnifiedStorageError::NamespaceNotFound(namespace.to_string())
        })?;

        // Check for duplicate index name
        if schema.indexes.iter().any(|i| i.name == index.name) {
            return Err(UnifiedStorageError::InvalidOperation(format!(
                "Index '{}' already exists in namespace '{}'",
                index.name, namespace
            )));
        }

        // Verify all indexed fields exist
        for field_name in &index.fields {
            if !schema.fields.iter().any(|f| &f.name == field_name) {
                return Err(UnifiedStorageError::InvalidOperation(format!(
                    "Field '{}' does not exist in namespace '{}'",
                    field_name, namespace
                )));
            }
        }

        schema.indexes.push(index);

        // Persist to storage
        if let Some(ref storage) = self.storage {
            let json = serde_json::to_string(&*schema)
                .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;
            storage.put(
                "__schema__",
                namespace,
                UniversalValue::String(json),
                None,
                false,
                None,
            ).await?;
        }

        Ok(())
    }

    /// Remove an index from a namespace
    pub async fn remove_index(
        &self,
        namespace: &str,
        index_name: &str,
    ) -> UnifiedStorageResult<bool> {
        let mut schemas = self.schemas.write().await;

        let schema = schemas.get_mut(namespace).ok_or_else(|| {
            UnifiedStorageError::NamespaceNotFound(namespace.to_string())
        })?;

        let original_len = schema.indexes.len();
        schema.indexes.retain(|i| i.name != index_name);
        let removed = schema.indexes.len() < original_len;

        // Persist to storage
        if removed {
            if let Some(ref storage) = self.storage {
                let json = serde_json::to_string(&*schema)
                    .map_err(|e| UnifiedStorageError::Serialization(e.to_string()))?;
                storage.put(
                    "__schema__",
                    namespace,
                    UniversalValue::String(json),
                    None,
                    false,
                    None,
                ).await?;
            }
        }

        Ok(removed)
    }

    // ============================================================================
    // Protocol Projections
    // ============================================================================

    /// Get the projection for a protocol and namespace
    pub async fn get_projection(
        &self,
        protocol: Protocol,
        namespace: &str,
    ) -> Option<ProtocolProjection> {
        let projections = self.projections.read().await;
        projections.get(&(protocol, namespace.to_string())).cloned()
    }

    /// Set a custom projection for a protocol and namespace
    pub async fn set_projection(&self, projection: ProtocolProjection) -> UnifiedStorageResult<()> {
        let key = (projection.protocol, projection.namespace.clone());

        let mut projections = self.projections.write().await;
        projections.insert(key, projection);

        Ok(())
    }

    /// Create default projections for all protocols
    async fn create_default_projections(&self, namespace: &str, schema: &NamespaceSchema) {
        let protocols = [
            Protocol::Redis,
            Protocol::PostgreSQL,
            Protocol::MySQL,
            Protocol::CQL,
            Protocol::Cypher,
            Protocol::AQL,
            Protocol::REST,
            Protocol::GRPC,
        ];

        let mut projections = self.projections.write().await;

        for protocol in protocols {
            let projection = ProtocolProjection::default_for(namespace, protocol, schema);
            projections.insert((protocol, namespace.to_string()), projection);
        }
    }

    /// Check if a namespace is visible to a protocol
    pub async fn is_visible(&self, protocol: Protocol, namespace: &str) -> bool {
        let projections = self.projections.read().await;
        projections
            .get(&(protocol, namespace.to_string()))
            .map(|p| p.visible)
            .unwrap_or(false)
    }

    /// Set visibility of a namespace for a protocol
    pub async fn set_visibility(
        &self,
        protocol: Protocol,
        namespace: &str,
        visible: bool,
    ) -> UnifiedStorageResult<()> {
        let mut projections = self.projections.write().await;

        if let Some(projection) = projections.get_mut(&(protocol, namespace.to_string())) {
            projection.visible = visible;
            Ok(())
        } else {
            Err(UnifiedStorageError::NamespaceNotFound(namespace.to_string()))
        }
    }

    // ============================================================================
    // Type Mappings
    // ============================================================================

    /// Get the protocol-native type name for a field
    pub fn get_protocol_type(&self, protocol: Protocol, field_type: &FieldType) -> String {
        match protocol {
            Protocol::PostgreSQL | Protocol::MySQL => self.sql_type(field_type),
            Protocol::Redis => self.redis_type(field_type),
            Protocol::CQL => self.cql_type(field_type),
            Protocol::Cypher | Protocol::AQL => self.graph_type(field_type),
            Protocol::REST | Protocol::GRPC => self.json_type(field_type),
        }
    }

    fn sql_type(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::Bool => "BOOLEAN".to_string(),
            FieldType::Int => "BIGINT".to_string(),
            FieldType::Float => "DOUBLE PRECISION".to_string(),
            FieldType::String => "TEXT".to_string(),
            FieldType::Bytes => "BYTEA".to_string(),
            FieldType::Timestamp => "TIMESTAMP WITH TIME ZONE".to_string(),
            FieldType::Date => "DATE".to_string(),
            FieldType::Time => "TIME".to_string(),
            FieldType::Json => "JSONB".to_string(),
            FieldType::List(_) => "JSONB".to_string(), // Arrays stored as JSON
            FieldType::Map(_, _) => "JSONB".to_string(),
            FieldType::Vector(dim) => format!("VECTOR({})", dim),
            FieldType::Uuid => "UUID".to_string(),
        }
    }

    fn redis_type(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::Bool => "string".to_string(),
            FieldType::Int => "string".to_string(),
            FieldType::Float => "string".to_string(),
            FieldType::String => "string".to_string(),
            FieldType::Bytes => "string".to_string(),
            FieldType::List(_) => "list".to_string(),
            FieldType::Map(_, _) => "hash".to_string(),
            _ => "string".to_string(),
        }
    }

    fn cql_type(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::Bool => "boolean".to_string(),
            FieldType::Int => "bigint".to_string(),
            FieldType::Float => "double".to_string(),
            FieldType::String => "text".to_string(),
            FieldType::Bytes => "blob".to_string(),
            FieldType::Timestamp => "timestamp".to_string(),
            FieldType::Date => "date".to_string(),
            FieldType::Time => "time".to_string(),
            FieldType::Json => "text".to_string(),
            FieldType::List(inner) => format!("list<{}>", self.cql_type(inner)),
            FieldType::Map(k, v) => format!("map<{}, {}>", self.cql_type(k), self.cql_type(v)),
            FieldType::Vector(_) => "list<float>".to_string(),
            FieldType::Uuid => "uuid".to_string(),
        }
    }

    fn graph_type(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::Bool => "Boolean".to_string(),
            FieldType::Int => "Integer".to_string(),
            FieldType::Float => "Float".to_string(),
            FieldType::String => "String".to_string(),
            FieldType::List(_) => "List".to_string(),
            FieldType::Map(_, _) => "Map".to_string(),
            _ => "Any".to_string(),
        }
    }

    fn json_type(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::Bool => "boolean".to_string(),
            FieldType::Int => "integer".to_string(),
            FieldType::Float => "number".to_string(),
            FieldType::String => "string".to_string(),
            FieldType::Bytes => "string".to_string(), // base64 encoded
            FieldType::List(_) => "array".to_string(),
            FieldType::Map(_, _) => "object".to_string(),
            _ => "string".to_string(),
        }
    }

    // ============================================================================
    // Schema Inference
    // ============================================================================

    /// Infer a schema from a sample record
    pub fn infer_schema(namespace: &str, sample: &UniversalValue) -> Option<NamespaceSchema> {
        if let UniversalValue::Map(fields) = sample {
            let field_defs: Vec<FieldDefinition> = fields
                .iter()
                .map(|(name, value)| FieldDefinition {
                    name: name.clone(),
                    field_type: Self::infer_field_type(value),
                    nullable: true,
                    default: None,
                })
                .collect();

            Some(NamespaceSchema {
                fields: field_defs,
                primary_key: vec!["id".to_string()], // Default to "id" as primary key
                indexes: Vec::new(),
            })
        } else {
            None
        }
    }

    fn infer_field_type(value: &UniversalValue) -> FieldType {
        match value {
            UniversalValue::Null => FieldType::String, // Default nullable field
            UniversalValue::Bool(_) => FieldType::Bool,
            UniversalValue::Int(_) => FieldType::Int,
            UniversalValue::Float(_) => FieldType::Float,
            UniversalValue::String(_) => FieldType::String,
            UniversalValue::Bytes(_) => FieldType::Bytes,
            UniversalValue::List(items) => {
                let inner_type = items.first()
                    .map(Self::infer_field_type)
                    .unwrap_or(FieldType::String);
                FieldType::List(Box::new(inner_type))
            }
            UniversalValue::Map(_, ..) => FieldType::Json,
            UniversalValue::Set(_) => FieldType::List(Box::new(FieldType::String)),
            UniversalValue::SortedSet(_) => FieldType::List(Box::new(FieldType::String)),
            UniversalValue::Timestamp(_) => FieldType::Timestamp,
            UniversalValue::Date(_) => FieldType::Date,
            UniversalValue::Time(_) => FieldType::Time,
            UniversalValue::Duration(_) => FieldType::Int,
            UniversalValue::Node { .. } => FieldType::Json,
            UniversalValue::Relationship { .. } => FieldType::Json,
            UniversalValue::Path(_) => FieldType::Json,
            UniversalValue::Point { .. } => FieldType::Json,
            UniversalValue::Polygon(_) => FieldType::Json,
            UniversalValue::BoundingBox { .. } => FieldType::Json,
            UniversalValue::Vector(v) => FieldType::Vector(v.len()),
            UniversalValue::Uuid(_) => FieldType::Uuid,
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::operations::IndexType;

    fn create_test_schema() -> NamespaceSchema {
        NamespaceSchema {
            fields: vec![
                FieldDefinition {
                    name: "id".to_string(),
                    field_type: FieldType::Int,
                    nullable: false,
                    default: None,
                },
                FieldDefinition {
                    name: "name".to_string(),
                    field_type: FieldType::String,
                    nullable: false,
                    default: None,
                },
                FieldDefinition {
                    name: "email".to_string(),
                    field_type: FieldType::String,
                    nullable: true,
                    default: None,
                },
            ],
            primary_key: vec!["id".to_string()],
            indexes: vec![],
        }
    }

    #[tokio::test]
    async fn test_create_namespace() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema.clone()).await.unwrap();

        assert!(registry.namespace_exists("users").await);
        let retrieved = registry.get_schema("users").await.unwrap();
        assert_eq!(retrieved.fields.len(), 3);
    }

    #[tokio::test]
    async fn test_duplicate_namespace() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema.clone()).await.unwrap();
        let result = registry.create_namespace("users", schema).await;

        assert!(matches!(result, Err(UnifiedStorageError::NamespaceExists(_))));
    }

    #[tokio::test]
    async fn test_drop_namespace() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();
        assert!(registry.namespace_exists("users").await);

        let removed = registry.drop_namespace("users").await.unwrap();
        assert!(removed);
        assert!(!registry.namespace_exists("users").await);
    }

    #[tokio::test]
    async fn test_add_field() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();

        let new_field = FieldDefinition {
            name: "age".to_string(),
            field_type: FieldType::Int,
            nullable: true,
            default: None,
        };

        registry.add_field("users", new_field).await.unwrap();

        let updated = registry.get_schema("users").await.unwrap();
        assert_eq!(updated.fields.len(), 4);
        assert!(updated.fields.iter().any(|f| f.name == "age"));
    }

    #[tokio::test]
    async fn test_remove_field() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();

        let removed = registry.remove_field("users", "email").await.unwrap();
        assert!(removed);

        let updated = registry.get_schema("users").await.unwrap();
        assert_eq!(updated.fields.len(), 2);
        assert!(!updated.fields.iter().any(|f| f.name == "email"));
    }

    #[tokio::test]
    async fn test_cannot_remove_primary_key_field() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();

        let result = registry.remove_field("users", "id").await;
        assert!(matches!(result, Err(UnifiedStorageError::InvalidOperation(_))));
    }

    #[tokio::test]
    async fn test_add_index() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();

        let index = IndexDefinition {
            name: "idx_email".to_string(),
            fields: vec!["email".to_string()],
            unique: true,
            index_type: IndexType::BTree,
        };

        registry.add_index("users", index).await.unwrap();

        let updated = registry.get_schema("users").await.unwrap();
        assert_eq!(updated.indexes.len(), 1);
        assert_eq!(updated.indexes[0].name, "idx_email");
    }

    #[tokio::test]
    async fn test_protocol_projections() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();

        // Check that default projections were created
        let pg_projection = registry.get_projection(Protocol::PostgreSQL, "users").await;
        assert!(pg_projection.is_some());

        let projection = pg_projection.unwrap();
        assert!(projection.visible);
        assert_eq!(projection.field_mappings.len(), 3);
    }

    #[tokio::test]
    async fn test_visibility() {
        let registry = SchemaRegistry::new();
        let schema = create_test_schema();

        registry.create_namespace("users", schema).await.unwrap();

        assert!(registry.is_visible(Protocol::Redis, "users").await);

        registry.set_visibility(Protocol::Redis, "users", false).await.unwrap();

        assert!(!registry.is_visible(Protocol::Redis, "users").await);
    }

    #[tokio::test]
    async fn test_type_mappings() {
        let registry = SchemaRegistry::new();

        // PostgreSQL types
        assert_eq!(registry.get_protocol_type(Protocol::PostgreSQL, &FieldType::Int), "BIGINT");
        assert_eq!(registry.get_protocol_type(Protocol::PostgreSQL, &FieldType::String), "TEXT");
        assert_eq!(registry.get_protocol_type(Protocol::PostgreSQL, &FieldType::Vector(128)), "VECTOR(128)");

        // CQL types
        assert_eq!(registry.get_protocol_type(Protocol::CQL, &FieldType::Int), "bigint");
        assert_eq!(registry.get_protocol_type(Protocol::CQL, &FieldType::String), "text");

        // Redis types
        assert_eq!(registry.get_protocol_type(Protocol::Redis, &FieldType::List(Box::new(FieldType::String))), "list");
        assert_eq!(registry.get_protocol_type(Protocol::Redis, &FieldType::Map(Box::new(FieldType::String), Box::new(FieldType::String))), "hash");
    }

    #[tokio::test]
    async fn test_infer_schema() {
        let mut sample = BTreeMap::new();
        sample.insert("id".to_string(), UniversalValue::Int(1));
        sample.insert("name".to_string(), UniversalValue::String("Alice".to_string()));
        sample.insert("active".to_string(), UniversalValue::Bool(true));

        let schema = SchemaRegistry::infer_schema("users", &UniversalValue::Map(sample));

        assert!(schema.is_some());
        let schema = schema.unwrap();
        assert_eq!(schema.fields.len(), 3);
    }
}
