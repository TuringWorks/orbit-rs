//! Graph query engine for Cypher execution
//!
//! This module provides query execution capabilities for Cypher queries,
//! integrating with the graph storage layer and translating parsed queries
//! into graph operations.

use crate::protocols::cypher::cypher_parser::{
    CypherClause, CypherQuery, NodePattern, Pattern, PatternElement, RelationshipDirection,
    RelationshipPattern, ReturnItem,
};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_shared::graph::{Direction, GraphNode, GraphRelationship, GraphStorage};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

/// Graph query engine that executes Cypher queries
#[derive(Debug)]
pub struct GraphEngine<S: GraphStorage> {
    /// Graph storage backend
    storage: Arc<S>,
}

impl<S: GraphStorage> GraphEngine<S> {
    /// Create a new graph engine with the given storage backend
    pub fn new(storage: Arc<S>) -> Self {
        Self { storage }
    }

    /// Execute a Cypher query string
    #[instrument(skip(self, query), fields(query_length = query.len()))]
    pub async fn execute_query(&self, query: &str) -> ProtocolResult<QueryResult> {
        let parser = super::cypher_parser::CypherParser::new();
        let parsed_query = parser.parse(query)?;

        debug!(
            query = query,
            clauses_count = parsed_query.clauses.len(),
            "Executing Cypher query"
        );

        self.execute_parsed_query(parsed_query).await
    }

    /// Execute a pre-parsed Cypher query
    #[instrument(skip(self, query), fields(clauses_count = query.clauses.len()))]
    pub async fn execute_parsed_query(&self, query: CypherQuery) -> ProtocolResult<QueryResult> {
        let mut context = ExecutionContext::new();
        let mut result_nodes = Vec::new();
        let mut result_relationships = Vec::new();

        for clause in &query.clauses {
            match clause {
                CypherClause::Match { pattern } => {
                    let (nodes, rels) = self.execute_match_clause(pattern, &mut context).await?;
                    result_nodes.extend(nodes);
                    result_relationships.extend(rels);
                }
                CypherClause::Create { pattern } => {
                    let (nodes, rels) = self.execute_create_clause(pattern, &mut context).await?;
                    result_nodes.extend(nodes);
                    result_relationships.extend(rels);
                }
                CypherClause::Return { items } => {
                    return self
                        .execute_return_clause(items, &context, result_nodes, result_relationships)
                        .await;
                }
                CypherClause::Where {
                    condition: _condition,
                } => {
                    warn!("WHERE clause not yet fully implemented");
                    // TODO: Implement WHERE clause filtering
                }
            }
        }

        // If no explicit RETURN clause, return all matched/created data
        Ok(QueryResult {
            nodes: result_nodes,
            relationships: result_relationships,
            columns: vec!["nodes".to_string(), "relationships".to_string()],
        })
    }

    /// Execute a MATCH clause
    async fn execute_match_clause(
        &self,
        pattern: &Pattern,
        context: &mut ExecutionContext,
    ) -> ProtocolResult<(Vec<GraphNode>, Vec<GraphRelationship>)> {
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();

        for element in &pattern.elements {
            match element {
                PatternElement::Node(node_pattern) => {
                    let matched_nodes = self.match_node_pattern(node_pattern).await?;
                    if let Some(var) = &node_pattern.variable {
                        context.bind_nodes(var.clone(), matched_nodes.clone());
                    }
                    nodes.extend(matched_nodes);
                }
                PatternElement::Relationship(rel_pattern) => {
                    let matched_rels = self
                        .match_relationship_pattern(rel_pattern, context)
                        .await?;
                    if let Some(var) = &rel_pattern.variable {
                        context.bind_relationships(var.clone(), matched_rels.clone());
                    }
                    relationships.extend(matched_rels);
                }
            }
        }

        info!(
            nodes_count = nodes.len(),
            relationships_count = relationships.len(),
            "Executed MATCH clause"
        );
        Ok((nodes, relationships))
    }

