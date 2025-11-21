//! MCP server implementation

use super::{McpCapabilities, McpConfig};
use crate::mcp::nlp::{NlpQueryProcessor, SchemaAnalyzer};
use crate::mcp::sql_generator::SqlGenerator;

/// MCP server with natural language processing
pub struct McpServer {
    #[allow(dead_code)]
    config: McpConfig,
    #[allow(dead_code)]
    capabilities: McpCapabilities,
    /// Natural language query processor
    pub nlp_processor: NlpQueryProcessor,
    /// SQL generator
    pub sql_generator: SqlGenerator,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(config: McpConfig, capabilities: McpCapabilities) -> Self {
        let schema_analyzer = SchemaAnalyzer::new();
        let nlp_processor = NlpQueryProcessor::new(schema_analyzer);
        let sql_generator = SqlGenerator::new();

        Self {
            config,
            capabilities,
            nlp_processor,
            sql_generator,
        }
    }

    /// Process a natural language query and generate SQL
    pub async fn process_natural_language_query(
        &self,
        query: &str,
    ) -> Result<crate::mcp::sql_generator::GeneratedQuery, crate::mcp::types::McpError> {
        // Step 1: Process natural language
        let intent = self
            .nlp_processor
            .process_query(query)
            .await
            .map_err(|e| crate::mcp::types::McpError::InternalError(e.to_string()))?;

        // Step 2: Generate SQL
        let generated_query = self
            .sql_generator
            .generate_sql(&intent)
            .map_err(|e| crate::mcp::types::McpError::SqlError(e.to_string()))?;

        Ok(generated_query)
    }
}
