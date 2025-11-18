//! SQL Integration for ML Functions
//!
//! Integrates machine learning functions with the Orbit-RS SQL engine,
//! providing seamless access to ML capabilities through SQL queries.

pub mod executor;
pub mod function_registry;
pub mod optimizer;

use crate::ml::{FunctionSignature, MLFunctionRegistry, MLValue};
use crate::postgres_wire::sql::ast::FunctionCall;
use orbit_shared::OrbitResult;

/// SQL-ML bridge that converts SQL function calls to ML function execution
pub struct SqlMlBridge {
    /// ML function registry
    ml_registry: MLFunctionRegistry,
}

impl SqlMlBridge {
    /// Create new SQL-ML bridge
    pub fn new(ml_registry: MLFunctionRegistry) -> Self {
        Self { ml_registry }
    }

    /// Execute ML function from SQL function call
    pub async fn execute_ml_function(&self, function_call: &FunctionCall) -> OrbitResult<MLValue> {
        let function_name = match &function_call.name {
            crate::postgres_wire::sql::ast::FunctionName::Simple(name) => name.clone(),
            crate::postgres_wire::sql::ast::FunctionName::Qualified { schema: _, name } => {
                name.clone()
            }
        };

        // Convert SQL expressions to ML values
        let mut ml_args = Vec::new();
        for arg in &function_call.args {
            let ml_value = self.convert_expression_to_ml_value(arg).await?;
            ml_args.push(ml_value);
        }

        // Execute the ML function
        self.ml_registry
            .execute_function(&function_name, ml_args)
            .await
    }

    /// Convert SQL value to ML value  
    #[allow(clippy::only_used_in_recursion)]
    fn convert_sql_value_to_ml_value<'a>(
        &'a self,
        sql_value: &'a crate::postgres_wire::sql::types::SqlValue,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = OrbitResult<MLValue>> + 'a>> {
        Box::pin(async move {
            use crate::postgres_wire::sql::types::SqlValue;
            match sql_value {
                SqlValue::DoublePrecision(f) => Ok(MLValue::Float(*f)),
                SqlValue::Real(f) => Ok(MLValue::Float(*f as f64)),
                SqlValue::Integer(i) => Ok(MLValue::Integer(*i as i64)),
                SqlValue::BigInt(i) => Ok(MLValue::Integer(*i)),
                SqlValue::SmallInt(i) => Ok(MLValue::Integer(*i as i64)),
                SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                    Ok(MLValue::String(s.clone()))
                }
                SqlValue::Boolean(b) => Ok(MLValue::Boolean(*b)),
                SqlValue::Null => Ok(MLValue::Null),
                SqlValue::Array(array) => {
                    let mut ml_array = Vec::new();
                    for item in array {
                        let ml_item = self.convert_sql_value_to_ml_value(item).await?;
                        ml_array.push(ml_item);
                    }
                    Ok(MLValue::Array(ml_array))
                }
                _ => Err(orbit_shared::OrbitError::internal(format!(
                    "Unsupported SQL value type for ML conversion: {sql_value:?}"
                ))),
            }
        })
    }

    /// Convert SQL expression to ML value
    fn convert_expression_to_ml_value<'a>(
        &'a self,
        expr: &'a crate::postgres_wire::sql::ast::Expression,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = OrbitResult<MLValue>> + 'a>> {
        Box::pin(async move {
            use crate::postgres_wire::sql::ast::Expression;

            match expr {
                Expression::Literal(sql_value) => {
                    use crate::postgres_wire::sql::types::SqlValue;
                    match sql_value {
                        SqlValue::DoublePrecision(f) => Ok(MLValue::Float(*f)),
                        SqlValue::Real(f) => Ok(MLValue::Float(*f as f64)),
                        SqlValue::Integer(i) => Ok(MLValue::Integer(*i as i64)),
                        SqlValue::BigInt(i) => Ok(MLValue::Integer(*i)),
                        SqlValue::SmallInt(i) => Ok(MLValue::Integer(*i as i64)),
                        SqlValue::Text(s) | SqlValue::Varchar(s) | SqlValue::Char(s) => {
                            Ok(MLValue::String(s.clone()))
                        }
                        SqlValue::Boolean(b) => Ok(MLValue::Boolean(*b)),
                        SqlValue::Null => Ok(MLValue::Null),
                        SqlValue::Array(array) => {
                            let mut ml_array = Vec::new();
                            for item in array {
                                let ml_item = self.convert_sql_value_to_ml_value(item).await?;
                                ml_array.push(ml_item);
                            }
                            Ok(MLValue::Array(ml_array))
                        }
                        _ => Err(orbit_shared::OrbitError::internal(format!(
                            "Unsupported SQL value type for ML conversion: {sql_value:?}"
                        ))),
                    }
                }
                Expression::Column(_) => {
                    // In a real implementation, this would fetch the column value
                    // For now, return a placeholder
                    Err(orbit_shared::OrbitError::internal(
                        "Column reference conversion not implemented yet",
                    ))
                }
                Expression::Function(func_call) => {
                    // Recursively handle nested function calls
                    self.execute_ml_function(func_call).await
                }
                _ => Err(orbit_shared::OrbitError::internal(format!(
                    "Expression type conversion not implemented: {expr:?}"
                ))),
            }
        })
    }

    /// Get available ML functions
    pub fn get_ml_function_signatures(&self) -> Vec<FunctionSignature> {
        // This would be implemented to return all registered ML function signatures
        // For now, return empty vector
        Vec::new()
    }
}