    /// Execute a CREATE clause
    async fn execute_create_clause(
        &self,
        pattern: &Pattern,
        context: &mut ExecutionContext,
    ) -> ProtocolResult<(Vec<GraphNode>, Vec<GraphRelationship>)> {
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();

        for element in &pattern.elements {
            match element {
                PatternElement::Node(node_pattern) => {
                    let created_node = self.create_node_from_pattern(node_pattern).await?;
                    if let Some(var) = &node_pattern.variable {
                        context.bind_nodes(var.clone(), vec![created_node.clone()]);
                    }
                    nodes.push(created_node);
                }
                PatternElement::Relationship(rel_pattern) => {
                    // For CREATE relationships, we need start and end nodes from context
                    if let Some(created_rel) = self
                        .create_relationship_from_pattern(rel_pattern, context)
                        .await?
                    {
                        if let Some(var) = &rel_pattern.variable {
                            context.bind_relationships(var.clone(), vec![created_rel.clone()]);
                        }
                        relationships.push(created_rel);
                    }
                }
            }
        }

        info!(
            nodes_count = nodes.len(),
            relationships_count = relationships.len(),
            "Executed CREATE clause"
        );
        Ok((nodes, relationships))
    }

    /// Execute a RETURN clause
    async fn execute_return_clause(
        &self,
        items: &[ReturnItem],
        context: &ExecutionContext,
        nodes: Vec<GraphNode>,
        relationships: Vec<GraphRelationship>,
    ) -> ProtocolResult<QueryResult> {
        let mut result_nodes = Vec::new();
        let mut result_relationships = Vec::new();
        let mut columns = Vec::new();

        for item in items {
            columns.push(item.expression.clone());

            // Check if it's a bound variable
            if let Some(bound_nodes) = context.get_nodes(&item.expression) {
                result_nodes.extend(bound_nodes.clone());
            } else if let Some(bound_rels) = context.get_relationships(&item.expression) {
                result_relationships.extend(bound_rels.clone());
            } else {
                // Fallback to returning all nodes/relationships
                result_nodes.extend(nodes.clone());
                result_relationships.extend(relationships.clone());
            }
        }

        info!(
            returned_nodes = result_nodes.len(),
            returned_relationships = result_relationships.len(),
            "Executed RETURN clause"
        );

        Ok(QueryResult {
            nodes: result_nodes,
            relationships: result_relationships,
            columns,
        })
    }

    /// Match nodes based on pattern
    async fn match_node_pattern(&self, pattern: &NodePattern) -> ProtocolResult<Vec<GraphNode>> {
        if pattern.labels.is_empty() {
            // No labels specified - this would be expensive for large graphs
            warn!("Pattern matching without labels is not optimized");
            return Ok(Vec::new());
        }

        let mut matched_nodes = Vec::new();

        // For each label, find matching nodes
        for label in &pattern.labels {
            let property_filters = if pattern.properties.is_empty() {
                None
            } else {
                Some(pattern.properties.clone())
            };

            let nodes = self
                .storage
                .find_nodes_by_label(label, property_filters, None)
                .await
                .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

            matched_nodes.extend(nodes);
        }

        debug!(pattern = ?pattern, found_count = matched_nodes.len(), "Matched node pattern");
        Ok(matched_nodes)
    }

    /// Match relationships based on pattern  
    async fn match_relationship_pattern(
        &self,
        pattern: &RelationshipPattern,
        context: &ExecutionContext,
    ) -> ProtocolResult<Vec<GraphRelationship>> {
        // For relationship matching, we need nodes from context
        // This is a simplified implementation
        let mut matched_relationships = Vec::new();

        // Get all nodes from context to search their relationships
        for nodes in context.bound_nodes.values() {
            for node in nodes {
                let direction = match pattern.direction {
                    RelationshipDirection::Outgoing => Direction::Outgoing,
                    RelationshipDirection::Incoming => Direction::Incoming,
                    RelationshipDirection::Both => Direction::Both,
                };

                let rel_types = pattern.rel_type.as_ref().map(|t| vec![t.clone()]);

                let relationships = self
                    .storage
                    .get_relationships(&node.id, direction, rel_types)
                    .await
                    .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

                matched_relationships.extend(relationships);
            }
        }

        debug!(pattern = ?pattern, found_count = matched_relationships.len(), "Matched relationship pattern");
        Ok(matched_relationships)
    }

