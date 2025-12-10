# Shared Graph Algorithms Specification

**Last Updated**: 2025-12-10
**Module**: `orbit/server/src/protocols/common/graph_algorithms.rs`
**Status**: Production Ready

## Overview

The shared graph algorithms module provides protocol-agnostic graph algorithm implementations that can be used by all OrbitRS protocols (AQL, Cypher, Redis, OrbitQL/PostgreSQL, etc.). This ensures consistency across protocols and eliminates code duplication.

## Protocol Integration Status

| Protocol | Integration Status | Helper Module/Function | Notes |
|----------|-------------------|------------------------|-------|
| **AQL** | ✅ Integrated | `build_graph_from_context()` | Full graph function support |
| **Cypher/Bolt** | ✅ Integrated | `build_shared_graph()` | Procedures use shared algos |
| **Redis RESP** | ✅ Integrated | `to_shared_graph()` | RedisGraph commands ready |
| **OrbitQL** | ✅ Integrated | `graph_traversal::execute_traverse()` | TRAVERSE clause execution |
| **CQL** | ✅ Integrated | `CqlGraphEngine` | DataStax-style graph extensions |
| **MongoDB** | ✅ Integrated | `MongoGraphEngine` | $graphLookup using BFS traversal |

## Architecture

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│     AQL     │  │   Cypher    │  │    Redis    │  │  OrbitQL    │  │     CQL     │  │   MongoDB   │
│  Protocol   │  │  Protocol   │  │  Protocol   │  │  Protocol   │  │  Protocol   │  │  Protocol   │
│     ✅      │  │     ✅      │  │     ✅      │  │     ✅      │  │     ✅      │  │     ✅      │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                │                │                │                │                │
       └────────────────┴────────────────┴────────────────┴────────────────┴────────────────┘
                                                   │
                                    ┌──────────────▼──────────────┐
                                    │    Shared Graph Algorithms   │
                                    │  protocols/common/graph_algo │
                                    │                              │
                                    │  Core Data Structures:       │
                                    │  - Graph (adjacency list)    │
                                    │  - GraphNode, GraphEdge      │
                                    │                              │
                                    │  Algorithm Categories:       │
                                    │  - Path Finding              │
                                    │  - Centrality                │
                                    │  - Community Detection       │
                                    │  - Similarity                │
                                    │  - Clustering                │
                                    │  - Graph Statistics          │
                                    └──────────────────────────────┘
```

## Data Structures

### Graph Representation

```rust
/// Core graph structure using adjacency list representation
pub struct Graph {
    /// All nodes: node_id -> GraphNode
    pub nodes: HashMap<String, GraphNode>,
    /// Outgoing edges: node_id -> [(neighbor_id, weight, edge_type)]
    pub adjacency: HashMap<String, Vec<(String, f64, Option<String>)>>,
    /// Incoming edges for reverse traversal
    pub reverse_adjacency: HashMap<String, Vec<(String, f64, Option<String>)>>,
}

pub struct GraphNode {
    pub id: String,
    pub properties: HashMap<String, serde_json::Value>,
}

pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub weight: f64,
    pub edge_type: Option<String>,
    pub properties: HashMap<String, serde_json::Value>,
}
```

## Algorithm Reference

### 1. Path Finding Algorithms

| Algorithm | Function | Time Complexity | Description |
|-----------|----------|-----------------|-------------|
| **Dijkstra** | `dijkstra(graph, source, target)` | O((V+E) log V) | Weighted shortest path |
| **BFS Shortest Path** | `bfs_shortest_path(graph, source, target)` | O(V+E) | Unweighted shortest path |
| **BFS Traversal** | `bfs_traversal(graph, source, max_depth)` | O(V+E) | Breadth-first traversal |
| **DFS Traversal** | `dfs_traversal(graph, source, max_depth)` | O(V+E) | Depth-first traversal |
| **All Shortest Paths** | `all_shortest_paths(graph, source, target)` | O(V!) worst | All minimum-length paths |
| **K Shortest Paths** | `k_shortest_paths(graph, source, target, k)` | O(K*V*(V+E)) | K shortest paths (Yen's algorithm) |

#### Result Structures

```rust
pub struct ShortestPathResult {
    pub path: Vec<String>,      // Node IDs in path
    pub cost: f64,              // Total path cost
    pub length: usize,          // Number of edges
    pub found: bool,            // Whether path exists
}

