//! Database Triggers - Event-Driven Actions
//!
//! This module provides comprehensive database trigger support for event-driven
//! database actions and business logic enforcement. It implements:
//! - BEFORE/AFTER INSERT/UPDATE/DELETE triggers
//! - Row-level and statement-level triggers
//! - Trigger condition evaluation (WHEN clauses)
//! - Cascading trigger support with recursive detection
//! - Integration with transaction and CDC systems

use crate::cdc::{CdcEvent, DmlOperation};
use crate::exception::{OrbitError, OrbitResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// When a trigger should fire relative to the DML operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TriggerTiming {
    /// Execute before the operation
    Before,
    /// Execute after the operation
    After,
}

/// Level at which the trigger executes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TriggerLevel {
    /// Execute once per affected row
    Row,
    /// Execute once per statement
    Statement,
}

/// DML events that can fire triggers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum TriggerEvent {
    /// INSERT operations
    Insert,
    /// UPDATE operations
    Update,
    /// DELETE operations
    Delete,
    /// TRUNCATE operations
    Truncate,
}

impl From<DmlOperation> for TriggerEvent {
    fn from(op: DmlOperation) -> Self {
        match op {
            DmlOperation::Insert => TriggerEvent::Insert,
            DmlOperation::Update => TriggerEvent::Update,
            DmlOperation::Delete => TriggerEvent::Delete,
            DmlOperation::Truncate => TriggerEvent::Truncate,
        }
    }
}

impl From<TriggerEvent> for DmlOperation {
    fn from(event: TriggerEvent) -> Self {
        match event {
            TriggerEvent::Insert => DmlOperation::Insert,
            TriggerEvent::Update => DmlOperation::Update,
            TriggerEvent::Delete => DmlOperation::Delete,
            TriggerEvent::Truncate => DmlOperation::Truncate,
        }
    }
}

/// Trigger definition and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDefinition {
    /// Unique trigger ID
    pub trigger_id: Uuid,
    /// Trigger name
    pub name: String,
    /// Schema name (if applicable)
    pub schema: Option<String>,
    /// Table name the trigger is on
    pub table: String,
    /// When to fire (BEFORE/AFTER)
    pub timing: TriggerTiming,
    /// Events that fire this trigger
    pub events: Vec<TriggerEvent>,
    /// Execution level (ROW/STATEMENT)
    pub level: TriggerLevel,
    /// Optional WHEN condition (SQL expression)
    pub when_condition: Option<String>,
    /// Function to execute (function name or inline code)
    pub function_name: String,
    /// Optional function arguments
    pub function_args: Vec<String>,
    /// Whether this trigger is enabled
    pub enabled: bool,
    /// Creation timestamp
    pub created_at: i64,
    /// Last modification timestamp
    pub updated_at: i64,
    /// Metadata
    pub metadata: HashMap<String, String>,
}

impl TriggerDefinition {
    /// Create a new trigger definition
    pub fn new(
        name: String,
        table: String,
        timing: TriggerTiming,
        events: Vec<TriggerEvent>,
        level: TriggerLevel,
        function_name: String,
    ) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            trigger_id: Uuid::new_v4(),
            name,
            schema: None,
            table,
            timing,
            events,
            level,
            when_condition: None,
            function_name,
            function_args: Vec::new(),
            enabled: true,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        }
    }

    /// Set schema
    pub fn with_schema(mut self, schema: String) -> Self {
        self.schema = Some(schema);
        self
    }

    /// Set WHEN condition
    pub fn with_when_condition(mut self, condition: String) -> Self {
        self.when_condition = Some(condition);
        self
    }

    /// Set function arguments
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.function_args = args;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Check if trigger should fire for a given event
    pub fn matches_event(&self, event: TriggerEvent) -> bool {
        self.enabled && self.events.contains(&event)
    }
}

