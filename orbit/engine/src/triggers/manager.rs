//! Trigger Manager

use crate::error::{EngineError, EngineResult};
use crate::procedures::ProcedureDef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

use crate::storage::Row;

/// Trigger Event Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
}

/// Trigger Timing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TriggerTiming {
    Before,
    After,
}

/// Trigger Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDef {
    pub name: String,
    pub table: String,
    pub event: TriggerEvent,
    pub timing: TriggerTiming,
    pub procedure_name: String,
}

/// Trigger Manager
#[derive(Debug, Default)]
pub struct TriggerManager {
    // Map: Table -> Event -> Timing -> List of Triggers
    triggers: RwLock<HashMap<String, Vec<TriggerDef>>>,
}

impl TriggerManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a trigger
    pub fn register_trigger(&self, trigger: TriggerDef) -> EngineResult<()> {
        let mut triggers = self.triggers.write().map_err(|_| EngineError::LockError)?;
        let table_triggers = triggers
            .entry(trigger.table.clone())
            .or_insert_with(Vec::new);

        // Check for duplicate name
        if table_triggers.iter().any(|t| t.name == trigger.name) {
            return Err(EngineError::AlreadyExists(format!(
                "Trigger {}",
                trigger.name
            )));
        }

        table_triggers.push(trigger);
        Ok(())
    }

    /// Get triggers for a specific event on a table
    pub fn get_triggers(
        &self,
        table: &str,
        event: TriggerEvent,
        timing: TriggerTiming,
    ) -> EngineResult<Vec<TriggerDef>> {
        let triggers = self.triggers.read().map_err(|_| EngineError::LockError)?;
        if let Some(table_triggers) = triggers.get(table) {
            let matching: Vec<TriggerDef> = table_triggers
                .iter()
                .filter(|t| t.event == event && t.timing == timing)
                .cloned()
                .collect();
            Ok(matching)
        } else {
            Ok(Vec::new())
        }
    }

    /// Drop a trigger
    pub fn drop_trigger(&self, table: &str, name: &str) -> EngineResult<()> {
        let mut triggers = self.triggers.write().map_err(|_| EngineError::LockError)?;
        if let Some(table_triggers) = triggers.get_mut(table) {
            if let Some(pos) = table_triggers.iter().position(|t| t.name == name) {
                table_triggers.remove(pos);
                Ok(())
            } else {
                Err(EngineError::NotFound(format!("Trigger {}", name)))
            }
        } else {
            Err(EngineError::NotFound(format!("Trigger {}", name)))
        }
    }
}

/// Trigger Executor Trait
#[async_trait::async_trait]
pub trait TriggerExecutor: Send + Sync {
    /// Execute a trigger
    async fn execute_trigger(
        &self,
        trigger: &TriggerDef,
        old_row: Option<&Row>,
        new_row: Option<&Row>,
    ) -> EngineResult<()>;
}
