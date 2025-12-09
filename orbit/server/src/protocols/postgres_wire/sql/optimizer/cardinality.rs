//! Cardinality Estimation for Query Optimization
//!
//! This module provides advanced cardinality estimation by:
//! - Extracting predicates from WHERE clauses
//! - Using histogram-based selectivity estimation
//! - Handling complex predicates (AND, OR, NOT)
//! - Estimating join cardinalities
//!
//! It bridges the SQL AST with the statistics system to provide
//! accurate row count estimates for query planning.

use crate::protocols::postgres_wire::sql::ast::{
    BinaryOperator, Expression, InList, UnaryOperator,
};
use crate::protocols::postgres_wire::sql::statistics::{
    Predicate, StatisticsManager, TableStatistics,
};
use crate::protocols::postgres_wire::sql::types::SqlValue;

/// Cardinality estimator configuration
#[derive(Debug, Clone)]
pub struct CardinalityConfig {
    /// Default selectivity for equality predicates when no stats available
    pub default_eq_selectivity: f64,
    /// Default selectivity for range predicates when no stats available
    pub default_range_selectivity: f64,
    /// Default selectivity for LIKE predicates
    pub default_like_selectivity: f64,
    /// Default selectivity for IS NULL predicates
    pub default_null_selectivity: f64,
    /// Minimum selectivity (to avoid zero estimates)
    pub min_selectivity: f64,
    /// Correlation factor for combining predicates (1.0 = independent)
    pub predicate_correlation: f64,
}

impl Default for CardinalityConfig {
    fn default() -> Self {
        Self {
            default_eq_selectivity: 0.1,
            default_range_selectivity: 0.33,
            default_like_selectivity: 0.25,
            default_null_selectivity: 0.01,
            min_selectivity: 0.0001,
            predicate_correlation: 1.0, // Assume independence by default
        }
    }
}

/// Cardinality estimator for query optimization
pub struct CardinalityEstimator {
    config: CardinalityConfig,
}

