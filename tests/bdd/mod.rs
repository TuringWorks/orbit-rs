//! Behavior-Driven Development (BDD) tests for Orbit-RS
//!
//! This module contains cucumber-based BDD tests that validate key user scenarios
//! in natural language format. These tests ensure that the system behaves correctly
//! from an end-user perspective.

use cucumber::{given, then, when, World};
use std::sync::Arc;
use tokio::sync::RwLock;

/// World object that maintains test state between scenario steps
#[derive(Debug, Default, World)]
pub struct OrbitWorld {
    /// SQL executor for database operations
    pub sql_executor: Option<orbit_protocols::postgres_wire::sql::SqlExecutor>,
    /// Vector store for similarity search operations
    pub vector_store: Option<Arc<RwLock<orbit_protocols::vector_store::VectorIndex>>>,
    /// Last execution result for assertions
    pub last_result: Option<serde_json::Value>,
    /// Error from last operation (if any)
    pub last_error: Option<String>,
    /// Temporary data storage for scenarios
    pub scenario_data: std::collections::HashMap<String, serde_json::Value>,
}

impl OrbitWorld {
    /// Create a new SQL executor for testing
    pub async fn create_sql_executor(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let storage = Arc::new(orbit_protocols::postgres_wire::storage::memory::MemoryStorage::new());
        self.sql_executor = Some(orbit_protocols::postgres_wire::sql::SqlExecutor::new(storage).await?);
        Ok(())
    }

    /// Execute SQL statement and store result
    pub async fn execute_sql(&mut self, sql: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(executor) = &self.sql_executor {
            match executor.execute(sql).await {
                Ok(result) => {
                    self.last_result = Some(serde_json::to_value(result)?);
                    self.last_error = None;
                }
                Err(e) => {
                    self.last_error = Some(e.to_string());
                    self.last_result = None;
                }
            }
        } else {
            return Err("SQL executor not initialized".into());
        }
        Ok(())
    }
}

// Step definitions for database operations
#[given(expr = "I have an empty database")]
async fn given_empty_database(world: &mut OrbitWorld) -> Result<(), Box<dyn std::error::Error>> {
    world.create_sql_executor().await?;
    Ok(())
}

#[given(expr = "I have a table {string} with columns {string}")]
async fn given_table_with_columns(
    world: &mut OrbitWorld,
    table_name: String,
    columns: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let sql = format!("CREATE TABLE {} ({})", table_name, columns);
    world.execute_sql(&sql).await?;
    Ok(())
}

#[when(expr = "I execute the SQL statement {string}")]
async fn when_execute_sql(
    world: &mut OrbitWorld,
    sql: String,
) -> Result<(), Box<dyn std::error::Error>> {
    world.execute_sql(&sql).await?;
    Ok(())
}

#[when(expr = "I insert the following data into {string}:")]
async fn when_insert_data(
    world: &mut OrbitWorld,
    table_name: String,
    data_table: &cucumber::DataTable,
) -> Result<(), Box<dyn std::error::Error>> {
    let headers = data_table.raw().first().unwrap();
    let columns = headers.join(", ");
    
    for row in data_table.raw().iter().skip(1) {
        let values = row.iter()
            .map(|v| if v.parse::<f64>().is_ok() { v.to_string() } else { format!("'{}'", v) })
            .collect::<Vec<_>>()
            .join(", ");
        
        let sql = format!("INSERT INTO {} ({}) VALUES ({})", table_name, columns, values);
        world.execute_sql(&sql).await?;
    }
    Ok(())
}

#[then(expr = "the operation should succeed")]
async fn then_operation_succeeds(world: &mut OrbitWorld) -> Result<(), Box<dyn std::error::Error>> {
    if world.last_error.is_some() {
        return Err(format!("Expected success but got error: {:?}", world.last_error).into());
    }
    Ok(())
}

