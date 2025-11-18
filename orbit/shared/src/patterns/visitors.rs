//! Visitor pattern for traversing and operating on data structures
//!
//! Separates algorithms from the objects they operate on, allowing new
//! operations without modifying existing structures.

use crate::error::{OrbitError, OrbitResult};
use serde::{Deserialize, Serialize};

// ===== Query AST for demonstration =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryNode {
    Select {
        columns: Vec<String>,
        from: Box<QueryNode>,
    },
    Table {
        name: String,
    },
    Filter {
        source: Box<QueryNode>,
        condition: String,
    },
    Join {
        left: Box<QueryNode>,
        right: Box<QueryNode>,
        condition: String,
    },
    Aggregate {
        source: Box<QueryNode>,
        function: String,
        column: String,
    },
}

// ===== Visitor Trait =====

/// Visitor trait for traversing query nodes
pub trait QueryVisitor {
    type Output;

    fn visit_select(&mut self, columns: &[String], from: &QueryNode) -> Self::Output;
    fn visit_table(&mut self, name: &str) -> Self::Output;
    fn visit_filter(&mut self, source: &QueryNode, condition: &str) -> Self::Output;
    fn visit_join(&mut self, left: &QueryNode, right: &QueryNode, condition: &str) -> Self::Output;
    fn visit_aggregate(&mut self, source: &QueryNode, function: &str, column: &str) -> Self::Output;
}

impl QueryNode {
    pub fn accept<V: QueryVisitor>(&self, visitor: &mut V) -> V::Output {
        match self {
            QueryNode::Select { columns, from } => visitor.visit_select(columns, from),
            QueryNode::Table { name } => visitor.visit_table(name),
            QueryNode::Filter { source, condition } => visitor.visit_filter(source, condition),
            QueryNode::Join { left, right, condition } => visitor.visit_join(left, right, condition),
            QueryNode::Aggregate { source, function, column } => visitor.visit_aggregate(source, function, column),
        }
    }
}

// ===== SQL Generator Visitor =====

pub struct SqlGenerator {
    indent_level: usize,
}

impl SqlGenerator {
    pub fn new() -> Self {
        Self { indent_level: 0 }
    }

    fn indent(&self) -> String {
        "  ".repeat(self.indent_level)
    }
}

impl QueryVisitor for SqlGenerator {
    type Output = String;

    fn visit_select(&mut self, columns: &[String], from: &QueryNode) -> String {
        let cols = columns.join(", ");
        self.indent_level += 1;
        let from_sql = from.accept(self);
        self.indent_level -= 1;

        format!("SELECT {}\n{}FROM {}", cols, self.indent(), from_sql)
    }

    fn visit_table(&mut self, name: &str) -> String {
        name.to_string()
    }

    fn visit_filter(&mut self, source: &QueryNode, condition: &str) -> String {
        self.indent_level += 1;
        let source_sql = source.accept(self);
        self.indent_level -= 1;

        format!("({})\n{}WHERE {}", source_sql, self.indent(), condition)
    }

    fn visit_join(&mut self, left: &QueryNode, right: &QueryNode, condition: &str) -> String {
        self.indent_level += 1;
        let left_sql = left.accept(self);
        let right_sql = right.accept(self);
        self.indent_level -= 1;

        format!("{}\n{}JOIN {}\n{}ON {}", left_sql, self.indent(), right_sql, self.indent(), condition)
    }

    fn visit_aggregate(&mut self, source: &QueryNode, function: &str, column: &str) -> String {
        self.indent_level += 1;
        let source_sql = source.accept(self);
        self.indent_level -= 1;

        format!("SELECT {}({})\n{}FROM {}", function, column, self.indent(), source_sql)
    }
}

// ===== Query Optimizer Visitor =====

pub struct QueryOptimizer {
    optimizations_applied: usize,
}

impl QueryOptimizer {
    pub fn new() -> Self {
        Self {
            optimizations_applied: 0,
        }
    }

    pub fn optimizations_count(&self) -> usize {
        self.optimizations_applied
    }
}

impl QueryVisitor for QueryOptimizer {
    type Output = QueryNode;

    fn visit_select(&mut self, columns: &[String], from: &QueryNode) -> QueryNode {
        let optimized_from = from.accept(self);

        // Optimization: remove duplicate columns
        let mut unique_columns: Vec<String> = Vec::new();
        for col in columns {
            if !unique_columns.contains(col) {
                unique_columns.push(col.clone());
            }
        }

        if unique_columns.len() < columns.len() {
            self.optimizations_applied += 1;
        }

        QueryNode::Select {
            columns: unique_columns,
            from: Box::new(optimized_from),
        }
    }