impl CardinalityEstimator {
    /// Create a new cardinality estimator
    pub fn new(config: CardinalityConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn new_default() -> Self {
        Self::new(CardinalityConfig::default())
    }

    /// Estimate cardinality for a table with optional filter
    pub fn estimate_table_cardinality(
        &self,
        table_stats: Option<&TableStatistics>,
        filter: Option<&Expression>,
    ) -> usize {
        let base_rows = table_stats.map(|s| s.row_count).unwrap_or(1000);

        if let Some(expr) = filter {
            let selectivity = self.estimate_selectivity(expr, table_stats);
            ((base_rows as f64) * selectivity).max(1.0) as usize
        } else {
            base_rows
        }
    }

    /// Estimate selectivity of an expression
    pub fn estimate_selectivity(
        &self,
        expr: &Expression,
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        match expr {
            // Binary comparisons
            Expression::Binary {
                left,
                operator,
                right,
            } => self.estimate_binary_selectivity(left, operator, right, table_stats),

            // Unary operators
            Expression::Unary { operator, operand } => {
                self.estimate_unary_selectivity(operator, operand, table_stats)
            }

            // IS NULL / IS NOT NULL
            Expression::IsNull { expr, negated } => {
                let base = self.estimate_is_null_selectivity(expr, table_stats);
                if *negated {
                    1.0 - base
                } else {
                    base
                }
            }

            // IN list
            Expression::In {
                expr,
                list,
                negated,
            } => {
                let base = self.estimate_in_list_selectivity(expr, list, table_stats);
                if *negated {
                    1.0 - base
                } else {
                    base
                }
            }

            // BETWEEN
            Expression::Between {
                expr,
                low,
                high,
                negated,
            } => {
                let base = self.estimate_between_selectivity(expr, low, high, table_stats);
                if *negated {
                    1.0 - base
                } else {
                    base
                }
            }

            // LIKE patterns
            Expression::Like {
                pattern, negated, ..
            } => {
                let base = self.estimate_like_selectivity(pattern);
                if *negated {
                    1.0 - base
                } else {
                    base
                }
            }

            // EXISTS subquery (assume 50% selectivity without stats)
            Expression::Exists(_) => 0.5,

            // Default for other expressions
            _ => 1.0,
        }
    }

    /// Estimate selectivity for binary operators
    fn estimate_binary_selectivity(
        &self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        match operator {
            // Logical operators combine child selectivities
            BinaryOperator::And => {
                let left_sel = self.estimate_selectivity(left, table_stats);
                let right_sel = self.estimate_selectivity(right, table_stats);
                // With correlation factor
                (left_sel * right_sel).powf(self.config.predicate_correlation)
            }
            BinaryOperator::Or => {
                let left_sel = self.estimate_selectivity(left, table_stats);
                let right_sel = self.estimate_selectivity(right, table_stats);
                // P(A or B) = P(A) + P(B) - P(A and B)
                (left_sel + right_sel - left_sel * right_sel).min(1.0)
            }

            // Comparison operators
            BinaryOperator::Equal => {
                self.estimate_comparison_selectivity(left, right, table_stats, "eq")
            }
            BinaryOperator::NotEqual => {
                1.0 - self.estimate_comparison_selectivity(left, right, table_stats, "eq")
            }
            BinaryOperator::LessThan
            | BinaryOperator::LessThanOrEqual
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterThanOrEqual => {
                self.estimate_comparison_selectivity(left, right, table_stats, "range")
            }

            // String pattern matching
            BinaryOperator::Like => self.estimate_like_selectivity(right),
            BinaryOperator::ILike => {
                // Case-insensitive LIKE typically has similar selectivity
                self.estimate_like_selectivity(right)
            }

            // Pattern matching (regex-like)
            BinaryOperator::Match => self.config.default_like_selectivity,
            BinaryOperator::NotMatch => 1.0 - self.config.default_like_selectivity,

            // Arithmetic operators don't filter, selectivity = 1.0
            BinaryOperator::Plus
            | BinaryOperator::Minus
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::Power
            | BinaryOperator::Concat
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor
            | BinaryOperator::LeftShift
            | BinaryOperator::RightShift => 1.0,

            // Array operators
            BinaryOperator::Contains | BinaryOperator::ContainedBy | BinaryOperator::Overlap => 0.1,

            // JSON/JSONB operators
            BinaryOperator::JsonExtract
            | BinaryOperator::JsonExtractText
            | BinaryOperator::JsonPathExtract
            | BinaryOperator::JsonPathExtractText
            | BinaryOperator::JsonConcat
            | BinaryOperator::JsonDelete
            | BinaryOperator::JsonDeletePath => 1.0,

            // JSON containment/existence operators
            BinaryOperator::JsonContains
            | BinaryOperator::JsonContainedBy
            | BinaryOperator::JsonExists
            | BinaryOperator::JsonExistsAny
            | BinaryOperator::JsonExistsAll => 0.5,

            // Range operators (PostgreSQL range types)
            BinaryOperator::RangeContains
            | BinaryOperator::RangeContainedBy
            | BinaryOperator::RangeOverlaps
            | BinaryOperator::RangeAdjacent
            | BinaryOperator::RangeStrictlyLeft
            | BinaryOperator::RangeStrictlyRight
            | BinaryOperator::RangeNotExtendRight
            | BinaryOperator::RangeNotExtendLeft => 0.1,

            // Vector similarity operators
            BinaryOperator::VectorDistance
            | BinaryOperator::VectorInnerProduct
            | BinaryOperator::VectorCosineDistance => {
                // These are typically used with ORDER BY LIMIT, not as filters
                1.0
            }

            // Similar pattern
            BinaryOperator::Similar => self.config.default_like_selectivity,

            // Set membership operators
            BinaryOperator::In => 0.1,
            BinaryOperator::NotIn => 0.9,

            // Is/IsNot operators
            BinaryOperator::Is => self.config.default_eq_selectivity,
            BinaryOperator::IsNot => 1.0 - self.config.default_eq_selectivity,
            BinaryOperator::IsDistinctFrom => 1.0 - self.config.default_eq_selectivity,
            BinaryOperator::IsNotDistinctFrom => self.config.default_eq_selectivity,

            // Regex operators
            BinaryOperator::RegexMatch | BinaryOperator::RegexMatchCaseInsensitive => {
                self.config.default_like_selectivity
            }
            BinaryOperator::RegexNotMatch | BinaryOperator::RegexNotMatchCaseInsensitive => {
                1.0 - self.config.default_like_selectivity
            }

            // Text Search operators
            BinaryOperator::TextSearchMatch => 0.1, // @@ match operator - relatively selective
            BinaryOperator::TextSearchContains | BinaryOperator::TextSearchContainedBy => 0.2,
            BinaryOperator::TextSearchConcat
            | BinaryOperator::TextSearchAnd
            | BinaryOperator::TextSearchNot
            | BinaryOperator::TextSearchFollowedBy => 1.0, // These produce tsquery, not filter
        }
    }

    /// Estimate comparison selectivity using column statistics
    fn estimate_comparison_selectivity(
        &self,
        left: &Expression,
        right: &Expression,
        table_stats: Option<&TableStatistics>,
        comparison_type: &str,
    ) -> f64 {
        // Try to extract column name
        let column_name = self.extract_column_name(left);

        // Try to extract literal value
        let literal_value = self.extract_literal_value(right);

        if let (Some(col_name), Some(stats)) = (column_name, table_stats) {
            if let Some(col_stats) = stats.get_column_stats(&col_name) {
                return match comparison_type {
                    "eq" => col_stats.selectivity_eq(stats.row_count),
                    "range" => col_stats.selectivity_range(literal_value.as_ref(), None),
                    _ => self.config.default_eq_selectivity,
                };
            }
        }

        // Default selectivity
        match comparison_type {
            "eq" => self.config.default_eq_selectivity,
            "range" => self.config.default_range_selectivity,
            _ => self.config.default_eq_selectivity,
        }
    }

    /// Estimate selectivity for unary operators
    fn estimate_unary_selectivity(
        &self,
        operator: &UnaryOperator,
        operand: &Expression,
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        match operator {
            UnaryOperator::Not => 1.0 - self.estimate_selectivity(operand, table_stats),
            // Other unary operators don't affect selectivity
            _ => self.estimate_selectivity(operand, table_stats),
        }
    }

    /// Estimate IS NULL selectivity
    fn estimate_is_null_selectivity(
        &self,
        expr: &Expression,
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        if let Some(col_name) = self.extract_column_name(expr) {
            if let Some(stats) = table_stats {
                if let Some(col_stats) = stats.get_column_stats(&col_name) {
                    return col_stats.selectivity_null(stats.row_count);
                }
            }
        }
        self.config.default_null_selectivity
    }

    /// Estimate IN list selectivity
    fn estimate_in_list_selectivity(
        &self,
        expr: &Expression,
        list: &InList,
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        let list_len = match list {
            InList::Expressions(exprs) => exprs.len(),
            InList::Subquery(_) => 10, // Assume subquery returns ~10 rows
        };

        if let Some(col_name) = self.extract_column_name(expr) {
            if let Some(stats) = table_stats {
                if let Some(col_stats) = stats.get_column_stats(&col_name) {
                    // Selectivity = number of values / distinct count
                    let eq_sel = col_stats.selectivity_eq(stats.row_count);
                    return (eq_sel * list_len as f64).min(1.0);
                }
            }
        }
        // Default: min of (list_len * default_eq_sel, 0.5)
        (self.config.default_eq_selectivity * list_len as f64).min(0.5)
    }

    /// Estimate BETWEEN selectivity
    fn estimate_between_selectivity(
        &self,
        expr: &Expression,
        low: &Expression,
        high: &Expression,
        table_stats: Option<&TableStatistics>,
    ) -> f64 {
        if let Some(col_name) = self.extract_column_name(expr) {
            if let Some(stats) = table_stats {
                if let Some(col_stats) = stats.get_column_stats(&col_name) {
                    let low_val = self.extract_literal_value(low);
                    let high_val = self.extract_literal_value(high);
                    return col_stats.selectivity_range(low_val.as_ref(), high_val.as_ref());
                }
            }
        }
        self.config.default_range_selectivity
    }

    /// Estimate LIKE selectivity based on pattern
    fn estimate_like_selectivity(&self, pattern: &Expression) -> f64 {
        if let Expression::Literal(SqlValue::Text(pat)) = pattern {
            // More specific patterns have lower selectivity
            if !pat.contains('%') && !pat.contains('_') {
                // Exact match
                return self.config.default_eq_selectivity;
            }
            if pat.starts_with('%') && pat.ends_with('%') {
                // Contains pattern - higher selectivity
                return self.config.default_like_selectivity * 2.0;
            }
            if pat.starts_with('%') {
                // Suffix match
                return self.config.default_like_selectivity * 1.5;
            }
            if pat.ends_with('%') {
                // Prefix match - can use index, lower selectivity
                return self.config.default_like_selectivity * 0.5;
            }
        }
        self.config.default_like_selectivity
    }

    /// Extract column name from an expression
    fn extract_column_name(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Column(col_ref) => Some(col_ref.name.clone()),
            _ => None,
        }
    }

