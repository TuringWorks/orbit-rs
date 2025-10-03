//! Cypher query parser (stub)

use crate::error::{ProtocolError, ProtocolResult};

/// Cypher query parser
pub struct CypherParser {
    // TODO: Implement Cypher AST
}

impl CypherParser {
    /// Create a new Cypher parser
    pub fn new() -> Self {
        Self {}
    }

    /// Parse Cypher query
    pub fn parse(&self, _cypher: &str) -> ProtocolResult<CypherQuery> {
        // TODO: Implement Cypher parsing
        Err(ProtocolError::CypherError("Not implemented".to_string()))
    }
}

impl Default for CypherParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parsed Cypher query
#[derive(Debug, Clone)]
pub struct CypherQuery {
    // TODO: Add query AST fields
}