/// ML function categories for SQL documentation
#[derive(Debug, Clone)]
pub enum MLFunctionCategory {
    Statistical,
    Supervised,
    Unsupervised,
    FeatureEngineering,
    TimeSeries,
    NLP,
    Vector,
}

/// ML function metadata for SQL integration
#[derive(Debug, Clone)]
pub struct MLFunctionMetadata {
    /// Function signature
    pub signature: FunctionSignature,
    /// Function category
    pub category: MLFunctionCategory,
    /// SQL usage examples
    pub examples: Vec<String>,
    /// Performance characteristics
    pub performance_notes: Vec<String>,
}

/// Registry of ML functions for SQL use
pub struct SqlMLFunctionRegistry {
    /// Registered functions with metadata
    functions: std::collections::HashMap<String, MLFunctionMetadata>,
}

impl SqlMLFunctionRegistry {
    /// Create new SQL ML function registry
    pub fn new() -> Self {
        Self {
            functions: std::collections::HashMap::new(),
        }
    }

    /// Register an ML function for SQL use
    pub fn register_function(&mut self, metadata: MLFunctionMetadata) {
        self.functions
            .insert(metadata.signature.name.clone(), metadata);
    }

    /// Get function metadata by name
    pub fn get_function(&self, name: &str) -> Option<&MLFunctionMetadata> {
        self.functions.get(name)
    }

    /// List all available functions
    pub fn list_functions(&self) -> Vec<&MLFunctionMetadata> {
        self.functions.values().collect()
    }

    /// List functions by category
    pub fn list_functions_by_category(
        &self,
        category: MLFunctionCategory,
    ) -> Vec<&MLFunctionMetadata> {
        self.functions
            .values()
            .filter(|meta| {
                std::mem::discriminant(&meta.category) == std::mem::discriminant(&category)
            })
            .collect()
    }

