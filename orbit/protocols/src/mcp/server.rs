//! MCP server implementation

#![cfg(feature = "storage-rocksdb")]

use super::{McpCapabilities, McpConfig};
use crate::mcp::integration::OrbitMcpIntegration;
use crate::mcp::nlp::{NlpQueryProcessor, SchemaAnalyzer};
use crate::mcp::result_processor::ResultProcessor;
use crate::mcp::sql_generator::SqlGenerator;
use std::sync::Arc;

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
    /// Result processor for formatting results
    pub result_processor: ResultProcessor,
    /// Orbit-RS integration (optional)
    pub orbit_integration: Option<Arc<OrbitMcpIntegration>>,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(config: McpConfig, capabilities: McpCapabilities) -> Self {
        let schema_analyzer = SchemaAnalyzer::new();
        let nlp_processor = NlpQueryProcessor::new(schema_analyzer);
        let sql_generator = SqlGenerator::new();
        let result_processor = ResultProcessor::new();

        Self {
            config,
            capabilities,
            nlp_processor,
            sql_generator,
            result_processor,
            orbit_integration: None,
        }
    }

    /// Create a new MCP server with Orbit-RS integration
    pub fn with_orbit_integration(
        config: McpConfig,
        capabilities: McpCapabilities,
        integration: Arc<OrbitMcpIntegration>,
    ) -> Self {
        let schema_analyzer = SchemaAnalyzer::new();
        let nlp_processor = NlpQueryProcessor::new(schema_analyzer);
        let sql_generator = SqlGenerator::new();
        let result_processor = ResultProcessor::new();

        Self {
            config,
            capabilities,
            nlp_processor,
            sql_generator,
            result_processor,
            orbit_integration: Some(integration),
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

    /// Execute a natural language query end-to-end (NLP → SQL → Results)
    pub async fn execute_natural_language_query(
        &self,
        query: &str,
    ) -> Result<crate::mcp::result_processor::ProcessedResult, crate::mcp::types::McpError> {
        // Step 1: Generate SQL from natural language
        let generated_query = self.process_natural_language_query(query).await?;

        // Step 2: Execute SQL if integration is available
        if let Some(ref integration) = self.orbit_integration {
            let query_result = integration
                .execute_generated_query(&generated_query)
                .await?;

            // Step 3: Process results
            let processed = self.result_processor.process_results(&query_result, 100);
            Ok(processed)
        } else {
            Err(crate::mcp::types::McpError::InternalError(
                "Orbit-RS integration not available".to_string(),
            ))
        }
    }
}
