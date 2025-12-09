# Legal Industry Examples ⚖️

This directory contains end-to-end examples for a **Legal Practice Management & Research Platform** using Orbit-RS.

Key scenarios include case management, precedent citation graphs, and document discovery.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **Matters** | PostgreSQL | 5432 | Clients, matters (cases), billing, timekeeping (ACID) |
| **Research** | Cypher (Bolt) | 7687 | Case citation graph (Precedents, Relevant Law) |
| **Discovery** | AQL (ArangoDB) | 8529 | Discovery documents, full-text search, graph connections |

## 🚀 Running the Examples

### 1. Matter Management (PostgreSQL)
Core schema for clients, cases, and billing.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_legal_schema.sql
```

### 2. Citation Graph (Cypher)
Analyze relationships between court cases ("Cited By", "Overruled").

```bash
cypher-shell -a bolt://localhost:7687 -u orbit -p orbit -f cypher/01_case_graph.cypher
```

### 3. Document Discovery (AQL)
Search and manage evidence documents.

```bash
arangosh --server.endpoint tcp://localhost:8529 --javascript.execute aql/01_discovery_search.aql
```

## 📚 Workflows

- **[Conflict Check & Case Initiation](workflows/01_conflict_check.md)**: Checking relations graph before accepting a new client.