    /// Create a node from pattern
    async fn create_node_from_pattern(&self, pattern: &NodePattern) -> ProtocolResult<GraphNode> {
        let node = self
            .storage
            .create_node(pattern.labels.clone(), pattern.properties.clone())
            .await
            .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

        info!(node_id = %node.id, labels = ?pattern.labels, "Created node from pattern");
        Ok(node)
    }

    /// Create a relationship from pattern
    async fn create_relationship_from_pattern(
        &self,
        pattern: &RelationshipPattern,
        context: &ExecutionContext,
    ) -> ProtocolResult<Option<GraphRelationship>> {
        // For relationship creation, we need start and end nodes
        // This is a simplified implementation that takes the first available nodes
        let all_nodes: Vec<_> = context.bound_nodes.values().flatten().collect();

        if all_nodes.len() < 2 {
            warn!("Insufficient nodes in context for relationship creation");
            return Ok(None);
        }

        let start_node = &all_nodes[0].id;
        let end_node = &all_nodes[1].id;
        let rel_type = pattern.rel_type.as_deref().unwrap_or("RELATED");

        let relationship = self
            .storage
            .create_relationship(
                start_node,
                end_node,
                rel_type.to_string(),
                pattern.properties.clone(),
            )
            .await
            .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

        info!(
            rel_id = %relationship.id,
            start = %start_node,
            end = %end_node,
            rel_type = rel_type,
            "Created relationship from pattern"
        );

        Ok(Some(relationship))
    }
}

/// Query execution context for variable bindings
#[derive(Debug, Default)]
struct ExecutionContext {
    /// Bound node variables
    bound_nodes: HashMap<String, Vec<GraphNode>>,
    /// Bound relationship variables
    bound_relationships: HashMap<String, Vec<GraphRelationship>>,
}

impl ExecutionContext {
    fn new() -> Self {
        Self::default()
    }

    fn bind_nodes(&mut self, variable: String, nodes: Vec<GraphNode>) {
        self.bound_nodes.insert(variable, nodes);
    }

    fn bind_relationships(&mut self, variable: String, relationships: Vec<GraphRelationship>) {
        self.bound_relationships.insert(variable, relationships);
    }

    fn get_nodes(&self, variable: &str) -> Option<&Vec<GraphNode>> {
        self.bound_nodes.get(variable)
    }

    fn get_relationships(&self, variable: &str) -> Option<&Vec<GraphRelationship>> {
        self.bound_relationships.get(variable)
    }
}

/// Query execution result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryResult {
    /// Result nodes
    pub nodes: Vec<GraphNode>,
    /// Result relationships
    pub relationships: Vec<GraphRelationship>,
    /// Column names for the result
    pub columns: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::graph::InMemoryGraphStorage;
    use std::sync::Arc;

    async fn create_test_engine() -> GraphEngine<InMemoryGraphStorage> {
        let storage = Arc::new(InMemoryGraphStorage::new());
        GraphEngine::new(storage)
    }

    #[tokio::test]
    async fn test_create_node_query() {
        let engine = create_test_engine().await;
        let query = "CREATE (n:Person {name: 'Alice'}) RETURN n";

        let result = engine.execute_query(query).await;
        assert!(result.is_ok());

        let query_result = result.unwrap();
        assert_eq!(query_result.nodes.len(), 1);
        assert!(query_result.nodes[0].has_label("Person"));
    }

    #[tokio::test]
    async fn test_match_node_query() {
        let engine = create_test_engine().await;

        // First create a node
        let create_result = engine
            .execute_query("CREATE (n:Person {name: 'Alice'})")
            .await;
        assert!(create_result.is_ok());

        // Then match it
        let match_result = engine.execute_query("MATCH (n:Person) RETURN n").await;
        assert!(match_result.is_ok());

        let query_result = match_result.unwrap();
        assert_eq!(query_result.nodes.len(), 1);
    }

    #[tokio::test]
    async fn test_invalid_query() {
        let engine = create_test_engine().await;
        let result = engine.execute_query("INVALID SYNTAX").await;
        assert!(result.is_err());
    }
}
