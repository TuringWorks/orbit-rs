//! Schema management for Neo4j Cypher queries
//!
//! Implements schema commands for constraints and indexes

use crate::protocols::error::{ProtocolError, ProtocolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Type of constraint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConstraintType {
    /// Uniqueness constraint on node properties
    Unique,
    /// Node property existence constraint
    NodePropertyExistence,
    /// Relationship property existence constraint
    RelationshipPropertyExistence,
    /// Node key constraint (combination of uniqueness and existence)
    NodeKey,
}

/// Constraint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    /// Constraint name
    pub name: String,
    /// Type of constraint
    pub constraint_type: ConstraintType,
    /// Label (for node constraints)
    pub label: Option<String>,
    /// Relationship type (for relationship constraints)
    pub relationship_type: Option<String>,
    /// Properties involved in the constraint
    pub properties: Vec<String>,
}

impl Constraint {
    /// Create a new uniqueness constraint
    pub fn unique(name: String, label: String, properties: Vec<String>) -> Self {
        Self {
            name,
            constraint_type: ConstraintType::Unique,
            label: Some(label),
            relationship_type: None,
            properties,
        }
    }

    /// Create a new node property existence constraint
    pub fn node_property_exists(name: String, label: String, property: String) -> Self {
        Self {
            name,
            constraint_type: ConstraintType::NodePropertyExistence,
            label: Some(label),
            relationship_type: None,
            properties: vec![property],
        }
    }

    /// Create a new relationship property existence constraint
    pub fn relationship_property_exists(
        name: String,
        relationship_type: String,
        property: String,
    ) -> Self {
        Self {
            name,
            constraint_type: ConstraintType::RelationshipPropertyExistence,
            label: None,
            relationship_type: Some(relationship_type),
            properties: vec![property],
        }
    }

    /// Create a new node key constraint
    pub fn node_key(name: String, label: String, properties: Vec<String>) -> Self {
        Self {
            name,
            constraint_type: ConstraintType::NodeKey,
            label: Some(label),
            relationship_type: None,
            properties,
        }
    }
}

/// Type of index
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexType {
    /// Standard B-tree index
    BTree,
    /// Full-text search index
    Fulltext,
    /// Lookup index for label/relationship type lookups
    Lookup,
    /// Spatial index for point properties
    Point,
    /// Range index for efficient range queries
    Range,
    /// Text index for string properties
    Text,
}

/// Index definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    /// Index name
    pub name: String,
    /// Type of index
    pub index_type: IndexType,
    /// Label (for node indexes)
    pub label: Option<String>,
    /// Relationship type (for relationship indexes)
    pub relationship_type: Option<String>,
    /// Properties to index
    pub properties: Vec<String>,
    /// Index configuration options
    pub options: IndexOptions,
}

/// Index configuration options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexOptions {
    /// Analyzer for full-text indexes
    pub analyzer: Option<String>,
    /// Eventually consistent (for full-text)
    pub eventually_consistent: bool,
    /// Custom configuration
    pub config: std::collections::HashMap<String, String>,
}

impl Index {
    /// Create a new B-tree index
    pub fn btree(name: String, label: String, properties: Vec<String>) -> Self {
        Self {
            name,
            index_type: IndexType::BTree,
            label: Some(label),
            relationship_type: None,
            properties,
            options: IndexOptions::default(),
        }
    }

    /// Create a new full-text index
    pub fn fulltext(
        name: String,
        labels: Vec<String>,
        properties: Vec<String>,
        analyzer: Option<String>,
    ) -> Self {
        let label = labels.first().cloned();
        Self {
            name,
            index_type: IndexType::Fulltext,
            label,
            relationship_type: None,
            properties,
            options: IndexOptions {
                analyzer,
                eventually_consistent: true,
                config: std::collections::HashMap::new(),
            },
        }
    }

    /// Create a new lookup index
    pub fn lookup(name: String) -> Self {
        Self {
            name,
            index_type: IndexType::Lookup,
            label: None,
            relationship_type: None,
            properties: vec![],
            options: IndexOptions::default(),
        }
    }

    /// Create a new point (spatial) index
    pub fn point(name: String, label: String, property: String) -> Self {
        Self {
            name,
            index_type: IndexType::Point,
            label: Some(label),
            relationship_type: None,
            properties: vec![property],
            options: IndexOptions::default(),
        }
    }

    /// Create a new range index
    pub fn range(name: String, label: String, properties: Vec<String>) -> Self {
        Self {
            name,
            index_type: IndexType::Range,
            label: Some(label),
            relationship_type: None,
            properties,
            options: IndexOptions::default(),
        }
    }

