//! Basic SQL DML Tests
//! 
//! Quick verification that DML parsing and execution work correctly

#[cfg(test)]
mod tests {
    use crate::postgres_wire::sql::{SqlEngine, SqlParser};

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
    fn test_where_expressions() {
        let mut parser = SqlParser::new();
        
        // Test various WHERE expressions
        let sql_queries = vec![
            "SELECT * FROM users WHERE id = 1",
            "SELECT * FROM users WHERE name LIKE 'John%'",
            "SELECT * FROM users WHERE age > 18 AND status = 'active'",
            "SELECT * FROM users WHERE id IN (1, 2, 3)",
            "SELECT * FROM users WHERE email IS NOT NULL",
        ];
        
        for sql in sql_queries {
            let result = parser.parse(sql);
            assert!(result.is_ok(), "Failed to parse: {} - Error: {:?}", sql, result);
        }
    }
}