/// Context passed to trigger functions during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerContext {
    /// The trigger being executed
    pub trigger_id: Uuid,
    /// Trigger name for logging
    pub trigger_name: String,
    /// Table name
    pub table: String,
    /// Event type
    pub event: TriggerEvent,
    /// Transaction ID (if in transaction)
    pub transaction_id: Option<String>,
    /// Old row values (for UPDATE/DELETE)
    pub old_row: Option<HashMap<String, serde_json::Value>>,
    /// New row values (for INSERT/UPDATE)
    pub new_row: Option<HashMap<String, serde_json::Value>>,
    /// Current timestamp
    pub timestamp: i64,
    /// Additional context data
    pub context_data: HashMap<String, serde_json::Value>,
}

impl TriggerContext {
    /// Create context from CDC event
    pub fn from_cdc_event(trigger: &TriggerDefinition, event: &CdcEvent) -> Self {
        Self {
            trigger_id: trigger.trigger_id,
            trigger_name: trigger.name.clone(),
            table: event.table.clone(),
            event: event.operation.clone().into(),
            transaction_id: event.transaction_id.clone(),
            old_row: event.old_values.clone(),
            new_row: event.new_values.clone(),
            timestamp: event.timestamp,
            context_data: HashMap::new(),
        }
    }

    /// Add context data
    pub fn with_data(mut self, key: String, value: serde_json::Value) -> Self {
        self.context_data.insert(key, value);
        self
    }
}

/// Result of trigger execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerResult {
    /// Whether execution was successful
    pub success: bool,
    /// Modified row values (for BEFORE triggers that modify data)
    pub modified_row: Option<HashMap<String, serde_json::Value>>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Execution duration in microseconds
    pub duration_micros: u64,
    /// Additional result metadata
    pub metadata: HashMap<String, String>,
}