    /// Extract literal value from an expression
    fn extract_literal_value(&self, expr: &Expression) -> Option<SqlValue> {
        if let Expression::Literal(value) = expr {
            Some(value.clone())
        } else {
            None
        }
    }

    /// Estimate join cardinality
    pub fn estimate_join_cardinality(
        &self,
        left_rows: usize,
        right_rows: usize,
        join_condition: Option<&Expression>,
        left_stats: Option<&TableStatistics>,
        right_stats: Option<&TableStatistics>,
    ) -> usize {
        // Without condition, it's a cross join
        if join_condition.is_none() {
            return left_rows * right_rows;
        }

        let condition = join_condition.unwrap();

        // Try to extract join columns for selectivity estimation
        if let Expression::Binary {
            left,
            operator: BinaryOperator::Equal,
            right,
        } = condition
        {
            let left_col = self.extract_column_name(left);
            let right_col = self.extract_column_name(right);

            if let (Some(l_col), Some(r_col)) = (left_col, right_col) {
                // Get distinct counts from both sides
                let left_distinct = left_stats
                    .and_then(|s| s.get_column_stats(&l_col))
                    .map(|cs| cs.distinct_count)
                    .unwrap_or(100);

                let right_distinct = right_stats
                    .and_then(|s| s.get_column_stats(&r_col))
                    .map(|cs| cs.distinct_count)
                    .unwrap_or(100);

                // Join selectivity = 1 / max(distinct_left, distinct_right)
                let max_distinct = left_distinct.max(right_distinct).max(1);
                let selectivity = 1.0 / max_distinct as f64;

                return ((left_rows as f64) * (right_rows as f64) * selectivity).max(1.0) as usize;
            }
        }

        // Default: assume 10% join selectivity
        ((left_rows as f64) * (right_rows as f64) * 0.1).max(1.0) as usize
    }

