//! Graph query engine for Cypher execution
//!
//! This module provides query execution capabilities for Cypher queries,
//! integrating with the graph storage layer and translating parsed queries
//! into graph operations.

use crate::protocols::cypher::cypher_parser::{
    CypherClause, CypherQuery, NodePattern, OrderByItem, Pattern, PatternElement, PropertyAssignment,
    RelationshipDirection, RelationshipPattern, RemoveItem, ReturnItem,
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
        let mut skip_count: Option<usize> = None;
        let mut limit_count: Option<usize> = None;
        let mut order_by: Option<Vec<OrderByItem>> = None;
        let mut return_items: Option<Vec<ReturnItem>> = None;

        // First pass: collect all clauses and extract modifiers (ORDER BY, SKIP, LIMIT)
        for clause in &query.clauses {
            match clause {
                CypherClause::OrderBy { items } => {
                    order_by = Some(items.clone());
                }
                CypherClause::Limit { count } => {
                    limit_count = Some(*count);
                }
                CypherClause::Skip { count } => {
                    skip_count = Some(*count);
                }
                _ => {}
            }
        }

        // Second pass: execute clauses in order
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
                    // Store RETURN items but don't execute yet
                    return_items = Some(items.clone());
                }
                CypherClause::Where { condition } => {
                    // Apply WHERE clause filtering to current results
                    result_nodes = self
                        .apply_where_filter_nodes(condition, &result_nodes, &context)
                        .await?;
                    result_relationships = self
                        .apply_where_filter_relationships(condition, &result_relationships, &context)
                        .await?;
                }
                CypherClause::Delete { variables, detach } => {
                    self.execute_delete_clause(variables, *detach, &mut context, &mut result_nodes, &mut result_relationships).await?;
                }
                CypherClause::Set { assignments } => {
                    self.execute_set_clause(assignments, &mut context, &mut result_nodes).await?;
                }
                CypherClause::Merge { pattern } => {
                    let (nodes, rels) = self.execute_merge_clause(pattern, &mut context).await?;
                    result_nodes.extend(nodes);
                    result_relationships.extend(rels);
                }
                CypherClause::Remove { items } => {
                    self.execute_remove_clause(items, &mut context, &mut result_nodes).await?;
                }
                // ORDER BY, LIMIT, SKIP already collected in first pass
                CypherClause::OrderBy { .. } | CypherClause::Limit { .. } | CypherClause::Skip { .. } => {}
            }
        }

        // Apply ORDER BY before projecting results
        if let Some(ref order_items) = order_by {
            self.apply_order_by(&mut result_nodes, order_items);
        }

        // Apply SKIP
        if let Some(skip) = skip_count {
            result_nodes = result_nodes.into_iter().skip(skip).collect();
            result_relationships = result_relationships.into_iter().skip(skip).collect();
        }

        // Apply LIMIT
        if let Some(limit) = limit_count {
            result_nodes.truncate(limit);
            result_relationships.truncate(limit);
        }

        // Execute RETURN clause if present
        if let Some(items) = return_items {
            return self
                .execute_return_clause(&items, &context, result_nodes, result_relationships)
                .await;
        }

        // If no explicit RETURN clause, return all matched/created data
        Ok(QueryResult {
            nodes: result_nodes,
            relationships: result_relationships,
            columns: vec!["nodes".to_string(), "relationships".to_string()],
            rows: Vec::new(),
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

            // Check if the variable matches nodes already in our result set
            // This is important for LIMIT/SKIP/ORDER BY which modify the result set
            let nodes_in_result: Vec<_> = nodes.iter()
                .filter(|n| {
                    // Check if any node matches this variable binding
                    context.get_nodes(&item.expression)
                        .map(|bound| bound.iter().any(|b| b.id == n.id))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();

            if !nodes_in_result.is_empty() {
                // Use the nodes from result set (preserves LIMIT/SKIP/ORDER BY)
                result_nodes.extend(nodes_in_result);
            } else if let Some(bound_nodes) = context.get_nodes(&item.expression) {
                // Fall back to context if no matching nodes in result
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
            rows: Vec::new(),
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

    /// Apply WHERE clause filtering to nodes
    async fn apply_where_filter_nodes(
        &self,
        condition: &crate::protocols::cypher::cypher_parser::Condition,
        nodes: &[GraphNode],
        context: &ExecutionContext,
    ) -> ProtocolResult<Vec<GraphNode>> {
        let mut filtered = Vec::new();

        for node in nodes {
            if self.evaluate_condition_for_node(condition, node, context).await? {
                filtered.push(node.clone());
            }
        }

        Ok(filtered)
    }

    /// Apply WHERE clause filtering to relationships
    async fn apply_where_filter_relationships(
        &self,
        condition: &crate::protocols::cypher::cypher_parser::Condition,
        relationships: &[GraphRelationship],
        context: &ExecutionContext,
    ) -> ProtocolResult<Vec<GraphRelationship>> {
        let mut filtered = Vec::new();

        for rel in relationships {
            if self
                .evaluate_condition_for_relationship(condition, rel, context)
                .await?
            {
                filtered.push(rel.clone());
            }
        }

        Ok(filtered)
    }

    /// Evaluate a condition for a node
    async fn evaluate_condition_for_node(
        &self,
        condition: &crate::protocols::cypher::cypher_parser::Condition,
        node: &GraphNode,
        _context: &ExecutionContext,
    ) -> ProtocolResult<bool> {
        use crate::protocols::cypher::cypher_parser::{ComparisonOperator, Condition};

        match condition {
            Condition::PropertyEquals { property, value } => {
                Ok(node.properties.get(property) == Some(value))
            }
            Condition::PropertyComparison {
                property,
                operator,
                value,
            } => {
                if let Some(prop_value) = node.properties.get(property) {
                    Ok(match operator {
                        ComparisonOperator::Equals => prop_value == value,
                        ComparisonOperator::NotEquals => prop_value != value,
                        ComparisonOperator::GreaterThan => {
                            self.compare_values(prop_value, value) > 0
                        }
                        ComparisonOperator::LessThan => {
                            self.compare_values(prop_value, value) < 0
                        }
                        ComparisonOperator::GreaterThanOrEqual => {
                            self.compare_values(prop_value, value) >= 0
                        }
                        ComparisonOperator::LessThanOrEqual => {
                            self.compare_values(prop_value, value) <= 0
                        }
                    })
                } else {
                    Ok(false)
                }
            }
            Condition::PropertyExists { property } => Ok(node.properties.contains_key(property)),
            Condition::And { left, right } => {
                let left_result = Box::pin(self.evaluate_condition_for_node(left, node, _context)).await?;
                let right_result = Box::pin(self.evaluate_condition_for_node(right, node, _context)).await?;
                Ok(left_result && right_result)
            }
            Condition::Or { left, right } => {
                let left_result = Box::pin(self.evaluate_condition_for_node(left, node, _context)).await?;
                let right_result = Box::pin(self.evaluate_condition_for_node(right, node, _context)).await?;
                Ok(left_result || right_result)
            }
            Condition::Not { condition } => {
                let result = Box::pin(self.evaluate_condition_for_node(condition, node, _context)).await?;
                Ok(!result)
            }
            Condition::HasLabel { variable: _, label } => Ok(node.labels.contains(label)),
            Condition::HasRelationshipType { .. } => {
                // Not applicable to nodes
                Ok(false)
            }
        }
    }

    /// Evaluate a condition for a relationship
    async fn evaluate_condition_for_relationship(
        &self,
        condition: &crate::protocols::cypher::cypher_parser::Condition,
        rel: &GraphRelationship,
        _context: &ExecutionContext,
    ) -> ProtocolResult<bool> {
        use crate::protocols::cypher::cypher_parser::{ComparisonOperator, Condition};

        match condition {
            Condition::PropertyEquals { property, value } => {
                Ok(rel.properties.get(property) == Some(value))
            }
            Condition::PropertyComparison {
                property,
                operator,
                value,
            } => {
                if let Some(prop_value) = rel.properties.get(property) {
                    Ok(match operator {
                        ComparisonOperator::Equals => prop_value == value,
                        ComparisonOperator::NotEquals => prop_value != value,
                        ComparisonOperator::GreaterThan => {
                            self.compare_values(prop_value, value) > 0
                        }
                        ComparisonOperator::LessThan => {
                            self.compare_values(prop_value, value) < 0
                        }
                        ComparisonOperator::GreaterThanOrEqual => {
                            self.compare_values(prop_value, value) >= 0
                        }
                        ComparisonOperator::LessThanOrEqual => {
                            self.compare_values(prop_value, value) <= 0
                        }
                    })
                } else {
                    Ok(false)
                }
            }
            Condition::PropertyExists { property } => Ok(rel.properties.contains_key(property)),
            Condition::And { left, right } => {
                let left_result = Box::pin(self.evaluate_condition_for_relationship(left, rel, _context)).await?;
                let right_result = Box::pin(self.evaluate_condition_for_relationship(right, rel, _context)).await?;
                Ok(left_result && right_result)
            }
            Condition::Or { left, right } => {
                let left_result = Box::pin(self.evaluate_condition_for_relationship(left, rel, _context)).await?;
                let right_result = Box::pin(self.evaluate_condition_for_relationship(right, rel, _context)).await?;
                Ok(left_result || right_result)
            }
            Condition::Not { condition } => {
                let result = Box::pin(self.evaluate_condition_for_relationship(condition, rel, _context)).await?;
                Ok(!result)
            }
            Condition::HasLabel { .. } => {
                // Not applicable to relationships
                Ok(false)
            }
            Condition::HasRelationshipType { variable: _, rel_type } => {
                Ok(rel.rel_type == *rel_type)
            }
        }
    }

    /// Compare two JSON values for ordering
    fn compare_values(&self, a: &serde_json::Value, b: &serde_json::Value) -> i32 {
        match (a, b) {
            (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
                if let (Some(fa), Some(fb)) = (na.as_f64(), nb.as_f64()) {
                    (fa - fb).signum() as i32
                } else {
                    0
                }
            }
            (serde_json::Value::String(sa), serde_json::Value::String(sb)) => {
                sa.cmp(sb) as i32
            }
            _ => 0,
        }
    }

    /// Execute a DELETE clause
    async fn execute_delete_clause(
        &self,
        variables: &[String],
        detach: bool,
        context: &mut ExecutionContext,
        result_nodes: &mut Vec<GraphNode>,
        result_relationships: &mut Vec<GraphRelationship>,
    ) -> ProtocolResult<()> {
        let mut deleted_node_ids = Vec::new();
        let mut deleted_rel_ids = Vec::new();

        for var in variables {
            // Check if variable is bound to nodes
            if let Some(nodes) = context.get_nodes(var) {
                for node in nodes.clone() {
                    if detach {
                        // DETACH DELETE: First delete all relationships connected to this node
                        let outgoing_rels = self
                            .storage
                            .get_relationships(&node.id, Direction::Outgoing, None)
                            .await
                            .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

                        let incoming_rels = self
                            .storage
                            .get_relationships(&node.id, Direction::Incoming, None)
                            .await
                            .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

                        for rel in outgoing_rels.iter().chain(incoming_rels.iter()) {
                            self.storage
                                .delete_relationship(&rel.id)
                                .await
                                .map_err(|e| ProtocolError::ActorError(e.to_string()))?;
                            deleted_rel_ids.push(rel.id.clone());
                        }
                    }

                    // Delete the node
                    self.storage
                        .delete_node(&node.id)
                        .await
                        .map_err(|e| ProtocolError::ActorError(e.to_string()))?;
                    deleted_node_ids.push(node.id.clone());
                }
            }

            // Check if variable is bound to relationships
            if let Some(rels) = context.get_relationships(var) {
                for rel in rels.clone() {
                    self.storage
                        .delete_relationship(&rel.id)
                        .await
                        .map_err(|e| ProtocolError::ActorError(e.to_string()))?;
                    deleted_rel_ids.push(rel.id.clone());
                }
            }
        }

        // Remove deleted items from result sets
        result_nodes.retain(|n| !deleted_node_ids.contains(&n.id));
        result_relationships.retain(|r| !deleted_rel_ids.contains(&r.id));

        info!(
            deleted_nodes = deleted_node_ids.len(),
            deleted_relationships = deleted_rel_ids.len(),
            detach = detach,
            "Executed DELETE clause"
        );

        Ok(())
    }

    /// Execute a SET clause
    async fn execute_set_clause(
        &self,
        assignments: &[PropertyAssignment],
        context: &mut ExecutionContext,
        result_nodes: &mut Vec<GraphNode>,
    ) -> ProtocolResult<()> {
        for assignment in assignments {
            // Parse target: variable.property
            let parts: Vec<&str> = assignment.target.split('.').collect();
            if parts.len() != 2 {
                return Err(ProtocolError::CypherError(format!(
                    "Invalid SET target: {}",
                    assignment.target
                )));
            }

            let var = parts[0];
            let prop = parts[1];

            // Check if variable is bound to nodes
            if let Some(nodes) = context.get_nodes(var) {
                let mut updated_nodes = Vec::new();
                for node in nodes.clone() {
                    // Update node property
                    let mut new_properties = node.properties.clone();
                    new_properties.insert(prop.to_string(), assignment.value.clone());

                    self.storage
                        .update_node(&node.id, new_properties.clone())
                        .await
                        .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

                    // Create updated node for context
                    let mut updated_node = node.clone();
                    updated_node.properties = new_properties;
                    updated_nodes.push(updated_node.clone());

                    // Update in result_nodes
                    if let Some(result_node) = result_nodes.iter_mut().find(|n| n.id == node.id) {
                        result_node.properties.insert(prop.to_string(), assignment.value.clone());
                    }
                }
                // Update context with modified nodes
                context.bind_nodes(var.to_string(), updated_nodes);
            }
        }

        info!(
            assignments_count = assignments.len(),
            "Executed SET clause"
        );

        Ok(())
    }

    /// Execute a MERGE clause (create-or-match)
    async fn execute_merge_clause(
        &self,
        pattern: &Pattern,
        context: &mut ExecutionContext,
    ) -> ProtocolResult<(Vec<GraphNode>, Vec<GraphRelationship>)> {
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();

        for element in &pattern.elements {
            match element {
                PatternElement::Node(node_pattern) => {
                    // Try to match first
                    let matched_nodes = self.match_node_pattern(node_pattern).await?;

                    if matched_nodes.is_empty() {
                        // No match found, create the node
                        let created_node = self.create_node_from_pattern(node_pattern).await?;
                        if let Some(var) = &node_pattern.variable {
                            context.bind_nodes(var.clone(), vec![created_node.clone()]);
                        }
                        nodes.push(created_node);
                    } else {
                        // Found match, use existing nodes
                        if let Some(var) = &node_pattern.variable {
                            context.bind_nodes(var.clone(), matched_nodes.clone());
                        }
                        nodes.extend(matched_nodes);
                    }
                }
                PatternElement::Relationship(rel_pattern) => {
                    // For relationships, we'll create if we have the required nodes in context
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
            "Executed MERGE clause"
        );
        Ok((nodes, relationships))
    }

    /// Execute a REMOVE clause
    async fn execute_remove_clause(
        &self,
        items: &[RemoveItem],
        context: &mut ExecutionContext,
        result_nodes: &mut Vec<GraphNode>,
    ) -> ProtocolResult<()> {
        for item in items {
            match item {
                RemoveItem::Property { variable, property } => {
                    // Remove property from nodes
                    if let Some(nodes) = context.get_nodes(variable) {
                        for node in nodes.clone() {
                            let mut new_properties = node.properties.clone();
                            new_properties.remove(property);

                            self.storage
                                .update_node(&node.id, new_properties.clone())
                                .await
                                .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

                            // Update in result_nodes
                            if let Some(result_node) = result_nodes.iter_mut().find(|n| n.id == node.id) {
                                result_node.properties.remove(property);
                            }
                        }
                    }
                }
                RemoveItem::Label { variable, label } => {
                    // Remove label from nodes
                    if let Some(nodes) = context.get_nodes(variable) {
                        for node in nodes.clone() {
                            self.storage
                                .remove_labels(&node.id, vec![label.clone()])
                                .await
                                .map_err(|e| ProtocolError::ActorError(e.to_string()))?;

                            // Update in result_nodes
                            if let Some(result_node) = result_nodes.iter_mut().find(|n| n.id == node.id) {
                                result_node.labels.retain(|l| l != label);
                            }
                        }
                    }
                }
            }
        }

        info!(
            items_count = items.len(),
            "Executed REMOVE clause"
        );

        Ok(())
    }

    /// Apply ORDER BY to nodes
    fn apply_order_by(&self, nodes: &mut [GraphNode], order_items: &[OrderByItem]) {
        nodes.sort_by(|a, b| {
            for item in order_items {
                // Parse expression: might be "property" or "var.property"
                let prop_name = item.expression.split('.').last().unwrap_or(&item.expression);

                let val_a = a.properties.get(prop_name);
                let val_b = b.properties.get(prop_name);

                let cmp = match (val_a, val_b) {
                    (Some(va), Some(vb)) => self.compare_values(va, vb),
                    (Some(_), None) => 1,
                    (None, Some(_)) => -1,
                    (None, None) => 0,
                };

                if cmp != 0 {
                    let ordering = match cmp.cmp(&0) {
                        std::cmp::Ordering::Greater => std::cmp::Ordering::Greater,
                        std::cmp::Ordering::Less => std::cmp::Ordering::Less,
                        std::cmp::Ordering::Equal => std::cmp::Ordering::Equal,
                    };
                    return if item.descending {
                        ordering.reverse()
                    } else {
                        ordering
                    };
                }
            }
            std::cmp::Ordering::Equal
        });
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
    /// Tabular rows (for procedure results)
    pub rows: Vec<Vec<Option<String>>>,
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

    #[tokio::test]
    async fn test_delete_node() {
        let engine = create_test_engine().await;

        // Create a node
        let create_result = engine
            .execute_query("CREATE (n:Person {name: 'ToDelete'})")
            .await;
        assert!(create_result.is_ok());

        // Verify the node exists
        let match_result = engine.execute_query("MATCH (n:Person) RETURN n").await;
        assert!(match_result.is_ok());
        assert_eq!(match_result.unwrap().nodes.len(), 1);

        // Delete the node
        let delete_result = engine.execute_query("MATCH (n:Person) DELETE n").await;
        assert!(delete_result.is_ok());

        // Verify the node is deleted
        let verify_result = engine.execute_query("MATCH (n:Person) RETURN n").await;
        assert!(verify_result.is_ok());
        assert_eq!(verify_result.unwrap().nodes.len(), 0);
    }

    #[tokio::test]
    async fn test_set_property() {
        let engine = create_test_engine().await;

        // Create a node
        let create_result = engine
            .execute_query("CREATE (n:Person {name: 'Alice'})")
            .await;
        assert!(create_result.is_ok());

        // Update the name property
        let set_result = engine
            .execute_query("MATCH (n:Person) SET n.name = 'Bob' RETURN n")
            .await;
        assert!(set_result.is_ok());

        let result = set_result.unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(
            result.nodes[0].properties.get("name"),
            Some(&serde_json::Value::String("Bob".to_string()))
        );
    }

    #[tokio::test]
    async fn test_merge_creates_when_not_exists() {
        let engine = create_test_engine().await;

        // Merge should create since no matching node exists
        let merge_result = engine
            .execute_query("MERGE (n:Person {name: 'Charlie'}) RETURN n")
            .await;
        assert!(merge_result.is_ok());

        let result = merge_result.unwrap();
        assert_eq!(result.nodes.len(), 1);
        assert!(result.nodes[0].has_label("Person"));
    }

    #[tokio::test]
    async fn test_limit_results() {
        let engine = create_test_engine().await;

        // Create multiple nodes
        for i in 0..5 {
            let query = format!("CREATE (n:Person {{name: 'Person{}'}})", i);
            engine.execute_query(&query).await.unwrap();
        }

        // Query with limit
        let limited_result = engine
            .execute_query("MATCH (n:Person) RETURN n LIMIT 3")
            .await;
        assert!(limited_result.is_ok());

        let result = limited_result.unwrap();
        assert_eq!(result.nodes.len(), 3);
    }

    #[tokio::test]
    async fn test_skip_results() {
        let engine = create_test_engine().await;

        // Create multiple nodes
        for i in 0..5 {
            let query = format!("CREATE (n:Person {{name: 'Person{}'}})", i);
            engine.execute_query(&query).await.unwrap();
        }

        // Query with skip
        let skip_result = engine
            .execute_query("MATCH (n:Person) RETURN n SKIP 2")
            .await;
        assert!(skip_result.is_ok());

        let result = skip_result.unwrap();
        assert_eq!(result.nodes.len(), 3); // 5 - 2 = 3
    }

    #[tokio::test]
    async fn test_skip_and_limit_together() {
        let engine = create_test_engine().await;

        // Create multiple nodes
        for i in 0..10 {
            let query = format!("CREATE (n:Person {{name: 'Person{}'}})", i);
            engine.execute_query(&query).await.unwrap();
        }

        // Query with skip and limit (pagination)
        let paginated_result = engine
            .execute_query("MATCH (n:Person) RETURN n SKIP 3 LIMIT 2")
            .await;
        assert!(paginated_result.is_ok());

        let result = paginated_result.unwrap();
        assert_eq!(result.nodes.len(), 2);
    }

    #[tokio::test]
    async fn test_order_by_sorting() {
        let engine = create_test_engine().await;

        // Create nodes with different values
        engine.execute_query("CREATE (n:Person {name: 'Charlie', age: 30})").await.unwrap();
        engine.execute_query("CREATE (n:Person {name: 'Alice', age: 25})").await.unwrap();
        engine.execute_query("CREATE (n:Person {name: 'Bob', age: 35})").await.unwrap();

        // Query with ORDER BY name ASC
        let asc_result = engine
            .execute_query("MATCH (n:Person) RETURN n ORDER BY n.name")
            .await;
        assert!(asc_result.is_ok());

        let result = asc_result.unwrap();
        assert_eq!(result.nodes.len(), 3);
        // First node should be Alice (alphabetically first)
        assert_eq!(
            result.nodes[0].properties.get("name"),
            Some(&serde_json::Value::String("Alice".to_string()))
        );
    }
}