pub struct TraversalResult {
    pub visited: Vec<String>,              // Visited nodes in order
    pub depths: HashMap<String, usize>,    // Depth of each node
    pub parents: HashMap<String, String>,  // Parent in traversal tree
}

pub struct AllPathsResult {
    pub paths: Vec<Vec<String>>,  // All paths found
}

pub struct KShortestPathsResult {
    pub paths: Vec<ShortestPathResult>,  // K shortest paths
}
```

### 2. Centrality Algorithms

| Algorithm | Function | Description |
|-----------|----------|-------------|
| **PageRank** | `pagerank(graph, damping, max_iterations, tolerance)` | Link analysis ranking |
| **Degree Centrality** | `degree_centrality(graph)` | In/out/total degree for all nodes |

#### PageRank Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `damping` | f64 | 0.85 | Damping factor (probability of following links) |
| `max_iterations` | usize | 100 | Maximum iterations |
| `tolerance` | f64 | 1e-6 | Convergence threshold |

#### Result Structures

```rust
pub struct PageRankResult {
    pub scores: HashMap<String, f64>,  // Node ID -> PageRank score
    pub iterations: usize,             // Iterations performed
    pub converged: bool,               // Whether converged within tolerance
}

pub struct DegreeCentralityResult {
    pub in_degree: HashMap<String, usize>,
    pub out_degree: HashMap<String, usize>,
    pub total_degree: HashMap<String, usize>,
}
```

### 3. Connected Components

| Algorithm | Function | Description |
|-----------|----------|-------------|
| **Connected Components** | `connected_components(graph)` | Undirected components (BFS) |
| **Strongly Connected** | `strongly_connected_components(graph)` | Directed components (Tarjan's) |

```rust
pub struct ConnectedComponentsResult {
    pub component_ids: HashMap<String, usize>,  // Node -> component ID
    pub num_components: usize,                   // Total components
    pub components: Vec<Vec<String>>,            // Nodes in each component
}
```

### 4. Similarity Algorithms

| Algorithm | Function | Description |
|-----------|----------|-------------|
| **Jaccard Similarity** | `jaccard_similarity(graph, node1, node2)` | Neighbor overlap ratio |
| **Adamic-Adar** | `adamic_adar(graph, node1, node2)` | Link prediction score |
| **Preferential Attachment** | `preferential_attachment(graph, node1, node2)` | Degree product |
| **Common Neighbors** | `common_neighbors(graph, node1, node2)` | Shared neighbor list |

### 5. Clustering and Triangle Analysis

| Algorithm | Function | Description |
|-----------|----------|-------------|
| **Triangle Count (Node)** | `triangle_count_node(graph, node_id)` | Triangles containing node |
| **Triangle Count (Total)** | `triangle_count_total(graph)` | Total triangles in graph |
| **Clustering Coefficient** | `clustering_coefficient(graph, node_id)` | Local clustering measure |

### 6. Graph Statistics

```rust
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub density: f64,           // edge_count / (node_count * (node_count - 1))
    pub avg_degree: f64,        // (2 * edge_count) / node_count
    pub max_in_degree: usize,
    pub max_out_degree: usize,
}
```

### 7. Neighbor Queries

| Function | Description |
|----------|-------------|
| `get_neighbors(graph, node_id, direction)` | Get neighbors with edge info |

```rust
pub enum NeighborDirection {
    Outgoing,  // Outbound edges only
    Incoming,  // Inbound edges only
    Both,      // All edges (undirected view)
}