    /// Create a new text index
    pub fn text(name: String, label: String, property: String) -> Self {
        Self {
            name,
            index_type: IndexType::Text,
            label: Some(label),
            relationship_type: None,
            properties: vec![property],
            options: IndexOptions::default(),
        }
    }
}

/// Schema manager for constraints and indexes
#[derive(Debug, Default)]
pub struct SchemaManager {
    /// Active constraints
    constraints: std::collections::HashMap<String, Constraint>,
    /// Active indexes
    indexes: std::collections::HashMap<String, Index>,
}

impl SchemaManager {
    /// Create a new schema manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a constraint
    pub fn create_constraint(&mut self, constraint: Constraint) -> ProtocolResult<()> {
        if self.constraints.contains_key(&constraint.name) {
            return Err(ProtocolError::CypherError(format!(
                "Constraint '{}' already exists",
                constraint.name
            )));
        }

        // Validate constraint
        self.validate_constraint(&constraint)?;

        self.constraints.insert(constraint.name.clone(), constraint);
        Ok(())
    }

    /// Drop a constraint
    pub fn drop_constraint(&mut self, name: &str) -> ProtocolResult<()> {
        if self.constraints.remove(name).is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Constraint '{}' does not exist",
                name
            )));
        }
        Ok(())
    }

    /// Get a constraint by name
    pub fn get_constraint(&self, name: &str) -> Option<&Constraint> {
        self.constraints.get(name)
    }

    /// List all constraints
    pub fn list_constraints(&self) -> Vec<&Constraint> {
        self.constraints.values().collect()
    }

    /// List constraints for a label
    pub fn list_constraints_for_label(&self, label: &str) -> Vec<&Constraint> {
        self.constraints
            .values()
            .filter(|c| c.label.as_deref() == Some(label))
            .collect()
    }

    /// Create an index
    pub fn create_index(&mut self, index: Index) -> ProtocolResult<()> {
        if self.indexes.contains_key(&index.name) {
            return Err(ProtocolError::CypherError(format!(
                "Index '{}' already exists",
                index.name
            )));
        }

        // Validate index
        self.validate_index(&index)?;

        self.indexes.insert(index.name.clone(), index);
        Ok(())
    }

    /// Drop an index
    pub fn drop_index(&mut self, name: &str) -> ProtocolResult<()> {
        if self.indexes.remove(name).is_none() {
            return Err(ProtocolError::CypherError(format!(
                "Index '{}' does not exist",
                name
            )));
        }
        Ok(())
    }

    /// Get an index by name
    pub fn get_index(&self, name: &str) -> Option<&Index> {
        self.indexes.get(name)
    }

    /// List all indexes
    pub fn list_indexes(&self) -> Vec<&Index> {
        self.indexes.values().collect()
    }

    /// List indexes for a label
    pub fn list_indexes_for_label(&self, label: &str) -> Vec<&Index> {
        self.indexes
            .values()
            .filter(|i| i.label.as_deref() == Some(label))
            .collect()
    }

    /// List indexes of a specific type
    pub fn list_indexes_by_type(&self, index_type: IndexType) -> Vec<&Index> {
        self.indexes
            .values()
            .filter(|i| i.index_type == index_type)
            .collect()
    }

    /// Validate a constraint
    fn validate_constraint(&self, constraint: &Constraint) -> ProtocolResult<()> {
        // Check that properties are not empty
        if constraint.properties.is_empty() {
            return Err(ProtocolError::CypherError(
                "Constraint must have at least one property".to_string(),
            ));
        }

        // Check that label or relationship type is specified
        match constraint.constraint_type {
            ConstraintType::Unique
            | ConstraintType::NodePropertyExistence
            | ConstraintType::NodeKey => {
                if constraint.label.is_none() {
                    return Err(ProtocolError::CypherError(
                        "Node constraint must have a label".to_string(),
                    ));
                }
            }
            ConstraintType::RelationshipPropertyExistence => {
                if constraint.relationship_type.is_none() {
                    return Err(ProtocolError::CypherError(
                        "Relationship constraint must have a type".to_string(),
                    ));
                }
            }
        }

        // Check for duplicate properties
        let unique_props: HashSet<_> = constraint.properties.iter().collect();
        if unique_props.len() != constraint.properties.len() {
            return Err(ProtocolError::CypherError(
                "Constraint has duplicate properties".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate an index
    fn validate_index(&self, index: &Index) -> ProtocolResult<()> {
        // Lookup indexes don't need properties
        if index.index_type != IndexType::Lookup && index.properties.is_empty() {
            return Err(ProtocolError::CypherError(
                "Index must have at least one property".to_string(),
            ));
        }

        // Check that label or relationship type is specified (except for lookup)
        if index.index_type != IndexType::Lookup
            && index.label.is_none()
            && index.relationship_type.is_none()
        {
            return Err(ProtocolError::CypherError(
                "Index must have a label or relationship type".to_string(),
            ));
        }

        // Check for duplicate properties
        let unique_props: HashSet<_> = index.properties.iter().collect();
        if unique_props.len() != index.properties.len() {
            return Err(ProtocolError::CypherError(
                "Index has duplicate properties".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if a constraint would be violated by a property set
    pub fn check_constraints(
        &self,
        label: &str,
        properties: &std::collections::HashMap<String, serde_json::Value>,
    ) -> ProtocolResult<()> {
        for constraint in self.list_constraints_for_label(label) {
            match constraint.constraint_type {
                ConstraintType::NodePropertyExistence | ConstraintType::NodeKey => {
                    // Check that all required properties exist
                    for prop in &constraint.properties {
                        if !properties.contains_key(prop) {
                            return Err(ProtocolError::CypherError(format!(
                                "Property '{}' is required by constraint '{}'",
                                prop, constraint.name
                            )));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_unique_constraint() {
        let mut manager = SchemaManager::new();
        let constraint = Constraint::unique(
            "person_email_unique".to_string(),
            "Person".to_string(),
            vec!["email".to_string()],
        );

        assert!(manager.create_constraint(constraint).is_ok());
        assert_eq!(manager.list_constraints().len(), 1);
    }

    #[test]
    fn test_duplicate_constraint() {
        let mut manager = SchemaManager::new();
        let constraint1 = Constraint::unique(
            "person_email_unique".to_string(),
            "Person".to_string(),
            vec!["email".to_string()],
        );
        let constraint2 = constraint1.clone();

        assert!(manager.create_constraint(constraint1).is_ok());
        assert!(manager.create_constraint(constraint2).is_err());
    }

    #[test]
    fn test_drop_constraint() {
        let mut manager = SchemaManager::new();
        let constraint = Constraint::unique(
            "person_email_unique".to_string(),
            "Person".to_string(),
            vec!["email".to_string()],
        );

        manager.create_constraint(constraint).unwrap();
        assert!(manager.drop_constraint("person_email_unique").is_ok());
        assert_eq!(manager.list_constraints().len(), 0);
    }

    #[test]
    fn test_create_btree_index() {
        let mut manager = SchemaManager::new();
        let index = Index::btree(
            "person_name_idx".to_string(),
            "Person".to_string(),
            vec!["name".to_string()],
        );

        assert!(manager.create_index(index).is_ok());
        assert_eq!(manager.list_indexes().len(), 1);
    }

    #[test]
    fn test_create_fulltext_index() {
        let mut manager = SchemaManager::new();
        let index = Index::fulltext(
            "article_content_fulltext".to_string(),
            vec!["Article".to_string()],
            vec!["title".to_string(), "content".to_string()],
            Some("standard".to_string()),
        );

        assert!(manager.create_index(index).is_ok());
        let indexes = manager.list_indexes();
        assert_eq!(indexes.len(), 1);
        assert_eq!(indexes[0].index_type, IndexType::Fulltext);
    }

    #[test]
    fn test_create_point_index() {
        let mut manager = SchemaManager::new();
        let index = Index::point(
            "place_location_point".to_string(),
            "Place".to_string(),
            "location".to_string(),
        );

        assert!(manager.create_index(index).is_ok());
        let indexes = manager.list_indexes_by_type(IndexType::Point);
        assert_eq!(indexes.len(), 1);
    }

    #[test]
    fn test_list_indexes_for_label() {
        let mut manager = SchemaManager::new();
        
        manager.create_index(Index::btree(
            "person_name_idx".to_string(),
            "Person".to_string(),
            vec!["name".to_string()],
        )).unwrap();
        
        manager.create_index(Index::btree(
            "person_email_idx".to_string(),
            "Person".to_string(),
            vec!["email".to_string()],
        )).unwrap();
        
        manager.create_index(Index::btree(
            "company_name_idx".to_string(),
            "Company".to_string(),
            vec!["name".to_string()],
        )).unwrap();

        let person_indexes = manager.list_indexes_for_label("Person");
        assert_eq!(person_indexes.len(), 2);
    }

    #[test]
    fn test_constraint_validation() {
        let mut manager = SchemaManager::new();
        
        // Test empty properties
        let invalid = Constraint {
            name: "invalid".to_string(),
            constraint_type: ConstraintType::Unique,
            label: Some("Person".to_string()),
            relationship_type: None,
            properties: vec![],
        };
        assert!(manager.create_constraint(invalid).is_err());

        // Test missing label
        let invalid = Constraint {
            name: "invalid".to_string(),
            constraint_type: ConstraintType::Unique,
            label: None,
            relationship_type: None,
            properties: vec!["email".to_string()],
        };
        assert!(manager.create_constraint(invalid).is_err());
    }
}
