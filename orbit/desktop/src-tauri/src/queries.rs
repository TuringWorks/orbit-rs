use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use tracing::{error, info, warn};

use crate::connections::{ConnectionManager, ConnectionType, ConnectionError, Connection};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryRequest {
    pub connection_id: String,
    pub query: String,
    pub query_type: QueryType,
    pub timeout: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub success: bool,
    pub data: Option<QueryResultData>,
    pub error: Option<String>,
    pub execution_time: f64,
    pub rows_affected: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResultData {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum QueryType {
    SQL,
    OrbitQL,
    Redis,
    MySQL,
    CQL,
    Cypher,
    AQL,
}

impl Default for QueryResult {
    fn default() -> Self {
        QueryResult {
            success: false,
            data: None,
            error: None,
            execution_time: 0.0,
            rows_affected: None,
        }
    }
}

// Query executor for handling database queries
#[derive(Default)]
pub struct QueryExecutor {
    history: Vec<QueryRequest>,
}

impl QueryExecutor {
    pub fn new() -> Self {
        QueryExecutor {
            history: Vec::new(),
        }
    }
    
    pub async fn execute_query(
        &mut self,
        request: QueryRequest,
        connection_manager: &ConnectionManager,
    ) -> Result<QueryResult, String> {
        let start_time = Instant::now();
        
        // Get connection info
        let connection = connection_manager
            .get_connection(&request.connection_id)
            .await
            .ok_or_else(|| "Connection not found".to_string())?;
        
        let result = match connection.info.connection_type {
            ConnectionType::PostgreSQL => {
                self.execute_postgresql_query(&request, &connection).await
            }
            ConnectionType::OrbitQL => {
                self.execute_orbitql_query(&request, &connection).await
            }
            ConnectionType::Redis => {
                self.execute_redis_query(&request, &connection).await
            }
            ConnectionType::MySQL => {
                self.execute_mysql_query(&request, &connection).await
            }
            ConnectionType::CQL => {
                self.execute_cql_query(&request, &connection).await
            }
            ConnectionType::Cypher => {
                self.execute_cypher_query(&request, &connection).await
            }
            ConnectionType::AQL => {
                self.execute_aql_query(&request, &connection).await
            }
        };
        
        let execution_time = start_time.elapsed().as_secs_f64() * 1000.0; // Convert to milliseconds
        
        // Store in history
        self.history.push(request.clone());
        if self.history.len() > 1000 {
            self.history.remove(0);
        }
        
        match result {
            Ok(mut res) => {
                res.execution_time = execution_time;
                Ok(res)
            }
            Err(e) => {
                Ok(QueryResult {
                    success: false,
                    data: None,
                    error: Some(e),
                    execution_time,
                    rows_affected: None,
                })
            }
        }
    }
    
    async fn execute_postgresql_query(
        &self,
        request: &QueryRequest,
        connection: &crate::connections::Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::PostgreSQLConnection;
        
        // Create connection
        let pg_conn = PostgreSQLConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Execute query
        let rows = pg_conn.execute_query(&request.query).await
            .map_err(|e| format!("Query execution failed: {}", e))?;
        
        if rows.is_empty() {
            return Ok(QueryResult {
                success: true,
                data: Some(QueryResultData {
                    columns: vec![],
                    rows: vec![],
                }),
                error: None,
                execution_time: 0.0,
                rows_affected: Some(0),
            });
        }
        
        // Extract column information from first row
        let columns: Vec<ColumnInfo> = rows[0]
            .columns()
            .iter()
            .map(|col| ColumnInfo {
                name: col.name().to_string(),
                column_type: format!("{:?}", col.type_()),
            })
            .collect();
        
        // Convert rows to JSON
        let mut result_rows = Vec::new();
        for row in rows {
            let mut row_map = HashMap::new();
            for col in &columns {
                // Try to extract value based on type
                let value = if col.column_type.contains("Int4") || col.column_type.contains("Int8") {
                    // Try i64 first, then i32
                    row.try_get::<_, i64>(col.name.as_str())
                        .map(|v| serde_json::Value::Number(v.into()))
                        .or_else(|_| row.try_get::<_, i32>(col.name.as_str())
                            .map(|v| serde_json::Value::Number(v.into())))
                        .unwrap_or(serde_json::Value::Null)
                } else if col.column_type.contains("Float4") || col.column_type.contains("Float8") {
                    row.try_get::<_, f64>(col.name.as_str())
                        .map(|v| serde_json::Number::from_f64(v)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null)
                } else if col.column_type.contains("Bool") {
                    row.try_get::<_, bool>(col.name.as_str())
                        .map(serde_json::Value::Bool)
                        .unwrap_or(serde_json::Value::Null)
                } else {
                    // Default to string
                    row.try_get::<_, String>(col.name.as_str())
                        .map(serde_json::Value::String)
                        .unwrap_or(serde_json::Value::Null)
                };
                row_map.insert(col.name.clone(), value);
            }
            result_rows.push(row_map);
        }
        
        Ok(QueryResult {
            success: true,
            data: Some(QueryResultData {
                columns,
                rows: result_rows,
            }),
            error: None,
            execution_time: 0.0,
            rows_affected: Some(result_rows.len() as u64),
        })
    }
    
    async fn execute_orbitql_query(
        &self,
        request: &QueryRequest,
        connection: &crate::connections::Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::OrbitQLConnection;
        
        // Create connection
        let orbit_conn = OrbitQLConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Execute query
        let result = orbit_conn.execute_orbitql(&request.query).await
            .map_err(|e| format!("Query execution failed: {}", e))?;
        
        // Parse result
        if let Some(data) = result.get("data") {
            if let Some(rows) = data.as_array() {
                let columns = if let Some(first_row) = rows.first().and_then(|r| r.as_object()) {
                    first_row.keys().map(|k| ColumnInfo {
                        name: k.clone(),
                        column_type: "unknown".to_string(),
                    }).collect()
                } else {
                    vec![]
                };
                
                let result_rows: Vec<HashMap<String, serde_json::Value>> = rows
                    .iter()
                    .filter_map(|r| r.as_object().map(|o| o.clone().into_iter().collect()))
                    .collect();
                
                Ok(QueryResult {
                    success: true,
                    data: Some(QueryResultData {
                        columns,
                        rows: result_rows,
                    }),
                    error: None,
                    execution_time: 0.0,
                    rows_affected: Some(rows.len() as u64),
                })
            } else {
                Ok(QueryResult {
                    success: true,
                    data: Some(QueryResultData {
                        columns: vec![],
                        rows: vec![],
                    }),
                    error: None,
                    execution_time: 0.0,
                    rows_affected: Some(0),
                })
            }
        } else {
            Err("Invalid response format".to_string())
        }
    }
    
    async fn execute_redis_query(
        &self,
        request: &QueryRequest,
        connection: &crate::connections::Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::RedisConnection;
        
        // Create connection
        let redis_conn = RedisConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Parse Redis command
        let parts: Vec<&str> = request.query.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Err("Empty Redis command".to_string());
        }
        
        let cmd = parts[0].to_uppercase();
        let args: Vec<&str> = parts[1..].to_vec();
        
        // Execute command
        let value = redis_conn.execute_redis_command(&cmd, &args).await
            .map_err(|e| format!("Redis command failed: {}", e))?;
        
        // Convert Redis value to JSON
        let json_value = match value {
            redis::Value::Nil => serde_json::Value::Null,
            redis::Value::Int(i) => serde_json::Value::Number(i.into()),
            redis::Value::Data(data) => {
                String::from_utf8(data)
                    .map(serde_json::Value::String)
                    .unwrap_or(serde_json::Value::Null)
            }
            redis::Value::Bulk(bulk) => {
                serde_json::Value::Array(
                    bulk.into_iter()
                        .map(|v| match v {
                            redis::Value::Int(i) => serde_json::Value::Number(i.into()),
                            redis::Value::Data(d) => {
                                String::from_utf8(d)
                                    .map(serde_json::Value::String)
                                    .unwrap_or(serde_json::Value::Null)
                            }
                            _ => serde_json::Value::String(format!("{:?}", v)),
                        })
                        .collect()
                )
            }
            redis::Value::Status(s) => serde_json::Value::String(s),
            redis::Value::Okay => serde_json::Value::String("OK".to_string()),
        };
        
        let mut row = HashMap::new();
        row.insert("result".to_string(), json_value);
        
        Ok(QueryResult {
            success: true,
            data: Some(QueryResultData {
                columns: vec![ColumnInfo {
                    name: "result".to_string(),
                    column_type: "redis_value".to_string(),
                }],
                rows: vec![row],
            }),
            error: None,
            execution_time: 0.0,
            rows_affected: Some(1),
        })
    }
    
    async fn execute_mysql_query(
        &self,
        request: &QueryRequest,
        connection: &Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::MySQLConnection;
        
        // Create connection
        let mysql_conn = MySQLConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Execute query
        let rows = mysql_conn.execute_query(&request.query).await
            .map_err(|e| format!("Query execution failed: {}", e))?;
        
        if rows.is_empty() {
            return Ok(QueryResult {
                success: true,
                data: Some(QueryResultData {
                    columns: vec![],
                    rows: vec![],
                }),
                error: None,
                execution_time: 0.0,
                rows_affected: Some(0),
            });
        }
        
        // Extract column information and convert rows
        // MySQL rows need to be converted to JSON format
        let mut result_rows = Vec::new();
        let mut columns = Vec::new();
        
        // Get column info from first row
        if let Some(first_row) = rows.first() {
            // MySQL rows have columns accessible via index
            // We'll need to extract column names from the row structure
            for i in 0..first_row.len() {
                columns.push(ColumnInfo {
                    name: format!("column_{}", i),
                    column_type: "unknown".to_string(),
                });
            }
        }
        
        // Convert rows to JSON
        for row in rows {
            let mut row_map = HashMap::new();
            for (i, col) in columns.iter().enumerate() {
                // Try to extract value as string first
                let value = if let Ok(val) = row.get::<String, _>(i) {
                    serde_json::Value::String(val)
                } else if let Ok(val) = row.get::<i64, _>(i) {
                    serde_json::Value::Number(val.into())
                } else if let Ok(val) = row.get::<f64, _>(i) {
                    serde_json::Number::from_f64(val)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                } else if let Ok(val) = row.get::<bool, _>(i) {
                    serde_json::Value::Bool(val)
                } else {
                    serde_json::Value::Null
                };
                row_map.insert(col.name.clone(), value);
            }
            result_rows.push(row_map);
        }
        
        Ok(QueryResult {
            success: true,
            data: Some(QueryResultData {
                columns,
                rows: result_rows,
            }),
            error: None,
            execution_time: 0.0,
            rows_affected: Some(result_rows.len() as u64),
        })
    }
    
    async fn execute_cql_query(
        &self,
        request: &QueryRequest,
        connection: &Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::CQLConnection;
        
        // Create connection
        let cql_conn = CQLConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Execute query
        let result = cql_conn.execute_cql(&request.query).await
            .map_err(|e| format!("Query execution failed: {}", e))?;
        
        // Parse CQL result
        if let Some(data) = result.get("data") {
            if let Some(rows) = data.as_array() {
                let columns = if let Some(first_row) = rows.first().and_then(|r| r.as_object()) {
                    first_row.keys().map(|k| ColumnInfo {
                        name: k.clone(),
                        column_type: "unknown".to_string(),
                    }).collect()
                } else {
                    vec![]
                };
                
                let result_rows: Vec<HashMap<String, serde_json::Value>> = rows
                    .iter()
                    .filter_map(|r| r.as_object().map(|o| o.clone().into_iter().collect()))
                    .collect();
                
                Ok(QueryResult {
                    success: true,
                    data: Some(QueryResultData {
                        columns,
                        rows: result_rows,
                    }),
                    error: None,
                    execution_time: 0.0,
                    rows_affected: Some(rows.len() as u64),
                })
            } else {
                Ok(QueryResult {
                    success: true,
                    data: Some(QueryResultData {
                        columns: vec![],
                        rows: vec![],
                    }),
                    error: None,
                    execution_time: 0.0,
                    rows_affected: Some(0),
                })
            }
        } else {
            Err("Invalid CQL response format".to_string())
        }
    }
    
    async fn execute_cypher_query(
        &self,
        request: &QueryRequest,
        connection: &Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::CypherConnection;
        
        // Create connection
        let cypher_conn = CypherConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Execute query
        let result = cypher_conn.execute_cypher(&request.query).await
            .map_err(|e| format!("Query execution failed: {}", e))?;
        
        // Parse Cypher result
        if let Some(results) = result.get("results").and_then(|r| r.as_array()) {
            if let Some(first_result) = results.first() {
                if let Some(data) = first_result.get("data").and_then(|d| d.as_array()) {
                    let mut result_rows = Vec::new();
                    let mut columns = Vec::new();
                    
                    for row_data in data {
                        if let Some(row) = row_data.get("row").and_then(|r| r.as_array()) {
                            let mut row_map = HashMap::new();
                            for (i, value) in row.iter().enumerate() {
                                let col_name = format!("column_{}", i);
                                if !columns.iter().any(|c: &ColumnInfo| c.name == col_name) {
                                    columns.push(ColumnInfo {
                                        name: col_name.clone(),
                                        column_type: "unknown".to_string(),
                                    });
                                }
                                row_map.insert(col_name, value.clone());
                            }
                            result_rows.push(row_map);
                        }
                    }
                    
                    Ok(QueryResult {
                        success: true,
                        data: Some(QueryResultData {
                            columns,
                            rows: result_rows,
                        }),
                        error: None,
                        execution_time: 0.0,
                        rows_affected: Some(result_rows.len() as u64),
                    })
                } else {
                    Ok(QueryResult {
                        success: true,
                        data: Some(QueryResultData {
                            columns: vec![],
                            rows: vec![],
                        }),
                        error: None,
                        execution_time: 0.0,
                        rows_affected: Some(0),
                    })
                }
            } else {
                Ok(QueryResult {
                    success: true,
                    data: Some(QueryResultData {
                        columns: vec![],
                        rows: vec![],
                    }),
                    error: None,
                    execution_time: 0.0,
                    rows_affected: Some(0),
                })
            }
        } else {
            Err("Invalid Cypher response format".to_string())
        }
    }
    
    async fn execute_aql_query(
        &self,
        request: &QueryRequest,
        connection: &Connection,
    ) -> Result<QueryResult, String> {
        use crate::connections::AQLConnection;
        
        // Create connection
        let aql_conn = AQLConnection::new(&connection.info).await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        // Execute query
        let result = aql_conn.execute_aql(&request.query).await
            .map_err(|e| format!("Query execution failed: {}", e))?;
        
        // Parse AQL result
        if let Some(result_array) = result.get("result").and_then(|r| r.as_array()) {
            let mut result_rows = Vec::new();
            let mut columns = Vec::new();
            
            for (idx, row_value) in result_array.iter().enumerate() {
                if let Some(row_obj) = row_value.as_object() {
                    let mut row_map = HashMap::new();
                    for (key, value) in row_obj {
                        if !columns.iter().any(|c: &ColumnInfo| c.name == key) {
                            columns.push(ColumnInfo {
                                name: key.clone(),
                                column_type: "unknown".to_string(),
                            });
                        }
                        row_map.insert(key.clone(), value.clone());
                    }
                    result_rows.push(row_map);
                } else {
                    // Single value result
                    let col_name = "result".to_string();
                    if idx == 0 && columns.is_empty() {
                        columns.push(ColumnInfo {
                            name: col_name.clone(),
                            column_type: "unknown".to_string(),
                        });
                    }
                    let mut row_map = HashMap::new();
                    row_map.insert(col_name, row_value.clone());
                    result_rows.push(row_map);
                }
            }
            
            Ok(QueryResult {
                success: true,
                data: Some(QueryResultData {
                    columns,
                    rows: result_rows,
                }),
                error: None,
                execution_time: 0.0,
                rows_affected: Some(result_rows.len() as u64),
            })
        } else {
            Ok(QueryResult {
                success: true,
                data: Some(QueryResultData {
                    columns: vec![],
                    rows: vec![],
                }),
                error: None,
                execution_time: 0.0,
                rows_affected: Some(0),
            })
        }
    }
    
    pub async fn explain_query(
        &mut self,
        request: QueryRequest,
        connection_manager: &ConnectionManager,
    ) -> Result<QueryResult, String> {
        // For PostgreSQL, prepend EXPLAIN ANALYZE
        let connection = connection_manager
            .get_connection(&request.connection_id)
            .await
            .ok_or_else(|| "Connection not found".to_string())?;
        
        match connection.info.connection_type {
            ConnectionType::PostgreSQL | ConnectionType::MySQL => {
                let explain_query = format!("EXPLAIN ANALYZE {}", request.query);
                let mut explain_request = request;
                explain_request.query = explain_query;
                self.execute_query(explain_request, connection_manager).await
            }
            _ => {
                Ok(QueryResult {
                    success: false,
                    data: None,
                    error: Some("EXPLAIN not supported for this connection type".to_string()),
                    execution_time: 0.0,
                    rows_affected: None,
                })
            }
        }
    }
    
    pub async fn get_history(&self, connection_id: &str, limit: usize) -> Result<Vec<QueryRequest>, String> {
        let filtered: Vec<QueryRequest> = self.history
            .iter()
            .filter(|q| q.connection_id == connection_id)
            .rev()
            .take(limit)
            .cloned()
            .collect();
        
        Ok(filtered)
    }
}