pub struct NeighborsResult {
    pub neighbors: Vec<String>,     // Neighbor IDs
    pub edges: Vec<GraphEdge>,      // Edge details
}
```

## Protocol Integration

### AQL Protocol

The AQL protocol integrates via `build_graph_from_context()`:

```rust
// In evaluate_builtin_function()
"SHORTEST_PATH" => {
    let graph = self.build_graph_from_context(context, None);
    let result = graph_algo::dijkstra(&graph, start_id, target_id);
    // Format and return result
}
```

**Supported AQL Functions:**
- `SHORTEST_PATH(start, target, options)`
- `K_SHORTEST_PATHS(start, target, k, options)`
- `ALL_SHORTEST_PATHS(start, target, options)`
- `GRAPH_VERTICES(graphName, start, options)`
- `GRAPH_EDGES(graphName, start, options)`
- `GRAPH_NEIGHBORS(graphName, start, options)`
- `GRAPH_COMMON_NEIGHBORS(graphName, v1, v2, options)`
- `GRAPH_PATHS(graphName, options)`
- `GRAPH_SHORTEST_PATH(graphName, start, target, options)`
- `GRAPH_DISTANCE_TO(graphName, start, target, options)`

### Cypher Protocol

The Cypher protocol integrates via `build_shared_graph()`:

```rust
// In GraphAlgorithmProcedures
async fn build_shared_graph(&self) -> ProtocolResult<graph_algo::Graph> {
    let nodes = self.get_all_nodes().await?;
    let relationships = self.get_all_relationships().await?;
    // Build and return shared graph
}

// Usage in procedures
async fn execute_shortest_path_cpu(&self, from_id: &str, to_id: &str, weighted: bool) {
    let graph = self.build_shared_graph().await?;
    let result = if weighted {
        graph_algo::dijkstra(&graph, from_id, to_id)
    } else {
        graph_algo::bfs_shortest_path(&graph, from_id, to_id)
    };
    // Format and return result
}
```

**Supported Cypher Procedures:**
- `CALL orbit.graph.shortestPath(from, to, {weighted: bool})`
- `CALL orbit.graph.bfs(start, {maxDepth: n})`
- `CALL orbit.graph.dfs(start, {maxDepth: n})`
- `CALL orbit.graph.pagerank({damping, iterations, tolerance})`

### Redis Protocol

The Redis protocol integrates via `to_shared_graph()`:

```rust
impl Graph {
    fn to_shared_graph(&self) -> graph_algo::Graph {
        let mut shared = graph_algo::Graph::new();
        for (id, node) in &self.nodes {
            shared.add_node(id.to_string(), node.properties.clone());
        }
        for rel in self.relationships.values() {
            shared.add_edge(rel.src_id.to_string(), rel.dest_id.to_string(), weight, type);
        }
        shared
    }
}
```

### OrbitQL Protocol

The OrbitQL/PostgresWire protocol integrates via the `graph_traversal` module:

**Module:** `protocols/postgres_wire/sql/graph_traversal.rs`

```rust
// Build graph from table data
let builder = OrbitQLGraphBuilder::new(&node_table, &edge_table);
let graph = builder.build_graph(&node_data, &edge_data)?;

// Execute TRAVERSE clause
let result = graph_traversal::execute_traverse(&graph, &start_nodes, traverse_clause)?;

// Execute shortest path queries
let result = graph_traversal::execute_shortest_path(&graph, from, to, weighted)?;
```

**Supported OrbitQL Features:**
- `TRAVERSE OUTBOUND 1..5 STEPS ON edges` - BFS/DFS traversal
- `TRAVERSE INBOUND 1..3 STEPS ON relationships` - Reverse traversal
- `TRAVERSE ANY 1..10 STEPS ON connections` - Bidirectional traversal
- Automatic depth tracking with `_depth` column
- Integration with WHERE clause for start node filtering

### CQL Protocol (Cassandra)

The CQL protocol integrates via `CqlGraphEngine`:

**Module:** `protocols/cql/graph.rs`

```rust
// Create and configure graph engine
let mut engine = CqlGraphEngine::new();
engine.build_from_tables(&vertices, &edges, "id", Some("label"), "from", "to", ...)?;

