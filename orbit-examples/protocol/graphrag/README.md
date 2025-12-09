# GraphRAG Examples

This directory contains examples demonstrating **Graph Retrieval Augmented Generation (GraphRAG)**, combining the power of Knowledge Graphs with Vector Search and LLMs.

## Structure

### [`aql/`](aql/)
GraphRAG implementations using **ArangoDB AQL**, combining document search with graph traversals.

### [`cypher/`](cypher/)
GraphRAG implementations using **Cypher** (Neo4j/Bolt), focusing on graph-native retrieval patterns.

### [`sql/`](sql/)
Implementations using **SQL** with vector extensions (pgvector style) linked to relational knowledge bases.

### [`python/`](python/)
Client-side scripts demonstrating how to orchestrate GraphRAG workflows using Python drivers.

## Use Cases

- **Context-Aware Chatbots**: Retrieving entities and their relationships to answer complex questions.
- **Semantic Search**: Finding documents based on meaning rather than keywords, enhanced by graph context.
- **Knowledge Discovery**: uncovering hidden connections in unstructured data.