    fn visit_table(&mut self, name: &str) -> QueryNode {
        QueryNode::Table {
            name: name.to_string(),
        }
    }

    fn visit_filter(&mut self, source: &QueryNode, condition: &str) -> QueryNode {
        let optimized_source = source.accept(self);

        QueryNode::Filter {
            source: Box::new(optimized_source),
            condition: condition.to_string(),
        }
    }

    fn visit_join(&mut self, left: &QueryNode, right: &QueryNode, condition: &str) -> QueryNode {
        let optimized_left = left.accept(self);
        let optimized_right = right.accept(self);

        QueryNode::Join {
            left: Box::new(optimized_left),
            right: Box::new(optimized_right),
            condition: condition.to_string(),
        }
    }

    fn visit_aggregate(&mut self, source: &QueryNode, function: &str, column: &str) -> QueryNode {
        let optimized_source = source.accept(self);

        QueryNode::Aggregate {
            source: Box::new(optimized_source),
            function: function.to_string(),
            column: column.to_string(),
        }
    }
}

// ===== Query Validator Visitor =====

pub struct QueryValidator {
    errors: Vec<String>,
}

impl QueryValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl QueryVisitor for QueryValidator {
    type Output = ();

    fn visit_select(&mut self, columns: &[String], from: &QueryNode) {
        if columns.is_empty() {
            self.errors.push("SELECT must have at least one column".to_string());
        }

        from.accept(self);
    }

    fn visit_table(&mut self, name: &str) {
        if name.is_empty() {
            self.errors.push("Table name cannot be empty".to_string());
        }
    }

    fn visit_filter(&mut self, source: &QueryNode, condition: &str) {
        if condition.is_empty() {
            self.errors.push("Filter condition cannot be empty".to_string());
        }

        source.accept(self);
    }

    fn visit_join(&mut self, left: &QueryNode, right: &QueryNode, condition: &str) {
        if condition.is_empty() {
            self.errors.push("Join condition cannot be empty".to_string());
        }

        left.accept(self);
        right.accept(self);
    }

    fn visit_aggregate(&mut self, source: &QueryNode, function: &str, column: &str) {
        let valid_functions = ["COUNT", "SUM", "AVG", "MIN", "MAX"];
        if !valid_functions.contains(&function.to_uppercase().as_str()) {
            self.errors.push(format!("Invalid aggregate function: {}", function));
        }

        if column.is_empty() {
            self.errors.push("Aggregate column cannot be empty".to_string());
        }

        source.accept(self);
    }
}

// ===== Cost Estimator Visitor =====

pub struct CostEstimator {
    estimated_cost: f64,
}

impl CostEstimator {
    pub fn new() -> Self {
        Self {
            estimated_cost: 0.0,
        }
    }

    pub fn cost(&self) -> f64 {
        self.estimated_cost
    }
}

impl QueryVisitor for CostEstimator {
    type Output = ();

    fn visit_select(&mut self, columns: &[String], from: &QueryNode) {
        self.estimated_cost += columns.len() as f64 * 1.0;
        from.accept(self);
    }

    fn visit_table(&mut self, _name: &str) {
        self.estimated_cost += 10.0; // Base table scan cost
    }

    fn visit_filter(&mut self, source: &QueryNode, _condition: &str) {
        self.estimated_cost += 5.0; // Filter cost
        source.accept(self);
    }

    fn visit_join(&mut self, left: &QueryNode, right: &QueryNode, _condition: &str) {
        self.estimated_cost += 50.0; // Join is expensive
        left.accept(self);
        right.accept(self);
    }

    fn visit_aggregate(&mut self, source: &QueryNode, _function: &str, _column: &str) {
        self.estimated_cost += 20.0; // Aggregation cost
        source.accept(self);
    }
}

// ===== Double Dispatch Pattern =====

/// Demonstrates double dispatch for operations on different types
pub trait Shape {
    fn accept<V: ShapeVisitor>(&self, visitor: &mut V) -> V::Output;
}

pub trait ShapeVisitor {
    type Output;