// Execute graph queries
let result = engine.execute(&GraphQuery::ShortestPath {
    from_vertex: "v1".to_string(),
    to_vertex: "v2".to_string(),
    weighted: false,
})?;
```

**Supported CQL Graph Queries:**
- `GraphQuery::Traverse` - BFS traversal with direction and depth
- `GraphQuery::ShortestPath` - Dijkstra/BFS shortest path
- `GraphQuery::AllShortestPaths` - All minimum-length paths
- `GraphQuery::PageRank` - PageRank centrality scores
- `GraphQuery::Neighbors` - Direct neighbor queries
- `GraphQuery::ConnectedComponents` - Component analysis
- `GraphQuery::StronglyConnectedComponents` - SCC analysis

### MongoDB Protocol

The MongoDB protocol integrates via `MongoGraphEngine` for `$graphLookup`:

**Module:** `protocols/mongodb/graph.rs`

```rust
// Build graph from documents
let mut engine = MongoGraphEngine::new();
engine.build_from_documents(&docs, "reports_to", "_id");

// Execute $graphLookup
let config = GraphLookupConfig {
    from: "employees".to_string(),
    connect_from_field: "reports_to".to_string(),
    connect_to_field: "_id".to_string(),
    as_field: "hierarchy".to_string(),
    max_depth: Some(5),
    depth_field: Some("depth".to_string()),
    ...
};
let result = engine.graph_lookup(&start_value, &config);

// Additional graph operations
let path = engine.shortest_path("from_id", "to_id", weighted);
let neighbors = engine.get_neighbors("node_id", NeighborDirection::Out);
```

**Supported MongoDB Operations:**
- `$graphLookup` aggregation stage - Recursive graph traversal
- `shortest_path()` - Path finding between documents
- `get_neighbors()` - Direct neighbor queries
- Support for `restrictSearchWithMatch` filtering
- Automatic `depthField` population

## Performance Characteristics

| Algorithm | Best Case | Average Case | Worst Case | Space |
|-----------|-----------|--------------|------------|-------|
| Dijkstra | O(E log V) | O((V+E) log V) | O((V+E) log V) | O(V) |
| BFS | O(V+E) | O(V+E) | O(V+E) | O(V) |
| DFS | O(V+E) | O(V+E) | O(V+E) | O(V) |
| PageRank | O(I*(V+E)) | O(I*(V+E)) | O(I*(V+E)) | O(V) |
| Connected Components | O(V+E) | O(V+E) | O(V+E) | O(V) |
| SCC (Tarjan) | O(V+E) | O(V+E) | O(V+E) | O(V) |
| K Shortest Paths | O(K*V*(V+E)) | O(K*V*(V+E)) | O(K*V*(V+E)) | O(K*V) |

Where: V = vertices, E = edges, I = iterations (PageRank)

## Testing

The module includes comprehensive unit tests in the same file:

```rust
#[cfg(test)]
mod tests {
    #[test] fn test_bfs_shortest_path() { ... }
    #[test] fn test_dijkstra() { ... }
    #[test] fn test_bfs_traversal() { ... }
    #[test] fn test_common_neighbors() { ... }
    // Additional tests...
}
```

Run tests with:
```bash
cargo test -p orbit-server graph_algorithms
```

## Future Enhancements

### Planned Algorithms
- [ ] A* pathfinding with heuristics
- [ ] Betweenness centrality
- [ ] Closeness centrality
- [ ] Eigenvector centrality
- [ ] Louvain community detection
- [ ] Label propagation clustering
- [ ] K-core decomposition
- [ ] Random walk

### GPU Acceleration

For large-scale graphs, GPU-accelerated versions are available in `orbit-compute`:
- `orbit_compute::graph_traversal::bfs()` - GPU BFS (Metal/Vulkan)
- `orbit_compute::graph_traversal::dijkstra()` - GPU Dijkstra
- `orbit_compute::graph_traversal::pagerank()` - GPU PageRank

## References

- Dijkstra, E. W. (1959). "A note on two problems in connexion with graphs"
- Page, L., et al. (1999). "The PageRank Citation Ranking"
- Tarjan, R. (1972). "Depth-first search and linear graph algorithms"
- Yen, J. Y. (1971). "Finding the K Shortest Loopless Paths in a Network"
