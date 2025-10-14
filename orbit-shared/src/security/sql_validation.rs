//! SQL Injection Prevention and Query Validation
//!
//! Provides comprehensive SQL injection detection and prevention:
//! - Pattern-based detection
//! - Parameterized query enforcement
//! - Query complexity analysis
//! - Dangerous operation blocking

use crate::exception::{OrbitError, OrbitResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// SQL injection detector
pub struct SqlInjectionDetector {
    dangerous_patterns: Vec<Regex>,
    blocked_keywords: HashSet<String>,
    max_query_length: usize,
    allow_dynamic_sql: bool,
}

impl SqlInjectionDetector {
    /// Create a new SQL injection detector
    pub fn new() -> Self {
        let dangerous_patterns = vec![
            // SQL injection patterns
            Regex::new(r"(?i)(\bor\b|\band\b)\s+\d+\s*=\s*\d+").unwrap(),
            Regex::new(r"(?i)\b(union|union\s+all)\b.*\bselect\b").unwrap(),
            Regex::new(r"(?i);\s*(drop|delete|truncate|alter)\s+").unwrap(),
            Regex::new(r"(?i)'\s*(or|and)\s+'.*'='").unwrap(),
            Regex::new(r"(?i)--").unwrap(),
            Regex::new(r"(?i)/\*.*\*/").unwrap(),
            Regex::new(r"(?i)xp_cmdshell").unwrap(),
            Regex::new(r"(?i)exec\s*\(").unwrap(),
            Regex::new(r"(?i)execute\s+immediate").unwrap(),
        ];

        let blocked_keywords = HashSet::from([
            "xp_cmdshell".to_string(),
            "sp_executesql".to_string(),
            "exec(".to_string(),
            "execute(".to_string(),
        ]);

        Self {
            dangerous_patterns,
            blocked_keywords,
            max_query_length: 10000,
            allow_dynamic_sql: false,
        }
    }

    /// Create with custom configuration
    pub fn with_config(max_query_length: usize, allow_dynamic_sql: bool) -> Self {
        let mut detector = Self::new();
        detector.max_query_length = max_query_length;
        detector.allow_dynamic_sql = allow_dynamic_sql;
        detector
    }

    /// Validate a SQL query
    pub fn validate(&self, query: &str) -> OrbitResult<()> {
        // Check query length
        if query.len() > self.max_query_length {
            return Err(OrbitError::internal("Query exceeds maximum length"));
        }

        // Check for dangerous patterns
        for pattern in &self.dangerous_patterns {
            if pattern.is_match(query) {
                return Err(OrbitError::internal(format!(
                    "Potential SQL injection detected: {}",
                    pattern.as_str()
                )));
            }
        }

        // Check for blocked keywords
        let query_lower = query.to_lowercase();
        for keyword in &self.blocked_keywords {
            if query_lower.contains(keyword) {
                return Err(OrbitError::internal(format!(
                    "Blocked keyword detected: {keyword}"
                )));
            }
        }

        // Check for unbalanced quotes
        self.check_balanced_quotes(query)?;

        // Check for excessive comments
        self.check_comments(query)?;

        Ok(())
    }

    /// Check for balanced quotes
    fn check_balanced_quotes(&self, query: &str) -> OrbitResult<()> {
        let single_quotes = query.chars().filter(|&c| c == '\'').count();
        let double_quotes = query.chars().filter(|&c| c == '"').count();

        if single_quotes % 2 != 0 {
            return Err(OrbitError::internal("Unbalanced single quotes"));
        }

        if double_quotes % 2 != 0 {
            return Err(OrbitError::internal("Unbalanced double quotes"));
        }

        Ok(())
    }

    /// Check for excessive comments
    fn check_comments(&self, query: &str) -> OrbitResult<()> {
        let comment_count = query.matches("--").count() + query.matches("/*").count();

        if comment_count > 5 {
            return Err(OrbitError::internal("Excessive comments detected"));
        }

        Ok(())
    }

    /// Sanitize input
    pub fn sanitize(&self, input: &str) -> String {
        input
            .replace('\'', "''")
            .replace(';', "")
            .replace("--", "")
            .replace("/*", "")
            .replace("*/", "")
    }
}

impl Default for SqlInjectionDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Query validator
pub struct QueryValidator {
    injection_detector: SqlInjectionDetector,
    max_complexity: usize,
    allowed_tables: Option<HashSet<String>>,
    blocked_operations: HashSet<String>,
}

impl QueryValidator {
    /// Create a new query validator
    pub fn new() -> Self {
        let blocked_operations = HashSet::from([
            "DROP".to_string(),
            "TRUNCATE".to_string(),
            "ALTER".to_string(),
        ]);

        Self {
            injection_detector: SqlInjectionDetector::new(),
            max_complexity: 100,
            allowed_tables: None,
            blocked_operations,
        }
    }

    /// Set allowed tables
    pub fn with_allowed_tables(mut self, tables: HashSet<String>) -> Self {
        self.allowed_tables = Some(tables);
        self
    }

    /// Add blocked operation
    pub fn add_blocked_operation(&mut self, operation: String) {
        self.blocked_operations.insert(operation.to_uppercase());
    }

    /// Validate a query
    pub fn validate(&self, query: &str) -> OrbitResult<ValidationResult> {
        // Check for SQL injection
        self.injection_detector.validate(query)?;

        // Check for blocked operations
        let query_upper = query.to_uppercase();
        for operation in &self.blocked_operations {
            if query_upper.contains(operation) {
                return Err(OrbitError::internal(format!(
                    "Blocked operation detected: {operation}"
                )));
            }
        }

        // Estimate query complexity
        let complexity = self.estimate_complexity(query);
        if complexity > self.max_complexity {
            return Err(OrbitError::internal("Query complexity exceeds limit"));
        }

        // Check table access
        if let Some(ref allowed_tables) = self.allowed_tables {
            self.check_table_access(query, allowed_tables)?;
        }

        Ok(ValidationResult {
            valid: true,
            complexity,
            warnings: Vec::new(),
        })
    }

    /// Estimate query complexity
    fn estimate_complexity(&self, query: &str) -> usize {
        let mut complexity = 0;

        // Count joins
        complexity += query.matches("JOIN").count() * 10;

        // Count subqueries
        complexity += query.matches("SELECT").count() * 5;

        // Count WHERE conditions
        complexity += query.matches("WHERE").count() * 3;

        // Count ORDER BY
        complexity += query.matches("ORDER BY").count() * 2;

        // Count GROUP BY
        complexity += query.matches("GROUP BY").count() * 3;

        complexity
    }

    /// Check table access
    fn check_table_access(&self, query: &str, allowed_tables: &HashSet<String>) -> OrbitResult<()> {
        // Simple pattern matching for table names after FROM and JOIN
        // In production, this would use a proper SQL parser
        let query_upper = query.to_uppercase();

        // Extract words after FROM and JOIN keywords
        for keyword in &["FROM", "JOIN"] {
            if let Some(pos) = query_upper.find(keyword) {
                let after_keyword = &query[pos + keyword.len()..];
                let words: Vec<&str> = after_keyword.split_whitespace().collect();

                if !words.is_empty() {
                    let table_name = words[0].trim_matches(|c| c == '(' || c == ')');
                    if !allowed_tables.contains(&table_name.to_uppercase()) {
                        return Err(OrbitError::internal(format!(
                            "Access to table '{table_name}' is not allowed"
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for QueryValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Query validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub complexity: usize,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_injection_detection() {
        let detector = SqlInjectionDetector::new();

        // Valid query should pass
        assert!(detector
            .validate("SELECT * FROM users WHERE id = 1")
            .is_ok());

        // SQL injection attempts should fail
        assert!(detector
            .validate("SELECT * FROM users WHERE id = 1 OR 1=1")
            .is_err());
        assert!(detector
            .validate("SELECT * FROM users; DROP TABLE users;")
            .is_err());
        assert!(detector
            .validate("SELECT * FROM users WHERE name = '' OR '1'='1'")
            .is_err());
        assert!(detector.validate("SELECT * FROM users -- comment").is_err());
    }

    #[test]
    fn test_balanced_quotes() {
        let detector = SqlInjectionDetector::new();

        // Balanced quotes should pass
        assert!(detector
            .validate("SELECT * FROM users WHERE name = 'John'")
            .is_ok());

        // Unbalanced quotes should fail
        assert!(detector
            .validate("SELECT * FROM users WHERE name = 'John")
            .is_err());
    }

    #[test]
    fn test_sanitize() {
        let detector = SqlInjectionDetector::new();

        let input = "O'Reilly; DROP TABLE users; --";
        let sanitized = detector.sanitize(input);

        assert!(!sanitized.contains(';'));
        assert!(!sanitized.contains("--"));
        assert!(sanitized.contains("''"));
    }

    #[test]
    fn test_query_validator() {
        let validator = QueryValidator::new();

        // Normal SELECT should pass
        let result = validator.validate("SELECT * FROM users WHERE id = 1");
        assert!(result.is_ok());

        // DROP should be blocked
        let result = validator.validate("DROP TABLE users");
        assert!(result.is_err());

        // Complex query should be detected
        let complex_query = "SELECT * FROM users \
                            JOIN orders ON users.id = orders.user_id \
                            WHERE users.active = true \
                            ORDER BY users.created_at";
        let result = validator.validate(complex_query);
        assert!(result.is_ok());
        assert!(result.unwrap().complexity > 0);
    }

    #[test]
    fn test_allowed_tables() {
        let mut allowed_tables = HashSet::new();
        allowed_tables.insert("USERS".to_string());
        allowed_tables.insert("ORDERS".to_string());

        let validator = QueryValidator::new().with_allowed_tables(allowed_tables);

        // Query to allowed table should pass
        assert!(validator.validate("SELECT * FROM users").is_ok());

        // Query to blocked table should fail
        assert!(validator.validate("SELECT * FROM sensitive_data").is_err());
    }
}