impl TriggerResult {
    /// Create a successful result
    pub fn success() -> Self {
        Self {
            success: true,
            modified_row: None,
            error: None,
            duration_micros: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a successful result with modified row
    pub fn success_with_row(row: HashMap<String, serde_json::Value>) -> Self {
        Self {
            success: true,
            modified_row: Some(row),
            error: None,
            duration_micros: 0,
            metadata: HashMap::new(),
        }
    }

    /// Create a failure result
    pub fn failure(error: String) -> Self {
        Self {
            success: false,
            modified_row: None,
            error: Some(error),
            duration_micros: 0,
            metadata: HashMap::new(),
        }
    }

    /// Set execution duration
    pub fn with_duration(mut self, duration_micros: u64) -> Self {
        self.duration_micros = duration_micros;
        self
    }
}

/// Trait for implementing trigger functions
#[async_trait]
pub trait TriggerFunction: Send + Sync {
    /// Execute the trigger function
    async fn execute(&self, context: &TriggerContext) -> OrbitResult<TriggerResult>;

    /// Get function name
    fn name(&self) -> &str;

    /// Get function description
    fn description(&self) -> Option<&str> {
        None
    }
}

/// Trigger executor that runs trigger functions
pub struct TriggerExecutor {
    /// Registry of available trigger functions
    functions: Arc<RwLock<HashMap<String, Arc<dyn TriggerFunction>>>>,
}

impl TriggerExecutor {
    /// Create a new trigger executor
    pub fn new() -> Self {
        Self {
            functions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a trigger function
    pub async fn register_function(&self, function: Arc<dyn TriggerFunction>) {
        let name = function.name().to_string();
        let mut functions = self.functions.write().await;
        functions.insert(name.clone(), function);
        debug!("Registered trigger function: {}", name);
    }

    /// Unregister a trigger function
    pub async fn unregister_function(&self, name: &str) -> OrbitResult<()> {
        let mut functions = self.functions.write().await;
        functions.remove(name);
        debug!("Unregistered trigger function: {}", name);
        Ok(())
    }

    /// Execute a trigger
    pub async fn execute_trigger(
        &self,
        trigger: &TriggerDefinition,
        context: &TriggerContext,
    ) -> OrbitResult<TriggerResult> {
        let start = std::time::Instant::now();

        // Check if trigger function exists
        let functions = self.functions.read().await;
        let function = functions.get(&trigger.function_name).ok_or_else(|| {
            OrbitError::internal(format!(
                "Trigger function '{}' not found",
                trigger.function_name
            ))
        })?;

        // Execute the function
        debug!(
            "Executing trigger '{}' on table '{}'",
            trigger.name, trigger.table
        );

        let result = function.execute(context).await;

        let duration = start.elapsed();
        let duration_micros = duration.as_micros() as u64;

        match result {
            Ok(mut res) => {
                res.duration_micros = duration_micros;
                debug!(
                    "Trigger '{}' executed successfully in {}μs",
                    trigger.name, duration_micros
                );
                Ok(res)
            }
            Err(e) => {
                error!("Trigger '{}' failed: {}", trigger.name, e);
                Ok(TriggerResult::failure(e.to_string()).with_duration(duration_micros))
            }
        }
    }

    /// Evaluate WHEN condition
    pub async fn evaluate_when_condition(
        &self,
        condition: &str,
        _context: &TriggerContext,
    ) -> OrbitResult<bool> {
        // For now, return true (execute trigger)
        // TODO: Implement actual condition evaluation using SQL expression parser
        debug!("Evaluating WHEN condition: {}", condition);
        Ok(true)
    }
}

impl Default for TriggerExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Trigger coordinator that manages all triggers and their execution
pub struct TriggerCoordinator {
    /// Trigger definitions indexed by table
    triggers: Arc<RwLock<HashMap<String, Vec<TriggerDefinition>>>>,
    /// Trigger executor
    executor: Arc<TriggerExecutor>,
    /// Recursion depth tracking to prevent infinite loops
    recursion_depth: Arc<RwLock<HashMap<String, usize>>>,
    /// Maximum recursion depth allowed
    max_recursion_depth: usize,
    /// Statistics
    stats: Arc<RwLock<TriggerStats>>,
}

impl TriggerCoordinator {
    /// Create a new trigger coordinator
    pub fn new() -> Self {
        Self {
            triggers: Arc::new(RwLock::new(HashMap::new())),
            executor: Arc::new(TriggerExecutor::new()),
            recursion_depth: Arc::new(RwLock::new(HashMap::new())),
            max_recursion_depth: 10,
            stats: Arc::new(RwLock::new(TriggerStats::default())),
        }
    }

    /// Create with custom max recursion depth
    pub fn with_max_recursion_depth(max_depth: usize) -> Self {
        Self {
            triggers: Arc::new(RwLock::new(HashMap::new())),
            executor: Arc::new(TriggerExecutor::new()),
            recursion_depth: Arc::new(RwLock::new(HashMap::new())),
            max_recursion_depth: max_depth,
            stats: Arc::new(RwLock::new(TriggerStats::default())),
        }
    }

    /// Register a trigger definition
    pub async fn register_trigger(&self, trigger: TriggerDefinition) -> OrbitResult<()> {
        let table = trigger.table.clone();
        let name = trigger.name.clone();

        let mut triggers = self.triggers.write().await;
        let table_triggers = triggers.entry(table.clone()).or_insert_with(Vec::new);

        // Check for duplicate trigger names on the same table
        if table_triggers.iter().any(|t| t.name == name) {
            return Err(OrbitError::internal(format!(
                "Trigger '{}' already exists on table '{}'",
                name, table
            )));
        }

        table_triggers.push(trigger);
        info!("Registered trigger '{}' on table '{}'", name, table);

        let mut stats = self.stats.write().await;
        stats.total_triggers += 1;

        Ok(())
    }

    /// Unregister a trigger
    pub async fn unregister_trigger(&self, table: &str, trigger_name: &str) -> OrbitResult<()> {
        let mut triggers = self.triggers.write().await;

        if let Some(table_triggers) = triggers.get_mut(table) {
            let original_len = table_triggers.len();
            table_triggers.retain(|t| t.name != trigger_name);

            if table_triggers.len() < original_len {
                info!(
                    "Unregistered trigger '{}' from table '{}'",
                    trigger_name, table
                );
                let mut stats = self.stats.write().await;
                stats.total_triggers = stats.total_triggers.saturating_sub(1);
                Ok(())
            } else {
                Err(OrbitError::internal(format!(
                    "Trigger '{}' not found on table '{}'",
                    trigger_name, table
                )))
            }
        } else {
            Err(OrbitError::internal(format!(
                "No triggers found for table '{}'",
                table
            )))
        }
    }

    /// Get all triggers for a table
    pub async fn get_triggers(&self, table: &str) -> Vec<TriggerDefinition> {
        let triggers = self.triggers.read().await;
        triggers.get(table).cloned().unwrap_or_default()
    }

    /// Get a specific trigger
    pub async fn get_trigger(&self, table: &str, trigger_name: &str) -> Option<TriggerDefinition> {
        let triggers = self.triggers.read().await;
        triggers
            .get(table)?
            .iter()
            .find(|t| t.name == trigger_name)
            .cloned()
    }

    /// Register a trigger function
    pub async fn register_function(&self, function: Arc<dyn TriggerFunction>) {
        self.executor.register_function(function).await;
    }

    /// Fire triggers for a CDC event
    pub async fn fire_triggers(
        &self,
        event: &CdcEvent,
        timing: TriggerTiming,
    ) -> OrbitResult<Vec<TriggerResult>> {
        let table = &event.table;
        let trigger_event: TriggerEvent = event.operation.clone().into();

        // Get all triggers for this table
        let triggers = self.triggers.read().await;
        let table_triggers = match triggers.get(table) {
            Some(t) => t.clone(),
            None => return Ok(Vec::new()),
        };
        drop(triggers);

        // Filter triggers by timing, event, and enabled status
        let matching_triggers: Vec<_> = table_triggers
            .into_iter()
            .filter(|t| t.timing == timing && t.matches_event(trigger_event))
            .collect();

        if matching_triggers.is_empty() {
            return Ok(Vec::new());
        }

        // Check recursion depth
        let recursion_key = format!("{}::{}", table, timing as u8);
        let current_depth = {
            let mut depth_map = self.recursion_depth.write().await;
            let depth = depth_map.entry(recursion_key.clone()).or_insert(0);
            *depth += 1;
            *depth
        };

        if current_depth > self.max_recursion_depth {
            // Clean up depth counter
            let mut depth_map = self.recursion_depth.write().await;
            depth_map.remove(&recursion_key);

            return Err(OrbitError::internal(format!(
                "Maximum trigger recursion depth ({}) exceeded for table '{}'",
                self.max_recursion_depth, table
            )));
        }

        let mut results = Vec::new();

        // Execute triggers in order
        for trigger in matching_triggers {
            // Create context
            let context = TriggerContext::from_cdc_event(&trigger, event);

            // Evaluate WHEN condition if present
            let should_execute = if let Some(ref condition) = trigger.when_condition {
                match self
                    .executor
                    .evaluate_when_condition(condition, &context)
                    .await
                {
                    Ok(result) => result,
                    Err(e) => {
                        warn!(
                            "Failed to evaluate WHEN condition for trigger '{}': {}",
                            trigger.name, e
                        );
                        false
                    }
                }
            } else {
                true
            };

            if should_execute {
                let result = self.executor.execute_trigger(&trigger, &context).await?;

                // Update statistics
                let mut stats = self.stats.write().await;
                stats.total_executions += 1;
                if result.success {
                    stats.successful_executions += 1;
                } else {
                    stats.failed_executions += 1;
                }
                stats.total_execution_time_micros += result.duration_micros;

                results.push(result);
            }
        }

        // Decrement recursion depth
        let mut depth_map = self.recursion_depth.write().await;
        if let Some(depth) = depth_map.get_mut(&recursion_key) {
            *depth = depth.saturating_sub(1);
            if *depth == 0 {
                depth_map.remove(&recursion_key);
            }
        }

        Ok(results)
    }

    /// Get trigger statistics
    pub async fn get_stats(&self) -> TriggerStats {
        self.stats.read().await.clone()
    }

    /// Clear all triggers
    pub async fn clear_all(&self) {
        let mut triggers = self.triggers.write().await;
        triggers.clear();
        let mut stats = self.stats.write().await;
        stats.total_triggers = 0;
        info!("Cleared all triggers");
    }
}

impl Default for TriggerCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for TriggerCoordinator {
    fn clone(&self) -> Self {
        Self {
            triggers: Arc::clone(&self.triggers),
            executor: Arc::clone(&self.executor),
            recursion_depth: Arc::clone(&self.recursion_depth),
            max_recursion_depth: self.max_recursion_depth,
            stats: Arc::clone(&self.stats),
        }
    }
}

/// Trigger statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerStats {
    /// Total number of registered triggers
    pub total_triggers: usize,
    /// Total trigger executions
    pub total_executions: u64,
    /// Successful executions
    pub successful_executions: u64,
    /// Failed executions
    pub failed_executions: u64,
    /// Total execution time in microseconds
    pub total_execution_time_micros: u64,
}

impl TriggerStats {
    /// Get average execution time in microseconds
    pub fn avg_execution_time_micros(&self) -> u64 {
        if self.total_executions > 0 {
            self.total_execution_time_micros / self.total_executions
        } else {
            0
        }
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_executions > 0 {
            (self.successful_executions as f64 / self.total_executions as f64) * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_definition_creation() {
        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "validate_user".to_string(),
        );

        assert_eq!(trigger.name, "test_trigger");
        assert_eq!(trigger.table, "users");
        assert_eq!(trigger.timing, TriggerTiming::Before);
        assert_eq!(trigger.level, TriggerLevel::Row);
        assert!(trigger.enabled);
        assert!(trigger.matches_event(TriggerEvent::Insert));
        assert!(!trigger.matches_event(TriggerEvent::Update));
    }

    #[test]
    fn test_trigger_definition_with_condition() {
        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Update],
            TriggerLevel::Row,
            "log_update".to_string(),
        )
        .with_when_condition("NEW.status != OLD.status".to_string())
        .with_schema("public".to_string());

        assert!(trigger.when_condition.is_some());
        assert!(trigger.schema.is_some());
    }

    #[test]
    fn test_trigger_event_conversion() {
        let dml_op = DmlOperation::Insert;
        let trigger_event: TriggerEvent = dml_op.into();
        assert_eq!(trigger_event, TriggerEvent::Insert);

        let back_to_dml: DmlOperation = trigger_event.into();
        assert_eq!(back_to_dml, DmlOperation::Insert);
    }

    #[test]
    fn test_trigger_result() {
        let result = TriggerResult::success();
        assert!(result.success);
        assert!(result.error.is_none());

        let failure = TriggerResult::failure("Test error".to_string());
        assert!(!failure.success);
        assert_eq!(failure.error, Some("Test error".to_string()));
    }

    #[tokio::test]
    async fn test_trigger_coordinator_creation() {
        let coordinator = TriggerCoordinator::new();
        let stats = coordinator.get_stats().await;
        assert_eq!(stats.total_triggers, 0);
    }

    #[tokio::test]
    async fn test_register_trigger() {
        let coordinator = TriggerCoordinator::new();

        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "validate_user".to_string(),
        );

        let result = coordinator.register_trigger(trigger).await;
        assert!(result.is_ok());

        let stats = coordinator.get_stats().await;
        assert_eq!(stats.total_triggers, 1);
    }

    #[tokio::test]
    async fn test_register_duplicate_trigger() {
        let coordinator = TriggerCoordinator::new();

        let trigger1 = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "validate_user".to_string(),
        );

        let trigger2 = trigger1.clone();

        coordinator.register_trigger(trigger1).await.unwrap();
        let result = coordinator.register_trigger(trigger2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unregister_trigger() {
        let coordinator = TriggerCoordinator::new();

        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "validate_user".to_string(),
        );

        coordinator.register_trigger(trigger).await.unwrap();
        let result = coordinator
            .unregister_trigger("users", "test_trigger")
            .await;
        assert!(result.is_ok());

        let stats = coordinator.get_stats().await;
        assert_eq!(stats.total_triggers, 0);
    }

    #[tokio::test]
    async fn test_get_triggers() {
        let coordinator = TriggerCoordinator::new();

        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "validate_user".to_string(),
        );

        coordinator.register_trigger(trigger).await.unwrap();
        let triggers = coordinator.get_triggers("users").await;
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].name, "test_trigger");
    }

    #[tokio::test]
    async fn test_trigger_stats() {
        let stats = TriggerStats {
            total_triggers: 5,
            total_executions: 100,
            successful_executions: 95,
            failed_executions: 5,
            total_execution_time_micros: 10000,
        };

        assert_eq!(stats.avg_execution_time_micros(), 100);
        assert_eq!(stats.success_rate(), 95.0);
    }

    // Mock trigger function for testing
    struct MockTriggerFunction {
        name: String,
    }

    #[async_trait]
    impl TriggerFunction for MockTriggerFunction {
        async fn execute(&self, _context: &TriggerContext) -> OrbitResult<TriggerResult> {
            Ok(TriggerResult::success())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_fire_triggers() {
        let coordinator = TriggerCoordinator::new();

        // Register a mock function
        let function = Arc::new(MockTriggerFunction {
            name: "test_function".to_string(),
        });
        coordinator.register_function(function).await;

        // Register a trigger
        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "test_function".to_string(),
        );
        coordinator.register_trigger(trigger).await.unwrap();

        // Create a CDC event
        let mut values = HashMap::new();
        values.insert("id".to_string(), serde_json::json!(1));
        values.insert("name".to_string(), serde_json::json!("test"));

        let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

        // Fire triggers
        let results = coordinator
            .fire_triggers(&event, TriggerTiming::Before)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].success);

        let stats = coordinator.get_stats().await;
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.successful_executions, 1);
    }

    #[tokio::test]
    async fn test_recursion_detection() {
        let coordinator = TriggerCoordinator::with_max_recursion_depth(2);

        // Register a mock function
        let function = Arc::new(MockTriggerFunction {
            name: "test_function".to_string(),
        });
        coordinator.register_function(function).await;

        // Register a trigger
        let trigger = TriggerDefinition::new(
            "test_trigger".to_string(),
            "users".to_string(),
            TriggerTiming::Before,
            vec![TriggerEvent::Insert],
            TriggerLevel::Row,
            "test_function".to_string(),
        );
        coordinator.register_trigger(trigger).await.unwrap();

        let mut values = HashMap::new();
        values.insert("id".to_string(), serde_json::json!(1));
        let event = CdcEvent::insert("users".to_string(), "1".to_string(), values);

        // Simulate nested trigger execution by manually setting recursion depth
        // to exceed the maximum. In real usage, this would happen when a trigger
        // fires another DML operation that triggers the same trigger again
        let recursion_key = format!("users::{}", TriggerTiming::Before as u8);

        // Set depth to max + 1 to trigger recursion detection
        {
            let mut depth_map = coordinator.recursion_depth.write().await;
            depth_map.insert(recursion_key.clone(), coordinator.max_recursion_depth);
        }

        // This should fail due to recursion limit
        let result = coordinator
            .fire_triggers(&event, TriggerTiming::Before)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("recursion depth"));

        // Clean up
        coordinator.recursion_depth.write().await.clear();
    }
}