    /// Extract predicates from WHERE clause for cardinality estimation
    pub fn extract_predicates(&self, expr: &Expression) -> Vec<Predicate> {
        let mut predicates = Vec::new();
        self.extract_predicates_recursive(expr, &mut predicates);
        predicates
    }

    fn extract_predicates_recursive(&self, expr: &Expression, predicates: &mut Vec<Predicate>) {
        match expr {
            Expression::Binary {
                left,
                operator,
                right,
            } => {
                match operator {
                    BinaryOperator::And => {
                        // Recurse into both sides
                        self.extract_predicates_recursive(left, predicates);
                        self.extract_predicates_recursive(right, predicates);
                    }
                    BinaryOperator::Equal => {
                        if let (Some(col), Some(val)) = (
                            self.extract_column_name(left),
                            self.extract_literal_value(right),
                        ) {
                            predicates.push(Predicate::Eq {
                                column: col,
                                value: val,
                            });
                        }
                    }
                    BinaryOperator::LessThan | BinaryOperator::LessThanOrEqual => {
                        if let (Some(col), Some(val)) = (
                            self.extract_column_name(left),
                            self.extract_literal_value(right),
                        ) {
                            predicates.push(Predicate::Range {
                                column: col,
                                min: None,
                                max: Some(val),
                            });
                        }
                    }
                    BinaryOperator::GreaterThan | BinaryOperator::GreaterThanOrEqual => {
                        if let (Some(col), Some(val)) = (
                            self.extract_column_name(left),
                            self.extract_literal_value(right),
                        ) {
                            predicates.push(Predicate::Range {
                                column: col,
                                min: Some(val),
                                max: None,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Expression::IsNull { expr, negated } => {
                if let Some(col) = self.extract_column_name(expr) {
                    if *negated {
                        predicates.push(Predicate::IsNotNull { column: col });
                    } else {
                        predicates.push(Predicate::IsNull { column: col });
                    }
                }
            }
            _ => {}
        }
    }

    /// Estimate cardinality using predicates and statistics manager
    pub async fn estimate_with_stats_manager(
        &self,
        table_name: &str,
        filter: Option<&Expression>,
        stats_manager: &StatisticsManager,
    ) -> usize {
        if let Some(table_stats) = stats_manager.get_table_stats(table_name).await {
            if let Some(expr) = filter {
                let predicates = self.extract_predicates(expr);
                if !predicates.is_empty() {
                    if let Some(estimate) = stats_manager
                        .estimate_cardinality(table_name, &predicates)
                        .await
                    {
                        return estimate;
                    }
                }
                // Fall back to direct selectivity estimation
                let selectivity = self.estimate_selectivity(expr, Some(&table_stats));
                return ((table_stats.row_count as f64) * selectivity).max(1.0) as usize;
            }
            return table_stats.row_count;
        }

        // Default when no stats available
        1000
    }
}

impl Default for CardinalityEstimator {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::postgres_wire::sql::ast::ColumnRef;

    fn make_col_expr(name: &str) -> Expression {
        Expression::Column(ColumnRef {
            table: None,
            name: name.to_string(),
        })
    }

    fn make_int_literal(val: i32) -> Expression {
        Expression::Literal(SqlValue::Integer(val))
    }

    fn make_text_literal(val: &str) -> Expression {
        Expression::Literal(SqlValue::Text(val.to_string()))
    }

    #[test]
    fn test_equality_selectivity() {
        let estimator = CardinalityEstimator::new_default();

        let expr = Expression::Binary {
            left: Box::new(make_col_expr("id")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_int_literal(5)),
        };

        // Without stats, should use default
        let sel = estimator.estimate_selectivity(&expr, None);
        assert!((sel - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_and_selectivity() {
        let estimator = CardinalityEstimator::new_default();

        let left = Expression::Binary {
            left: Box::new(make_col_expr("a")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_int_literal(1)),
        };

        let right = Expression::Binary {
            left: Box::new(make_col_expr("b")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_int_literal(2)),
        };

        let and_expr = Expression::Binary {
            left: Box::new(left),
            operator: BinaryOperator::And,
            right: Box::new(right),
        };

        let sel = estimator.estimate_selectivity(&and_expr, None);
        // 0.1 * 0.1 = 0.01
        assert!(sel < 0.02);
    }

    #[test]
    fn test_or_selectivity() {
        let estimator = CardinalityEstimator::new_default();

        let left = Expression::Binary {
            left: Box::new(make_col_expr("status")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_text_literal("active")),
        };

        let right = Expression::Binary {
            left: Box::new(make_col_expr("status")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_text_literal("pending")),
        };

        let or_expr = Expression::Binary {
            left: Box::new(left),
            operator: BinaryOperator::Or,
            right: Box::new(right),
        };

        let sel = estimator.estimate_selectivity(&or_expr, None);
        // 0.1 + 0.1 - 0.01 = 0.19
        assert!(sel > 0.15 && sel < 0.25);
    }

    #[test]
    fn test_like_selectivity() {
        let estimator = CardinalityEstimator::new_default();

        // Prefix pattern (most selective)
        let prefix_like = Expression::Binary {
            left: Box::new(make_col_expr("name")),
            operator: BinaryOperator::Like,
            right: Box::new(make_text_literal("John%")),
        };
        let prefix_sel = estimator.estimate_selectivity(&prefix_like, None);

        // Contains pattern (least selective)
        let contains_like = Expression::Binary {
            left: Box::new(make_col_expr("name")),
            operator: BinaryOperator::Like,
            right: Box::new(make_text_literal("%ohn%")),
        };
        let contains_sel = estimator.estimate_selectivity(&contains_like, None);

        // Prefix should be more selective than contains
        assert!(prefix_sel < contains_sel);
    }

    #[test]
    fn test_is_null_selectivity() {
        let estimator = CardinalityEstimator::new_default();

        let expr = Expression::IsNull {
            expr: Box::new(make_col_expr("deleted_at")),
            negated: false,
        };
        let sel = estimator.estimate_selectivity(&expr, None);

        assert!(sel < 0.1); // Should be low for NULL
    }

    #[test]
    fn test_in_list_selectivity() {
        let estimator = CardinalityEstimator::new_default();

        let expr = Expression::In {
            expr: Box::new(make_col_expr("status")),
            list: InList::Expressions(vec![
                make_text_literal("a"),
                make_text_literal("b"),
                make_text_literal("c"),
            ]),
            negated: false,
        };

        let sel = estimator.estimate_selectivity(&expr, None);
        // Should be roughly 3 * 0.1 = 0.3
        assert!(sel > 0.2 && sel < 0.4);
    }

    #[test]
    fn test_join_cardinality() {
        let estimator = CardinalityEstimator::new_default();

        // Simple equi-join
        let join_condition = Expression::Binary {
            left: Box::new(make_col_expr("user_id")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_col_expr("id")),
        };

        let cardinality = estimator.estimate_join_cardinality(
            1000, // left rows
            100,  // right rows
            Some(&join_condition),
            None,
            None,
        );

        // Should be much less than cross join (100,000)
        assert!(cardinality < 10000);
        assert!(cardinality > 0);
    }

    #[test]
    fn test_extract_predicates() {
        let estimator = CardinalityEstimator::new_default();

        // a = 5 AND b > 10
        let expr = Expression::Binary {
            left: Box::new(Expression::Binary {
                left: Box::new(make_col_expr("a")),
                operator: BinaryOperator::Equal,
                right: Box::new(make_int_literal(5)),
            }),
            operator: BinaryOperator::And,
            right: Box::new(Expression::Binary {
                left: Box::new(make_col_expr("b")),
                operator: BinaryOperator::GreaterThan,
                right: Box::new(make_int_literal(10)),
            }),
        };

        let predicates = estimator.extract_predicates(&expr);
        assert_eq!(predicates.len(), 2);
    }

    #[test]
    fn test_table_cardinality_with_filter() {
        let estimator = CardinalityEstimator::new_default();

        // Create table stats
        let mut stats = TableStatistics::new("users");
        stats.set_row_count(10000);

        // Filter: id = 5
        let filter = Expression::Binary {
            left: Box::new(make_col_expr("id")),
            operator: BinaryOperator::Equal,
            right: Box::new(make_int_literal(5)),
        };

        let cardinality = estimator.estimate_table_cardinality(Some(&stats), Some(&filter));

        // Should be about 10% of 10000 = 1000
        assert!(cardinality > 500 && cardinality < 2000);
    }
}