    fn visit_circle(&mut self, radius: f64) -> Self::Output;
    fn visit_rectangle(&mut self, width: f64, height: f64) -> Self::Output;
    fn visit_triangle(&mut self, base: f64, height: f64) -> Self::Output;
}

pub struct Circle {
    pub radius: f64,
}

impl Shape for Circle {
    fn accept<V: ShapeVisitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_circle(self.radius)
    }
}

pub struct Rectangle {
    pub width: f64,
    pub height: f64,
}

impl Shape for Rectangle {
    fn accept<V: ShapeVisitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_rectangle(self.width, self.height)
    }
}

pub struct Triangle {
    pub base: f64,
    pub height: f64,
}

impl Shape for Triangle {
    fn accept<V: ShapeVisitor>(&self, visitor: &mut V) -> V::Output {
        visitor.visit_triangle(self.base, self.height)
    }
}

/// Area calculator visitor
pub struct AreaCalculator;

impl ShapeVisitor for AreaCalculator {
    type Output = f64;

    fn visit_circle(&mut self, radius: f64) -> f64 {
        std::f64::consts::PI * radius * radius
    }

    fn visit_rectangle(&mut self, width: f64, height: f64) -> f64 {
        width * height
    }

    fn visit_triangle(&mut self, base: f64, height: f64) -> f64 {
        0.5 * base * height
    }
}

/// Perimeter calculator visitor
pub struct PerimeterCalculator;

impl ShapeVisitor for PerimeterCalculator {
    type Output = f64;

    fn visit_circle(&mut self, radius: f64) -> f64 {
        2.0 * std::f64::consts::PI * radius
    }

    fn visit_rectangle(&mut self, width: f64, height: f64) -> f64 {
        2.0 * (width + height)
    }

    fn visit_triangle(&mut self, base: f64, height: f64) -> f64 {
        // Simplified: assumes equilateral-ish triangle
        base + 2.0 * ((base / 2.0).powi(2) + height.powi(2)).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_generator() {
        let query = QueryNode::Select {
            columns: vec!["id".to_string(), "name".to_string()],
            from: Box::new(QueryNode::Table {
                name: "users".to_string(),
            }),
        };

        let mut generator = SqlGenerator::new();
        let sql = query.accept(&mut generator);

        assert!(sql.contains("SELECT id, name"));
        assert!(sql.contains("FROM users"));
    }

    #[test]
    fn test_query_optimizer() {
        let query = QueryNode::Select {
            columns: vec!["id".to_string(), "name".to_string(), "id".to_string()],
            from: Box::new(QueryNode::Table {
                name: "users".to_string(),
            }),
        };

        let mut optimizer = QueryOptimizer::new();
        let optimized = query.accept(&mut optimizer);

        assert_eq!(optimizer.optimizations_count(), 1);

        if let QueryNode::Select { columns, .. } = optimized {
            assert_eq!(columns.len(), 2); // Duplicate "id" removed
        }
    }

    #[test]
    fn test_query_validator() {
        let invalid_query = QueryNode::Select {
            columns: vec![],
            from: Box::new(QueryNode::Table {
                name: "".to_string(),
            }),
        };

        let mut validator = QueryValidator::new();
        invalid_query.accept(&mut validator);

        assert!(!validator.is_valid());
        assert!(validator.errors().len() > 0);
    }

    #[test]
    fn test_cost_estimator() {
        let query = QueryNode::Join {
            left: Box::new(QueryNode::Table {
                name: "users".to_string(),
            }),
            right: Box::new(QueryNode::Table {
                name: "orders".to_string(),
            }),
            condition: "users.id = orders.user_id".to_string(),
        };

        let mut estimator = CostEstimator::new();
        query.accept(&mut estimator);

        assert!(estimator.cost() > 0.0);
    }

    #[test]
    fn test_shape_visitors() {
        let circle = Circle { radius: 5.0 };
        let rectangle = Rectangle { width: 4.0, height: 6.0 };
        let triangle = Triangle { base: 3.0, height: 4.0 };

        let mut area_calc = AreaCalculator;
        assert!((circle.accept(&mut area_calc) - 78.539).abs() < 0.01);
        assert_eq!(rectangle.accept(&mut area_calc), 24.0);
        assert_eq!(triangle.accept(&mut area_calc), 6.0);

        let mut perim_calc = PerimeterCalculator;
        assert!((circle.accept(&mut perim_calc) - 31.415).abs() < 0.01);
        assert_eq!(rectangle.accept(&mut perim_calc), 20.0);
    }
}
