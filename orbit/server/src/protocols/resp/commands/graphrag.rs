//! GraphRAG commands for RESP protocol
//!
//! This module implements RESP commands for GraphRAG operations:
//! - GRAPHRAG.BUILD - Build knowledge graph from document
//! - GRAPHRAG.QUERY - Execute RAG query
//! - GRAPHRAG.REASON - Find reasoning paths
//! - GRAPHRAG.STATS - Get statistics
//! - GRAPHRAG.ENTITIES - List entities
//! - GRAPHRAG.SIMILAR - Find similar entities

use super::traits::CommandHandler;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::graphrag::graph_rag_actor::{
    GraphRAGActor, GraphRAGDocumentRequest, GraphRAGQuery,
};
use crate::protocols::resp::simple_local::SimpleLocalRegistry;
use crate::protocols::resp::types::RespValue;
use orbit_client::OrbitClient;
use orbit_shared::graphrag::LLMProvider;
use std::sync::Arc;

/// GraphRAG command handler
pub struct GraphRAGCommands {
    #[allow(dead_code)]
    local_registry: Arc<SimpleLocalRegistry>,
    orbit_client: Arc<OrbitClient>,
}

impl GraphRAGCommands {
    /// Create a new GraphRAG commands handler
    pub fn new(
        orbit_client: Arc<OrbitClient>,
        local_registry: Arc<SimpleLocalRegistry>,
    ) -> Self {
        Self {
            local_registry,
            orbit_client,
        }
    }

    /// Get or create GraphRAG actor for a knowledge graph
    async fn get_or_create_graphrag_actor(
        &self,
        kg_name: &str,
    ) -> ProtocolResult<Arc<GraphRAGActor>> {
        // Try to get existing actor from local registry
        let _actor_key = format!("graphrag:{}", kg_name);

        // For now, create a new actor (in production, this would use the actor system)
        // TODO: Integrate with actual Orbit actor system
        let mut actor = GraphRAGActor::new(kg_name.to_string());
        actor.initialize_components();

        // Try to add default LLM provider if available
        if let Ok(ollama_model) = std::env::var("OLLAMA_MODEL") {
            actor.add_llm_provider(
                "ollama".to_string(),
                LLMProvider::Ollama {
                    model: ollama_model,
                    temperature: Some(0.7),
                },
            );
        }

        if let Ok(openai_key) = std::env::var("OPENAI_API_KEY") {
            actor.add_llm_provider(
                "openai".to_string(),
                LLMProvider::OpenAI {
                    api_key: openai_key,
                    model: "gpt-4".to_string(),
                    temperature: Some(0.7),
                    max_tokens: Some(2048),
                },
            );
        }

        Ok(Arc::new(actor))
    }

