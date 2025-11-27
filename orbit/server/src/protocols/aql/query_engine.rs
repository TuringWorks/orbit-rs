//! AQL query engine with GraphRAG integration
//!
//! This module provides a complete AQL query engine that includes GraphRAG function support.

use crate::protocols::aql::aql_parser::{
    AqlClause, AqlCondition, AqlExpression, ComparisonOperator,
};
use crate::protocols::aql::{
    AqlDocument, AqlGraphRAGEngine, AqlParser, AqlQuery, AqlStorage, AqlValue,
};
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_client::OrbitClient;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

/// AQL query execution result
#[derive(Debug, Clone)]
pub struct AqlQueryResult {
    /// Result data as AQL values
    pub data: Vec<AqlValue>,
    /// Query execution metadata
    pub metadata: HashMap<String, AqlValue>,
}

/// AQL query engine with GraphRAG function support
pub struct AqlQueryEngine {
    /// AQL parser for standard queries
    parser: AqlParser,
    /// GraphRAG function engine
    graphrag_engine: Option<AqlGraphRAGEngine>,
    /// Storage backend for document operations
    storage: Option<Arc<AqlStorage>>,
}

impl AqlQueryEngine {
    /// Create new AQL query engine
    pub fn new() -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: None,
            storage: None,
        }
    }

    /// Create new AQL query engine with storage
    pub fn with_storage(storage: Arc<AqlStorage>) -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: None,
            storage: Some(storage),
        }
    }

    /// Create new AQL query engine with GraphRAG support
    pub fn new_with_graphrag(orbit_client: OrbitClient) -> Self {
        Self {
            parser: AqlParser::new(),
            graphrag_engine: Some(AqlGraphRAGEngine::new(orbit_client)),
            storage: None,
        }
    }

    /// Execute an AQL query
    pub async fn execute_query(&self, aql: &str) -> ProtocolResult<AqlQueryResult> {
        // Check if this is a GraphRAG function call
        if self.is_graphrag_function_call(aql) {
            return self.execute_graphrag_function_call(aql).await;
        }

        // Parse and execute regular AQL query
        let parsed_query = self.parser.parse(aql)?;
        self.execute_parsed_query(parsed_query).await
    }

    /// Check if query contains GraphRAG function calls
    fn is_graphrag_function_call(&self, aql: &str) -> bool {
        let aql_upper = aql.to_uppercase();
        aql_upper.contains("GRAPHRAG_BUILD_KNOWLEDGE(")
            || aql_upper.contains("GRAPHRAG_EXTRACT_ENTITIES(")
            || aql_upper.contains("GRAPHRAG_QUERY(")
            || aql_upper.contains("GRAPHRAG_FIND_PATHS(")
            || aql_upper.contains("GRAPHRAG_FIND_SIMILAR(")
            || aql_upper.contains("GRAPHRAG_SEMANTIC_SEARCH(")
            || aql_upper.contains("GRAPHRAG_GET_STATS(")
            || aql_upper.contains("GRAPHRAG_LIST_ENTITIES(")
            || aql_upper.contains("GRAPHRAG_ANALYZE_TRENDS(")
            || aql_upper.contains("GRAPHRAG_DETECT_COMMUNITIES(")
    }

    /// Execute GraphRAG function call
    async fn execute_graphrag_function_call(&self, aql: &str) -> ProtocolResult<AqlQueryResult> {
        let graphrag_engine = self.graphrag_engine.as_ref().ok_or_else(|| {
            ProtocolError::AqlError(
                "GraphRAG functions require GraphRAG engine integration".to_string(),
            )
        })?;

        // Simple parsing to extract function name and arguments
        // In a real implementation, this would use the full AQL parser
        let (function_name, args) = self.parse_graphrag_function_call(aql)?;

        let result = graphrag_engine
            .execute_graphrag_function(&function_name, &args)
            .await?;

        // Wrap result in query result format
        let mut metadata = HashMap::new();
        metadata.insert(
            "function_called".to_string(),
            AqlValue::String(function_name),
        );
        metadata.insert(
            "execution_time".to_string(),
            AqlValue::String(chrono::Utc::now().to_rfc3339()),
        );

        Ok(AqlQueryResult {
            data: vec![result],
            metadata,
        })
    }

    /// Parse GraphRAG function call (simplified implementation)
    fn parse_graphrag_function_call(&self, aql: &str) -> ProtocolResult<(String, Vec<AqlValue>)> {
        // This is a simplified implementation
        // In practice, this would use a proper AQL parser to extract function calls

        let aql = aql.trim();
        if let Some(start) = aql.find('(') {
            let function_name = aql[..start]
                .trim()
                .strip_prefix("FOR result IN ")
                .unwrap_or(&aql[..start])
                .trim()
                .to_string();

            // For now, return empty args - a full implementation would parse the arguments
            Ok((function_name, vec![]))
        } else {
            Err(ProtocolError::AqlError(
                "Invalid function call syntax".to_string(),
            ))
        }
    }

    /// Execute parsed AQL query
    async fn execute_parsed_query(&self, query: AqlQuery) -> ProtocolResult<AqlQueryResult> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            ProtocolError::AqlError("Storage backend required for query execution".to_string())
        })?;

        // Execution context for variables
        let mut context: HashMap<String, AqlValue> = HashMap::new();
        let mut result_data = Vec::new();

        // Process clauses in order
        let mut for_variable: Option<String> = None;
        let mut for_documents: Vec<AqlDocument> = Vec::new();

        for clause in &query.clauses {
            match clause {
                AqlClause::For {
                    variable,
                    data_source,
                } => {
                    // Execute FOR clause - iterate over collection
                    for_documents = self.execute_for_clause(storage, data_source).await?;
                    for_variable = Some(variable.clone());
                }
                AqlClause::Filter { condition } => {
                    // Apply filter to documents from FOR clause
                    if let Some(ref var) = for_variable {
                        for_documents = for_documents
                            .into_iter()
                            .filter(|doc| {
                                let mut ctx = context.clone();
                                ctx.insert(var.clone(), self.document_to_value(doc));
                                self.evaluate_condition(condition, &ctx).unwrap_or(false)
                            })
                            .collect();
                    }
                }
                AqlClause::Return {
                    distinct,
                    expression,
                } => {
                    // Process RETURN clause
                    if let Some(ref var) = for_variable {
                        // Return documents from FOR clause
                        for doc in &for_documents {
                            context.insert(var.clone(), self.document_to_value(doc));
                            let value = self.evaluate_expression(expression, &context)?;
                            result_data.push(value);
                        }
                    } else {
                        // No FOR clause - evaluate expression directly
                        let value = self.evaluate_expression(expression, &context)?;
                        result_data.push(value);
                    }

                    if *distinct {
                        result_data = self.deduplicate_results(result_data);
                    }
                    break; // RETURN is typically the last clause
                }
                AqlClause::Sort { items: _ } => {
                    // Sorting is deferred until after all rows are collected
                    continue;
                }
                AqlClause::Limit {
                    offset: _,
                    count: _,
                } => {
                    // Limiting is deferred until after all rows are collected
                    continue;
                }
                _ => {
                    // Other clauses not yet implemented
                    warn!("Unsupported clause type in AQL query execution");
                }
            }
        }

        // Apply SORT and LIMIT if present
        result_data = self.apply_sort_and_limit(&query.clauses, result_data)?;

        let mut metadata = HashMap::new();
        metadata.insert(
            "rows_returned".to_string(),
            AqlValue::Number(serde_json::Number::from(result_data.len())),
        );
        metadata.insert(
            "execution_time".to_string(),
            AqlValue::String(chrono::Utc::now().to_rfc3339()),
        );

        Ok(AqlQueryResult {
            data: result_data,
            metadata,
        })
    }

    /// Execute FOR clause - get documents from collection
    async fn execute_for_clause(
        &self,
        storage: &AqlStorage,
        collection_name: &str,
    ) -> ProtocolResult<Vec<AqlDocument>> {
        // Get all documents from the collection
        storage.get_collection_documents(collection_name).await
    }

    /// Evaluate an AQL expression
    fn evaluate_expression(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        use crate::protocols::aql::aql_parser::AqlExpression;

        match expression {
            AqlExpression::Variable(name) => context
                .get(name)
                .cloned()
                .ok_or_else(|| ProtocolError::AqlError(format!("Variable '{}' not found", name))),
            AqlExpression::Literal(value) => Ok(value.clone()),
            AqlExpression::PropertyAccess { object, property } => {
                let obj_value = context.get(object).ok_or_else(|| {
                    ProtocolError::AqlError(format!("Object '{}' not found", object))
                })?;

                if let AqlValue::Object(obj_map) = obj_value {
                    obj_map.get(property).cloned().ok_or_else(|| {
                        ProtocolError::AqlError(format!("Property '{}' not found", property))
                    })
                } else {
                    Err(ProtocolError::AqlError(
                        "Property access on non-object".to_string(),
                    ))
                }
            }
            AqlExpression::Object(fields) => {
                let mut result = HashMap::new();
                for (key, expr) in fields {
                    result.insert(key.clone(), self.evaluate_expression(expr, context)?);
                }
                Ok(AqlValue::Object(result))
            }
            AqlExpression::Array(elements) => {
                let mut result = Vec::new();
                for expr in elements {
                    result.push(self.evaluate_expression(expr, context)?);
                }
                Ok(AqlValue::Array(result))
            }
        }
    }

    /// Evaluate an AQL condition
    fn evaluate_condition(
        &self,
        condition: &AqlCondition,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        use crate::protocols::aql::aql_parser::AqlCondition;

        match condition {
            AqlCondition::Comparison {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate_expression(left, context)?;
                let right_val = self.evaluate_expression(right, context)?;
                self.compare_values(&left_val, operator, &right_val)
            } // Note: AqlCondition currently only supports Comparison
              // AND, OR, NOT would need to be added to the enum
        }
    }

    /// Compare two AQL values
    fn compare_values(
        &self,
        left: &AqlValue,
        operator: &ComparisonOperator,
        right: &AqlValue,
    ) -> ProtocolResult<bool> {
        use crate::protocols::aql::data_model::AqlValue;

        match (left, right) {
            (AqlValue::Number(l), AqlValue::Number(r)) => {
                let l_f64 = l.as_f64().unwrap_or(0.0);
                let r_f64 = r.as_f64().unwrap_or(0.0);
                Ok(match operator {
                    ComparisonOperator::Equals => (l_f64 - r_f64).abs() < f64::EPSILON,
                    ComparisonOperator::NotEquals => (l_f64 - r_f64).abs() >= f64::EPSILON,
                    ComparisonOperator::Greater => l_f64 > r_f64,
                    ComparisonOperator::Less => l_f64 < r_f64,
                    ComparisonOperator::GreaterOrEqual => l_f64 >= r_f64,
                    ComparisonOperator::LessOrEqual => l_f64 <= r_f64,
                })
            }
            (AqlValue::String(l), AqlValue::String(r)) => Ok(match operator {
                ComparisonOperator::Equals => l == r,
                ComparisonOperator::NotEquals => l != r,
                ComparisonOperator::Greater => l > r,
                ComparisonOperator::Less => l < r,
                ComparisonOperator::GreaterOrEqual => l >= r,
                ComparisonOperator::LessOrEqual => l <= r,
            }),
            _ => Ok(false),
        }
    }

    /// Convert AQL document to AQL value
    fn document_to_value(&self, doc: &AqlDocument) -> AqlValue {
        AqlValue::Object(doc.data.clone())
    }

    /// Apply SORT and LIMIT clauses
    fn apply_sort_and_limit(
        &self,
        clauses: &[AqlClause],
        mut results: Vec<AqlValue>,
    ) -> ProtocolResult<Vec<AqlValue>> {
        use crate::protocols::aql::aql_parser::AqlClause;

        // Apply SORT if present
        for clause in clauses {
            if let AqlClause::Sort { items: _ } = clause {
                // Simplified sorting - would need proper implementation
                // For now, just keep original order
                break;
            }
        }

        // Apply LIMIT if present
        for clause in clauses {
            if let AqlClause::Limit { offset, count } = clause {
                let offset = offset.unwrap_or(0) as usize;
                let count = *count as usize;
                results = results.into_iter().skip(offset).take(count).collect();
                break;
            }
        }

        Ok(results)
    }

    /// Remove duplicate results (for DISTINCT)
    fn deduplicate_results(&self, results: Vec<AqlValue>) -> Vec<AqlValue> {
        let mut seen = std::collections::HashSet::new();
        let mut unique = Vec::new();

        for result in results {
            // Simple deduplication based on string representation
            let key = format!("{:?}", result);
            if seen.insert(key) {
                unique.push(result);
            }
        }

        unique
    }
}

impl Default for AqlQueryEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphrag_function_detection() {
        let engine = AqlQueryEngine::new();

        assert!(engine.is_graphrag_function_call(
            "FOR result IN GRAPHRAG_QUERY('kg', 'query', {}) RETURN result"
        ));
        assert!(
            engine.is_graphrag_function_call("FOR stats IN GRAPHRAG_GET_STATS('kg') RETURN stats")
        );
        assert!(!engine.is_graphrag_function_call("FOR doc IN documents RETURN doc"));
    }

    #[tokio::test]
    async fn test_regular_aql_query() {
        let engine = AqlQueryEngine::new();
        let result = engine
            .execute_query("FOR doc IN documents RETURN doc")
            .await;

        // Query should fail without storage backend
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Storage backend required"));
    }

    #[tokio::test]
    async fn test_graphrag_function_without_engine() {
        let engine = AqlQueryEngine::new();
        let result = engine
            .execute_query("FOR result IN GRAPHRAG_QUERY('kg', 'query', {}) RETURN result")
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("GraphRAG functions require"));
    }
}