    /// Initialize with built-in ML functions
    pub fn with_builtin_functions() -> Self {
        let mut registry = Self::new();

        // Register statistical functions
        registry.register_function(MLFunctionMetadata {
            signature: FunctionSignature {
                name: "ML_LINEAR_REGRESSION".to_string(),
                parameters: vec![
                    crate::ml::ParameterSpec {
                        name: "features".to_string(),
                        param_type: crate::ml::FeatureType::Vector(0),
                        required: true,
                        default: None,
                    },
                    crate::ml::ParameterSpec {
                        name: "target".to_string(),
                        param_type: crate::ml::FeatureType::Vector(0),
                        required: true,
                        default: None,
                    },
                ],
                return_type: crate::ml::FeatureType::Vector(0),
                description: "Performs linear regression and returns coefficients".to_string(),
            },
            category: MLFunctionCategory::Statistical,
            examples: vec![
                "SELECT ML_LINEAR_REGRESSION(ARRAY[age, income], price) FROM houses".to_string(),
                "SELECT *, ML_LINEAR_REGRESSION(features, target) OVER (PARTITION BY category) FROM data".to_string(),
            ],
            performance_notes: vec![
                "O(n*p^2 + p^3) where n=samples, p=features".to_string(),
                "Requires matrix inversion, may be unstable for ill-conditioned data".to_string(),
            ],
        });

        registry.register_function(MLFunctionMetadata {
            signature: FunctionSignature {
                name: "ML_CORRELATION".to_string(),
                parameters: vec![
                    crate::ml::ParameterSpec {
                        name: "x".to_string(),
                        param_type: crate::ml::FeatureType::Vector(0),
                        required: true,
                        default: None,
                    },
                    crate::ml::ParameterSpec {
                        name: "y".to_string(),
                        param_type: crate::ml::FeatureType::Vector(0),
                        required: true,
                        default: None,
                    },
                ],
                return_type: crate::ml::FeatureType::Float64,
                description: "Computes Pearson correlation coefficient".to_string(),
            },
            category: MLFunctionCategory::Statistical,
            examples: vec![
                "SELECT ML_CORRELATION(x_values, y_values) FROM dataset".to_string(),
                "SELECT var1, var2, ML_CORRELATION(var1, var2) FROM correlation_analysis"
                    .to_string(),
            ],
            performance_notes: vec!["O(n) where n=number of data points".to_string()],
        });

        registry.register_function(MLFunctionMetadata {
            signature: FunctionSignature {
                name: "ML_ZSCORE".to_string(),
                parameters: vec![
                    crate::ml::ParameterSpec {
                        name: "values".to_string(),
                        param_type: crate::ml::FeatureType::Vector(0),
                        required: true,
                        default: None,
                    },
                    crate::ml::ParameterSpec {
                        name: "mean".to_string(),
                        param_type: crate::ml::FeatureType::Float64,
                        required: false,
                        default: Some("calculated".to_string()),
                    },
                    crate::ml::ParameterSpec {
                        name: "std".to_string(),
                        param_type: crate::ml::FeatureType::Float64,
                        required: false,
                        default: Some("calculated".to_string()),
                    },
                ],
                return_type: crate::ml::FeatureType::Vector(0),
                description: "Computes Z-score normalization".to_string(),
            },
            category: MLFunctionCategory::FeatureEngineering,
            examples: vec![
                "SELECT ML_ZSCORE(values) FROM measurements".to_string(),
                "SELECT ML_ZSCORE(price, 50000, 15000) FROM products".to_string(),
            ],
            performance_notes: vec!["O(n) where n=number of values".to_string()],
        });

        registry
    }
}

impl Default for SqlMLFunctionRegistry {
    fn default() -> Self {
        Self::with_builtin_functions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_ml_function_registry() {
        let registry = SqlMLFunctionRegistry::with_builtin_functions();

        assert!(registry.get_function("ML_LINEAR_REGRESSION").is_some());
        assert!(registry.get_function("ML_CORRELATION").is_some());
        assert!(registry.get_function("ML_ZSCORE").is_some());

        let statistical_functions =
            registry.list_functions_by_category(MLFunctionCategory::Statistical);
        assert!(statistical_functions.len() >= 2);
    }

    #[test]
    fn test_function_metadata() {
        let registry = SqlMLFunctionRegistry::with_builtin_functions();
        let linear_reg = registry.get_function("ML_LINEAR_REGRESSION").unwrap();

        assert_eq!(linear_reg.signature.name, "ML_LINEAR_REGRESSION");
        assert_eq!(linear_reg.signature.parameters.len(), 2);
        assert!(!linear_reg.examples.is_empty());
    }
}
