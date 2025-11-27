//! Procedure Catalog

use super::ast::ProcedureDef;
use crate::error::{EngineError, EngineResult};
use std::collections::HashMap;
use std::sync::RwLock;

/// Procedure Catalog
#[derive(Debug, Default)]
pub struct ProcedureCatalog {
    procedures: RwLock<HashMap<String, ProcedureDef>>,
}

impl ProcedureCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a stored procedure
    pub fn register_procedure(&self, procedure: ProcedureDef) -> EngineResult<()> {
        let mut procedures = self
            .procedures
            .write()
            .map_err(|_| EngineError::LockError)?;
        if procedures.contains_key(&procedure.name) {
            return Err(EngineError::AlreadyExists(format!(
                "Procedure {}",
                procedure.name
            )));
        }
        procedures.insert(procedure.name.clone(), procedure);
        Ok(())
    }

    /// Get a stored procedure by name
    pub fn get_procedure(&self, name: &str) -> EngineResult<ProcedureDef> {
        let procedures = self.procedures.read().map_err(|_| EngineError::LockError)?;
        procedures
            .get(name)
            .cloned()
            .ok_or_else(|| EngineError::NotFound(format!("Procedure {}", name)))
    }

    /// Drop a stored procedure
    pub fn drop_procedure(&self, name: &str) -> EngineResult<()> {
        let mut procedures = self
            .procedures
            .write()
            .map_err(|_| EngineError::LockError)?;
        if !procedures.contains_key(name) {
            return Err(EngineError::NotFound(format!("Procedure {}", name)));
        }
        procedures.remove(name);
        Ok(())
    }

    /// List all procedures
    pub fn list_procedures(&self) -> EngineResult<Vec<ProcedureDef>> {
        let procedures = self.procedures.read().map_err(|_| EngineError::LockError)?;
        Ok(procedures.values().cloned().collect())
    }
}
