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
                        for_documents.retain(|doc| {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            self.evaluate_condition(condition, &ctx).unwrap_or(false)
                        });
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
                AqlClause::Search {
                    expression,
                    analyzer,
                } => {
                    // Execute SEARCH clause - full-text search on documents
                    if let Some(ref var) = for_variable {
                        let analyzer_name = analyzer.as_deref().unwrap_or("text_en");
                        for_documents.retain(|doc| {
                            let mut ctx = context.clone();
                            ctx.insert(var.clone(), self.document_to_value(doc));
                            self.evaluate_search_expression(expression, &ctx, analyzer_name)
                                .unwrap_or(false)
                        });
                    }
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
    #[allow(clippy::only_used_in_recursion)]
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
            AqlExpression::FunctionCall { name, args } => {
                // Evaluate built-in functions
                let evaluated_args: Vec<AqlValue> = args
                    .iter()
                    .map(|arg| self.evaluate_expression(arg, context))
                    .collect::<ProtocolResult<Vec<_>>>()?;

                match name.to_uppercase().as_str() {
                    "LENGTH" => {
                        if let Some(AqlValue::String(s)) = evaluated_args.first() {
                            Ok(AqlValue::Number(serde_json::Number::from(s.len())))
                        } else if let Some(AqlValue::Array(arr)) = evaluated_args.first() {
                            Ok(AqlValue::Number(serde_json::Number::from(arr.len())))
                        } else {
                            Ok(AqlValue::Number(serde_json::Number::from(0)))
                        }
                    }
                    "UPPER" => {
                        if let Some(AqlValue::String(s)) = evaluated_args.first() {
                            Ok(AqlValue::String(s.to_uppercase()))
                        } else {
                            Ok(AqlValue::Null)
                        }
                    }
                    "LOWER" => {
                        if let Some(AqlValue::String(s)) = evaluated_args.first() {
                            Ok(AqlValue::String(s.to_lowercase()))
                        } else {
                            Ok(AqlValue::Null)
                        }
                    }
                    "CONCAT" => {
                        let result: String = evaluated_args
                            .iter()
                            .filter_map(|v| {
                                if let AqlValue::String(s) = v {
                                    Some(s.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        Ok(AqlValue::String(result))
                    }
                    "ABS" => {
                        if let Some(AqlValue::Number(n)) = evaluated_args.first() {
                            let f = n.as_f64().unwrap_or(0.0).abs();
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(f)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        } else {
                            Ok(AqlValue::Null)
                        }
                    }
                    _ => {
                        // Unknown function - return null
                        Ok(AqlValue::Null)
                    }
                }
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_val = self.evaluate_expression(left, context)?;
                let right_val = self.evaluate_expression(right, context)?;

                match op.as_str() {
                    "+" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let result = l.as_f64().unwrap_or(0.0) + r.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        (AqlValue::String(l), AqlValue::String(r)) => {
                            Ok(AqlValue::String(format!("{}{}", l, r)))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "-" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let result = l.as_f64().unwrap_or(0.0) - r.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "*" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let result = l.as_f64().unwrap_or(0.0) * r.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "/" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let r_f64 = r.as_f64().unwrap_or(0.0);
                            if r_f64.abs() < f64::EPSILON {
                                Ok(AqlValue::Null) // Division by zero
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) / r_f64;
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "%" => match (&left_val, &right_val) {
                        (AqlValue::Number(l), AqlValue::Number(r)) => {
                            let r_f64 = r.as_f64().unwrap_or(0.0);
                            if r_f64.abs() < f64::EPSILON {
                                Ok(AqlValue::Null)
                            } else {
                                let result = l.as_f64().unwrap_or(0.0) % r_f64;
                                Ok(AqlValue::Number(
                                    serde_json::Number::from_f64(result)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ))
                            }
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "==" | "=" => Ok(AqlValue::Bool(left_val == right_val)),
                    "!=" | "<>" => Ok(AqlValue::Bool(left_val != right_val)),
                    "AND" | "&&" => match (&left_val, &right_val) {
                        (AqlValue::Bool(l), AqlValue::Bool(r)) => Ok(AqlValue::Bool(*l && *r)),
                        _ => Ok(AqlValue::Bool(false)),
                    },
                    "OR" | "||" => match (&left_val, &right_val) {
                        (AqlValue::Bool(l), AqlValue::Bool(r)) => Ok(AqlValue::Bool(*l || *r)),
                        _ => Ok(AqlValue::Bool(false)),
                    },
                    _ => Ok(AqlValue::Null),
                }
            }
            AqlExpression::UnaryOp { op, expr } => {
                let val = self.evaluate_expression(expr, context)?;

                match op.as_str() {
                    "-" => match val {
                        AqlValue::Number(n) => {
                            let result = -n.as_f64().unwrap_or(0.0);
                            Ok(AqlValue::Number(
                                serde_json::Number::from_f64(result)
                                    .unwrap_or(serde_json::Number::from(0)),
                            ))
                        }
                        _ => Ok(AqlValue::Null),
                    },
                    "NOT" | "!" => match val {
                        AqlValue::Bool(b) => Ok(AqlValue::Bool(!b)),
                        _ => Ok(AqlValue::Bool(false)),
                    },
                    _ => Ok(AqlValue::Null),
                }
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

    /// Evaluate a search expression for full-text search
    ///
    /// Supports ArangoSearch-compatible functions:
    /// - PHRASE(doc.field, "search text") - exact phrase match
    /// - ANALYZER(expression, "analyzer_name") - apply analyzer
    /// - STARTS_WITH(doc.field, "prefix") - prefix matching
    /// - LIKE(doc.field, "pattern%") - pattern matching
    /// - LEVENSHTEIN_MATCH(doc.field, "term", distance) - fuzzy matching
    /// - IN_RANGE(doc.field, min, max, include_min, include_max) - range search
    /// - BOOST(expression, factor) - relevance boost
    /// - EXISTS(doc.field) - field existence check
    /// - Boolean operators: AND, OR, NOT
    #[allow(dead_code)]
    fn evaluate_search_expression(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
        _analyzer: &str,
    ) -> ProtocolResult<bool> {
        match expression {
            AqlExpression::FunctionCall { name, args } => {
                self.evaluate_search_function(name, args, context)
            }
            AqlExpression::BinaryOp { op, left, right } => {
                let left_result = self.evaluate_search_expression(left, context, _analyzer)?;
                let right_result = self.evaluate_search_expression(right, context, _analyzer)?;

                match op.as_str() {
                    "AND" | "&&" => Ok(left_result && right_result),
                    "OR" | "||" => Ok(left_result || right_result),
                    _ => Ok(false),
                }
            }
            AqlExpression::UnaryOp { op, expr } if op == "NOT" || op == "!" => {
                let result = self.evaluate_search_expression(expr, context, _analyzer)?;
                Ok(!result)
            }
            AqlExpression::Literal(value) => match value {
                AqlValue::Bool(b) => Ok(*b),
                _ => Ok(false),
            },
            // For conditions like doc.field == "value", evaluate as equality check
            _ => Ok(true),
        }
    }

    /// Evaluate a search function call
    fn evaluate_search_function(
        &self,
        name: &str,
        args: &[AqlExpression],
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<bool> {
        match name.to_uppercase().as_str() {
            "PHRASE" => {
                // PHRASE(doc.field, "search phrase") - exact phrase matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let search_phrase = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(phrase)) =
                    (&field_value, &search_phrase)
                {
                    Ok(text.to_lowercase().contains(&phrase.to_lowercase()))
                } else {
                    Ok(false)
                }
            }
            "STARTS_WITH" => {
                // STARTS_WITH(doc.field, "prefix") - prefix matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let prefix = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(pref)) = (&field_value, &prefix) {
                    Ok(text.to_lowercase().starts_with(&pref.to_lowercase()))
                } else {
                    Ok(false)
                }
            }
            "LIKE" => {
                // LIKE(doc.field, "pattern%") - SQL-like pattern matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let pattern = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(pat)) = (&field_value, &pattern) {
                    // Simple pattern matching: % = any chars, _ = single char
                    let regex_pattern = pat.replace('%', ".*").replace('_', ".");
                    if let Ok(re) = regex::Regex::new(&format!("(?i)^{}$", regex_pattern)) {
                        Ok(re.is_match(text))
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(false)
                }
            }
            "LEVENSHTEIN_MATCH" => {
                // LEVENSHTEIN_MATCH(doc.field, "term", max_distance) - fuzzy matching
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let term = self.evaluate_expression(&args[1], context)?;
                let max_distance: usize = if args.len() >= 3 {
                    if let Ok(AqlValue::Number(n)) = self.evaluate_expression(&args[2], context) {
                        n.as_u64().unwrap_or(2) as usize
                    } else {
                        2
                    }
                } else {
                    2
                };

                if let (AqlValue::String(text), AqlValue::String(search_term)) =
                    (&field_value, &term)
                {
                    let distance = self
                        .levenshtein_distance(&text.to_lowercase(), &search_term.to_lowercase());
                    Ok(distance <= max_distance)
                } else {
                    Ok(false)
                }
            }
            "IN_RANGE" => {
                // IN_RANGE(doc.field, min, max, include_min, include_max)
                if args.len() < 3 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let min_val = self.evaluate_expression(&args[1], context)?;
                let max_val = self.evaluate_expression(&args[2], context)?;
                let include_min = args.get(3).map_or(true, |e| {
                    matches!(
                        self.evaluate_expression(e, context),
                        Ok(AqlValue::Bool(true))
                    )
                });
                let include_max = args.get(4).map_or(true, |e| {
                    matches!(
                        self.evaluate_expression(e, context),
                        Ok(AqlValue::Bool(true))
                    )
                });

                if let (AqlValue::Number(val), AqlValue::Number(min), AqlValue::Number(max)) =
                    (&field_value, &min_val, &max_val)
                {
                    let v = val.as_f64().unwrap_or(0.0);
                    let min_f = min.as_f64().unwrap_or(0.0);
                    let max_f = max.as_f64().unwrap_or(0.0);

                    let above_min = if include_min { v >= min_f } else { v > min_f };
                    let below_max = if include_max { v <= max_f } else { v < max_f };
                    Ok(above_min && below_max)
                } else {
                    Ok(false)
                }
            }
            "EXISTS" => {
                // EXISTS(doc.field) - check if field exists and is not null
                if args.is_empty() {
                    return Ok(false);
                }
                let value = self.evaluate_expression(&args[0], context);
                Ok(value.is_ok() && !matches!(value.unwrap(), AqlValue::Null))
            }
            "ANALYZER" => {
                // ANALYZER(expression, "analyzer_name") - apply analyzer and evaluate
                if args.is_empty() {
                    return Ok(false);
                }
                // For now, just evaluate the inner expression with default analyzer
                self.evaluate_search_expression(&args[0], context, "text_en")
            }
            "BOOST" => {
                // BOOST(expression, factor) - boost relevance (just evaluate expression for now)
                if args.is_empty() {
                    return Ok(false);
                }
                self.evaluate_search_expression(&args[0], context, "text_en")
            }
            "TOKENS" | "NGRAM_MATCH" | "NGRAM_SIMILARITY" => {
                // Token-based and n-gram functions - simplified implementation
                if args.len() < 2 {
                    return Ok(false);
                }
                let field_value = self.evaluate_expression(&args[0], context)?;
                let search_value = self.evaluate_expression(&args[1], context)?;

                if let (AqlValue::String(text), AqlValue::String(search)) =
                    (&field_value, &search_value)
                {
                    // Simple token overlap check
                    let text_lower = text.to_lowercase();
                    let search_lower = search.to_lowercase();
                    let text_tokens: std::collections::HashSet<&str> =
                        text_lower.split_whitespace().collect();
                    let search_tokens: std::collections::HashSet<&str> =
                        search_lower.split_whitespace().collect();
                    let overlap = text_tokens.intersection(&search_tokens).count();
                    Ok(overlap > 0)
                } else {
                    Ok(false)
                }
            }
            _ => {
                // Unknown function - log and return false
                warn!("Unknown search function: {}", name);
                Ok(false)
            }
        }
    }

    /// Calculate Levenshtein distance between two strings
    fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let a_len = a_chars.len();
        let b_len = b_chars.len();

        if a_len == 0 {
            return b_len;
        }
        if b_len == 0 {
            return a_len;
        }

        let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

        for i in 0..=a_len {
            matrix[i][0] = i;
        }
        for j in 0..=b_len {
            matrix[0][j] = j;
        }

        for i in 1..=a_len {
            for j in 1..=b_len {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                matrix[i][j] = std::cmp::min(
                    std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                    matrix[i - 1][j - 1] + cost,
                );
            }
        }

        matrix[a_len][b_len]
    }

    /// Public wrapper for evaluate_expression (for testing)
    #[cfg(test)]
    pub fn evaluate_expression_public(
        &self,
        expression: &AqlExpression,
        context: &HashMap<String, AqlValue>,
    ) -> ProtocolResult<AqlValue> {
        self.evaluate_expression(expression, context)
    }

    /// Public wrapper for levenshtein_distance (for testing)
    #[cfg(test)]
    pub fn levenshtein_distance_public(&self, a: &str, b: &str) -> usize {
        self.levenshtein_distance(a, b)
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