    /// Handle GRAPHRAG.BUILD command
    async fn handle_build(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 3 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPHRAG.BUILD'".to_string(),
            ));
        }

        let kg_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid kg_name".to_string()))?;
        let doc_id = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid doc_id".to_string()))?;
        let text = args[2]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid text".to_string()))?;

        let _actor = self.get_or_create_graphrag_actor(&kg_name).await?;

        // Use Orbit client
        let orbit_client = self.orbit_client.clone();

        let request = GraphRAGDocumentRequest {
            document_id: doc_id.clone(),
            text,
            metadata: std::collections::HashMap::new(),
            build_knowledge_graph: true,
            generate_embeddings: true,
            extractors: None,
        };

        // Process document
        // Note: GraphRAGActor methods require &mut self
        // In production, this would use the actor system to get mutable access
        let mut actor_mut = GraphRAGActor::new(kg_name.clone());
        actor_mut.initialize_components();

        let result = actor_mut
            .process_document(orbit_client, request)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        use bytes::Bytes;
        Ok(RespValue::Array(vec![
            RespValue::BulkString(Bytes::from("entities_extracted")),
            RespValue::Integer(result.entities_extracted as i64),
            RespValue::BulkString(Bytes::from("relationships_extracted")),
            RespValue::Integer(result.relationships_extracted as i64),
            RespValue::BulkString(Bytes::from("graph_nodes_created")),
            RespValue::Integer(result.graph_nodes_created as i64),
            RespValue::BulkString(Bytes::from("graph_relationships_created")),
            RespValue::Integer(result.graph_relationships_created as i64),
        ]))
    }

    /// Handle GRAPHRAG.QUERY command
    async fn handle_query(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.len() < 2 {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPHRAG.QUERY'".to_string(),
            ));
        }

        let kg_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid kg_name".to_string()))?;
        let query_text = args[1]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid query_text".to_string()))?;

        // Parse optional parameters
        let mut max_hops = None;
        let mut llm_provider = None;
        let mut include_explanation = false;

        let mut i = 2;
        while i < args.len() {
            if let Some(key) = args[i].as_string() {
                match key.to_uppercase().as_str() {
                    "MAX_HOPS" => {
                        if i + 1 < args.len() {
                            max_hops = args[i + 1].as_integer().map(|v| v as u32);
                            i += 2;
                            continue;
                        }
                    }
                    "LLM_PROVIDER" => {
                        if i + 1 < args.len() {
                            llm_provider = args[i + 1].as_string();
                            i += 2;
                            continue;
                        }
                    }
                    "INCLUDE_EXPLANATION" => {
                        include_explanation = true;
                        i += 1;
                        continue;
                    }
                    _ => {}
                }
            }
            i += 1;
        }

        let _actor = self.get_or_create_graphrag_actor(&kg_name).await?;

        let orbit_client = self.orbit_client.clone();

        let query = GraphRAGQuery {
            query_text,
            max_hops,
            context_size: None,
            llm_provider,
            search_strategy: None,
            include_explanation,
            max_results: Some(50),
        };

        // Note: GraphRAGActor methods require &mut self, so we need to handle this differently
        // For now, create a new actor instance for each operation
        // In production, this would use the actor system properly
        let mut actor_mut = GraphRAGActor::new(kg_name.clone());
        actor_mut.initialize_components();

        // Copy LLM providers if any were configured
        // (This is a limitation of the current design - actors should be managed by the actor system)

        let result = actor_mut
            .query_rag(orbit_client, query)
            .await
            .map_err(|e| ProtocolError::RespError(format!("ERR {}", e)))?;

        use bytes::Bytes;
        Ok(RespValue::Array(vec![
            RespValue::BulkString(Bytes::from("response")),
            RespValue::BulkString(Bytes::from(result.response.response)),
            RespValue::BulkString(Bytes::from("confidence")),
            RespValue::BulkString(Bytes::from(format!("{:.2}", result.response.confidence))),
            RespValue::BulkString(Bytes::from("processing_time_ms")),
            RespValue::Integer(result.processing_times.total_ms as i64),
        ]))
    }

    /// Handle GRAPHRAG.STATS command
    async fn handle_stats(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
        if args.is_empty() {
            return Err(ProtocolError::RespError(
                "ERR wrong number of arguments for 'GRAPHRAG.STATS'".to_string(),
            ));
        }

        let kg_name = args[0]
            .as_string()
            .ok_or_else(|| ProtocolError::RespError("ERR invalid kg_name".to_string()))?;

        let actor = self.get_or_create_graphrag_actor(&kg_name).await?;

        use bytes::Bytes;
        Ok(RespValue::Array(vec![
            RespValue::BulkString(Bytes::from("documents_processed")),
            RespValue::Integer(actor.stats.documents_processed as i64),
            RespValue::BulkString(Bytes::from("rag_queries_executed")),
            RespValue::Integer(actor.stats.rag_queries_executed as i64),
            RespValue::BulkString(Bytes::from("entities_extracted")),
            RespValue::Integer(actor.stats.entities_extracted as i64),
            RespValue::BulkString(Bytes::from("relationships_extracted")),
            RespValue::Integer(actor.stats.relationships_extracted as i64),
        ]))
    }
}

#[async_trait::async_trait]
impl CommandHandler for GraphRAGCommands {
    async fn handle(&self, command: &str, args: &[RespValue]) -> ProtocolResult<RespValue> {
        match command {
            "GRAPHRAG.BUILD" => self.handle_build(args).await,
            "GRAPHRAG.QUERY" => self.handle_query(args).await,
            "GRAPHRAG.STATS" => self.handle_stats(args).await,
            _ => Err(ProtocolError::RespError(format!(
                "ERR unknown GraphRAG command: {}",
                command
            ))),
        }
    }

    fn supported_commands(&self) -> &[&'static str] {
        &["GRAPHRAG.BUILD", "GRAPHRAG.QUERY", "GRAPHRAG.STATS"]
    }
}
