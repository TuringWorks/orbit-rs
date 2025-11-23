# MCP and GraphRAG Completion Plan

**Date**: November 2025  
**Status**: Implementation In Progress

## Overview

This document outlines the completion plan for MCP (Model Context Protocol) and GraphRAG implementations according to the documentation and RFCs.

## Current Status

### MCP Implementation
- ✅ Core NLP components (NLP processor, SQL generator, result processor)
- ✅ Integration layer with Orbit-RS
- ⏳ Schema discovery (needs connection to storage)
- ⏳ MCP handlers (resources/read, prompts/get)
- ⏳ MCP server initialization in main.rs
- ⏳ ML model integration (framework ready, needs actual models)

### GraphRAG Implementation
- ✅ Core GraphRAG actor and components
- ✅ Protocol integrations (RESP, PostgreSQL, AQL, Cypher)
- ⏳ LLM provider integration (framework ready, needs actual LLM calls)
- ⏳ Vector search integration
- ⏳ Entity extraction enhancements (LLM-based, fuzzy matching)
- ⏳ GraphRAG initialization in main.rs

## Implementation Tasks

### Phase 1: MCP Completion (Priority: High)

#### Task 1.1: Complete Schema Discovery
- Connect SchemaAnalyzer to TieredTableStorage
- Implement discover_schema() to query actual storage
- Implement refresh_cache() to load all schemas
- Update schema cache on table changes

#### Task 1.2: Complete MCP Handlers
- Implement resources/read handler
- Implement prompts/get handler
- Add resource management
- Add prompt templates

#### Task 1.3: Initialize MCP Server
- Add MCP server initialization in main.rs
- Connect to PostgreSQL storage
- Connect to query engine
- Start MCP server on configured port

### Phase 2: GraphRAG Completion (Priority: High)

#### Task 2.1: Complete LLM Integration
- Implement actual LLM provider calls (OpenAI, Ollama, etc.)
- Add LLM response parsing
- Add error handling and retries
- Add streaming support

#### Task 2.2: Complete Vector Search Integration
- Connect to vector store actors
- Implement similarity search
- Add embedding generation
- Add vector indexing

#### Task 2.3: Complete Entity Extraction
- Implement LLM-based entity extraction
- Implement fuzzy matching
- Implement semantic similarity deduplication
- Add relationship inference

#### Task 2.4: Initialize GraphRAG
- Register GraphRAG actors in main.rs
- Initialize knowledge graphs
- Connect to protocol engines
- Start GraphRAG services

## Implementation Order

1. **MCP Schema Discovery** (Critical for MCP functionality)
2. **MCP Server Initialization** (Enables MCP endpoint)
3. **MCP Handlers** (Completes MCP protocol)
4. **GraphRAG LLM Integration** (Core GraphRAG functionality)
5. **GraphRAG Vector Search** (Enhances GraphRAG capabilities)
6. **GraphRAG Initialization** (Enables GraphRAG services)

## Success Criteria

### MCP
- ✅ MCP server starts and accepts connections
- ✅ Natural language queries execute successfully
- ✅ Schema discovery works with actual storage
- ✅ All MCP handlers respond correctly
- ✅ Integration with Orbit-RS is functional

### GraphRAG
- ✅ GraphRAG actors are registered and accessible
- ✅ Knowledge graph construction works
- ✅ RAG queries execute with LLM integration
- ✅ Vector search is integrated
- ✅ Entity extraction works with LLM and fuzzy matching

## Testing

- Unit tests for each component
- Integration tests for MCP → Orbit-RS flow
- Integration tests for GraphRAG → LLM flow
- End-to-end tests for natural language queries
- End-to-end tests for GraphRAG queries