#[then(expr = "the operation should fail")]
async fn then_operation_fails(world: &mut OrbitWorld) -> Result<(), Box<dyn std::error::Error>> {
    if world.last_error.is_none() {
        return Err("Expected failure but operation succeeded".into());
    }
    Ok(())
}

#[then(expr = "the error message should contain {string}")]
async fn then_error_contains(
    world: &mut OrbitWorld,
    expected: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(error) = &world.last_error {
        if !error.contains(&expected) {
            return Err(format!("Error '{}' does not contain '{}'", error, expected).into());
        }
    } else {
        return Err("No error occurred but one was expected".into());
    }
    Ok(())
}

#[then(expr = "the result should have {int} rows")]
async fn then_result_has_rows(
    world: &mut OrbitWorld,
    expected_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = &world.last_result {
        if let Some(rows) = result.get("rows") {
            if let Some(rows_array) = rows.as_array() {
                let actual_count = rows_array.len();
                if actual_count != expected_count {
                    return Err(format!("Expected {} rows but got {}", expected_count, actual_count).into());
                }
            } else {
                return Err("Result does not contain rows array".into());
            }
        } else {
            return Err("Result does not contain rows field".into());
        }
    } else {
        return Err("No result available".into());
    }
    Ok(())
}

#[then(expr = "the table {string} should have {int} rows")]
async fn then_table_has_rows(
    world: &mut OrbitWorld,
    table_name: String,
    expected_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let sql = format!("SELECT COUNT(*) as count FROM {}", table_name);
    world.execute_sql(&sql).await?;
    
    if let Some(result) = &world.last_result {
        if let Some(rows) = result.get("rows") {
            if let Some(rows_array) = rows.as_array() {
                if let Some(first_row) = rows_array.first() {
                    if let Some(row_array) = first_row.as_array() {
                        if let Some(count_value) = row_array.first() {
                            if let Some(count_str) = count_value.as_str() {
                                let actual_count: usize = count_str.parse()?;
                                if actual_count != expected_count {
                                    return Err(format!("Expected {} rows in table {} but got {}", 
                                                     expected_count, table_name, actual_count).into());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// Vector similarity search step definitions
#[given(expr = "I have a vector store with dimension {int}")]
async fn given_vector_store(
    world: &mut OrbitWorld,
    dimension: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = orbit_protocols::vector_store::VectorIndexConfig::new(
        "test_index".to_string(),
        dimension,
        orbit_protocols::vector_store::SimilarityMetric::Cosine,
    );
    let index = orbit_protocols::vector_store::VectorIndex::new(config);
    world.vector_store = Some(Arc::new(RwLock::new(index)));
    Ok(())
}

#[when(expr = "I add a vector with id {string} and data {string}")]
async fn when_add_vector(
    world: &mut OrbitWorld,
    vector_id: String,
    vector_data: String,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(store) = &world.vector_store {
        // Parse vector data (e.g., "[1.0, 2.0, 3.0]")
        let cleaned = vector_data.trim_matches(|c| c == '[' || c == ']');
        let values: Result<Vec<f32>, _> = cleaned
            .split(',')
            .map(|s| s.trim().parse::<f32>())
            .collect();
        
        let data = values?;
        let vector = orbit_protocols::vector_store::Vector::new(vector_id, data);
        
        let mut index = store.write().await;
        match index.add_vector(vector) {
            Ok(_) => {
                world.last_error = None;
            }
            Err(e) => {
                world.last_error = Some(e.to_string());
            }
        }
    } else {
        return Err("Vector store not initialized".into());
    }
    Ok(())
}

#[when(expr = "I search for vectors similar to {string} with limit {int}")]
async fn when_search_vectors(
    world: &mut OrbitWorld,
    query_vector: String,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(store) = &world.vector_store {
        // Parse query vector
        let cleaned = query_vector.trim_matches(|c| c == '[' || c == ']');
        let values: Result<Vec<f32>, _> = cleaned
            .split(',')
            .map(|s| s.trim().parse::<f32>())
            .collect();
        
        let query_data = values?;
        
        let index = store.read().await;
        match index.search(&query_data, limit) {
            Ok(results) => {
                world.last_result = Some(serde_json::to_value(results)?);
                world.last_error = None;
            }
            Err(e) => {
                world.last_error = Some(e.to_string());
            }
        }
    } else {
        return Err("Vector store not initialized".into());
    }
    Ok(())
}

#[then(expr = "I should find {int} similar vectors")]
async fn then_find_similar_vectors(
    world: &mut OrbitWorld,
    expected_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(result) = &world.last_result {
        if let Some(results_array) = result.as_array() {
            let actual_count = results_array.len();
            if actual_count != expected_count {
                return Err(format!("Expected {} similar vectors but found {}", 
                                 expected_count, actual_count).into());
            }
        } else {
            return Err("Search result is not an array".into());
        }
    } else {
        return Err("No search result available".into());
    }
    Ok(())
}

#[then(expr = "the first result should have id {string}")]
async fn then_first_result_has_id(
    world: &mut OrbitWorld,
    expected_id: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let result = get_search_result(world)?;
    let results_array = get_results_array(&result)?;
    let first_result = get_first_result(&results_array)?;
    let actual_id = extract_vector_id(&first_result)?;
    
    validate_expected_id(&actual_id, &expected_id)?;
    Ok(())
}

/// Extract search result from world, returning error if none exists
fn get_search_result(world: &OrbitWorld) -> Result<&serde_json::Value, Box<dyn std::error::Error>> {
    world.last_result.as_ref()
        .ok_or_else(|| "No search result available".into())
}

/// Extract results array from JSON value, returning error if not an array
fn get_results_array(result: &serde_json::Value) -> Result<&Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    result.as_array()
        .ok_or_else(|| "Search result is not an array".into())
}

/// Extract first result from results array, returning error if empty
fn get_first_result(results_array: &[serde_json::Value]) -> Result<&serde_json::Value, Box<dyn std::error::Error>> {
    results_array.first()
        .ok_or_else(|| "No results found".into())
}

/// Extract vector ID from result object
fn extract_vector_id(first_result: &serde_json::Value) -> Result<String, Box<dyn std::error::Error>> {
    let vector_obj = first_result.get("vector")
        .ok_or("First result does not contain vector object")?;
    
    let actual_id = vector_obj.get("id")
        .ok_or("First result does not contain vector ID")?;
    
    actual_id.as_str()
        .ok_or("Vector ID is not a string")
        .map(|s| s.to_string())
        .map_err(|e| e.into())
}

/// Validate that actual ID matches expected ID
fn validate_expected_id(actual_id: &str, expected_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    if actual_id != expected_id {
        return Err(format!(
            "Expected first result ID '{}' but got '{}'", 
            expected_id, actual_id
        ).into());
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    OrbitWorld::run("tests/bdd/features").await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bdd_step_definitions() {
        // This test ensures that the BDD step definitions compile and can be used
        let mut world = OrbitWorld::default();
        
        // Test SQL operations
        given_empty_database(&mut world).await.unwrap();
        given_table_with_columns(&mut world, "users".to_string(), "id INTEGER, name TEXT".to_string()).await.unwrap();
        when_execute_sql(&mut world, "INSERT INTO users VALUES (1, 'Alice')".to_string()).await.unwrap();
        then_operation_succeeds(&mut world).await.unwrap();
        
        // Test vector operations
        given_vector_store(&mut world, 3).await.unwrap();
        when_add_vector(&mut world, "vec1".to_string(), "[1.0, 2.0, 3.0]".to_string()).await.unwrap();
        when_search_vectors(&mut world, "[1.0, 2.0, 3.0]".to_string(), 1).await.unwrap();
        then_find_similar_vectors(&mut world, 1).await.unwrap();
    }
}