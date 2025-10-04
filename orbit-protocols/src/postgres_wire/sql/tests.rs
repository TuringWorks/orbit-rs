//! Basic SQL DML Tests
//! 
//! Quick verification that DML parsing and execution work correctly

#[cfg(test)]
mod tests {
use crate::postgres_wire::sql::lexer::*;
use crate::postgres_wire::sql::parser::select::SelectParser;
use crate::postgres_wire::sql::ast::*;
use crate::postgres_wire::SqlEngine;

    #[tokio::test]
    async fn test_select_parsing() {
        let mut engine = SqlEngine::new();
        
        // Test basic SELECT
        let result = engine.parse("SELECT * FROM users WHERE id = 1 ORDER BY name LIMIT 10");
        assert!(result.is_ok(), "Failed to parse basic SELECT: {:?}", result);
        
        // Test SELECT with specific columns
        let result = engine.parse("SELECT id, name, email FROM users WHERE age > 18");
        assert!(result.is_ok(), "Failed to parse column-specific SELECT: {:?}", result);
    }
    
    #[tokio::test]
    async fn test_insert_parsing() {
        let mut engine = SqlEngine::new();
        
        // Test basic INSERT
        let result = engine.parse("INSERT INTO users (name, email) VALUES ('John', 'john@example.com')");
        assert!(result.is_ok(), "Failed to parse basic INSERT: {:?}", result);
        
        // Test multiple values INSERT
        let result = engine.parse("INSERT INTO users (name, age) VALUES ('Alice', 25), ('Bob', 30)");
        assert!(result.is_ok(), "Failed to parse multi-value INSERT: {:?}", result);
    }
    
    #[tokio::test]
    async fn test_update_parsing() {
        let mut engine = SqlEngine::new();
        
        // Test basic UPDATE
        let result = engine.parse("UPDATE users SET name = 'Jane' WHERE id = 1");
        assert!(result.is_ok(), "Failed to parse basic UPDATE: {:?}", result);
        
        // Test multiple column UPDATE
        let result = engine.parse("UPDATE users SET name = 'Jane', age = 25, email = 'jane@example.com' WHERE id = 1");
        assert!(result.is_ok(), "Failed to parse multi-column UPDATE: {:?}", result);
    }
    
    #[tokio::test]
    async fn test_delete_parsing() {
        let mut engine = SqlEngine::new();
        
        // Test basic DELETE
        let result = engine.parse("DELETE FROM users WHERE id = 1");
        assert!(result.is_ok(), "Failed to parse basic DELETE: {:?}", result);
        
        // Test DELETE without WHERE
        let result = engine.parse("DELETE FROM users");
        assert!(result.is_ok(), "Failed to parse DELETE without WHERE: {:?}", result);
    }
    
    #[tokio::test]
    async fn test_dml_execution() {
        let mut engine = SqlEngine::new();
        
        // Test SELECT execution
        let result = engine.execute("SELECT * FROM test_table").await;
        assert!(result.is_ok(), "Failed to execute SELECT: {:?}", result);
        
        // Test INSERT execution
        let result = engine.execute("INSERT INTO test_table (name) VALUES ('test')").await;
        assert!(result.is_ok(), "Failed to execute INSERT: {:?}", result);
        
        // Test UPDATE execution
        let result = engine.execute("UPDATE test_table SET name = 'updated' WHERE id = 1").await;
        assert!(result.is_ok(), "Failed to execute UPDATE: {:?}", result);
        
        // Test DELETE execution
        let result = engine.execute("DELETE FROM test_table WHERE id = 1").await;
        assert!(result.is_ok(), "Failed to execute DELETE: {:?}", result);
    }
    
    #[test]
    fn test_basic_select_parsing() {
        let sql = "SELECT id, name FROM users WHERE age > 21 LIMIT 10";
        let mut lexer = Lexer::new(sql);
        let tokens = lexer.tokenize();

        let mut parser = SelectParser::new();
        let mut pos = 0;
        let select = parser.parse_select(&tokens, &mut pos).expect("parse select");

        // Verify basic structure
        assert_eq!(select.select_list.len(), 2);
        assert!(select.from_clause.is_some());
        assert!(select.where_clause.is_some());
        assert!(select.limit.is_some());
    }
}