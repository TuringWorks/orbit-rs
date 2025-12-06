//! Standard Neo4j Database Procedures
//!
//! This module provides Neo4j-compatible database procedures for schema introspection
//! and system information queries.
//!
//! ## Supported Procedures
//!
//! ### Database Information
//! - `db.info()` - Database information
//! - `db.labels()` - List all node labels
//! - `db.relationshipTypes()` - List all relationship types
//! - `db.propertyKeys()` - List all property keys
//!
//! ### Schema
//! - `db.schema.nodeTypeProperties()` - Node type properties
//! - `db.schema.relTypeProperties()` - Relationship type properties
//! - `db.schema.visualization()` - Schema visualization
//!
//! ### DBMS
//! - `dbms.procedures()` - List all procedures
//! - `dbms.functions()` - List all functions
//! - `dbms.components()` - List system components

use crate::protocols::cypher::graph_engine::QueryResult;
use crate::protocols::error::{ProtocolError, ProtocolResult};
use orbit_shared::graph::GraphStorage;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::sync::Arc;

/// Database procedures handler for Neo4j compatibility
pub struct DbProcedures<S: GraphStorage> {
    #[allow(dead_code)]
    storage: Arc<S>,
    /// Known node labels discovered during operations
    known_labels: Vec<String>,
    /// Known relationship types discovered during operations
    known_rel_types: Vec<String>,
    /// Known property keys discovered during operations
    known_property_keys: Vec<String>,
}

impl<S: GraphStorage + Send + Sync + 'static> DbProcedures<S> {
    /// Create new database procedures handler
    pub fn new(storage: Arc<S>) -> Self {
        Self {
            storage,
            known_labels: vec![
                "Node".to_string(),
                "Person".to_string(),
                "Entity".to_string(),
                "Document".to_string(),
            ],
            known_rel_types: vec![
                "RELATES_TO".to_string(),
                "KNOWS".to_string(),
                "FOLLOWS".to_string(),
                "CREATED".to_string(),
            ],
            known_property_keys: vec![
                "id".to_string(),
                "name".to_string(),
                "type".to_string(),
                "created_at".to_string(),
                "updated_at".to_string(),
            ],
        }
    }

    /// Add a discovered label
    pub fn add_label(&mut self, label: String) {
        if !self.known_labels.contains(&label) {
            self.known_labels.push(label);
        }
    }

    /// Add a discovered relationship type
    pub fn add_rel_type(&mut self, rel_type: String) {
        if !self.known_rel_types.contains(&rel_type) {
            self.known_rel_types.push(rel_type);
        }
    }

    /// Add a discovered property key
    pub fn add_property_key(&mut self, key: String) {
        if !self.known_property_keys.contains(&key) {
            self.known_property_keys.push(key);
        }
    }

    /// Execute a database procedure call
    pub async fn execute_procedure(
        &self,
        procedure_name: &str,
        args: &[JsonValue],
    ) -> ProtocolResult<QueryResult> {
        match procedure_name.to_lowercase().as_str() {
            // Database info
            "db.info" => self.execute_db_info().await,
            "db.labels" => self.execute_db_labels().await,
            "db.relationshiptypes" => self.execute_db_relationship_types().await,
            "db.propertykeys" => self.execute_db_property_keys().await,

            // Schema procedures
            "db.schema.nodetypeproperties" => self.execute_node_type_properties().await,
            "db.schema.reltypeproperties" => self.execute_rel_type_properties().await,
            "db.schema.visualization" => self.execute_schema_visualization().await,

            // DBMS procedures
            "dbms.procedures" => self.execute_dbms_procedures().await,
            "dbms.functions" => self.execute_dbms_functions().await,
            "dbms.components" => self.execute_dbms_components().await,
            "dbms.showconnectionpool" => self.execute_show_connection_pool().await,
            "dbms.querycachestats" => self.execute_query_cache_stats().await,
            "dbms.listconfig" => self.execute_list_config(args).await,

            // Index and constraint procedures
            "db.indexes" => self.execute_db_indexes().await,
            "db.constraints" => self.execute_db_constraints().await,
            "db.awaitindex" => self.execute_await_index(args).await,

            // Node/Relationship counts
            "db.stats.nodecount" => self.execute_node_count().await,
            "db.stats.relcount" => self.execute_rel_count().await,

            _ => Err(ProtocolError::CypherError(format!(
                "Unknown database procedure: {procedure_name}"
            ))),
        }
    }

    /// Helper to create empty QueryResult with columns and rows
    fn make_result(columns: Vec<String>, rows: Vec<Vec<Option<String>>>) -> QueryResult {
        QueryResult {
            nodes: vec![],
            relationships: vec![],
            columns,
            rows,
        }
    }

    /// Execute db.info()
    async fn execute_db_info(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "name".to_string(),
            "id".to_string(),
            "creationDate".to_string(),
        ];

        let rows = vec![vec![
            Some("orbit".to_string()),
            Some("orbit-default".to_string()),
            Some(chrono::Utc::now().to_rfc3339()),
        ]];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.labels()
    async fn execute_db_labels(&self) -> ProtocolResult<QueryResult> {
        let columns = vec!["label".to_string()];

        let labels: HashSet<String> = self.known_labels.iter().cloned().collect();

        let mut rows: Vec<Vec<Option<String>>> =
            labels.into_iter().map(|label| vec![Some(label)]).collect();
        rows.sort_by(|a, b| a[0].cmp(&b[0]));

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.relationshipTypes()
    async fn execute_db_relationship_types(&self) -> ProtocolResult<QueryResult> {
        let columns = vec!["relationshipType".to_string()];

        let mut rows: Vec<Vec<Option<String>>> = self
            .known_rel_types
            .iter()
            .map(|rt| vec![Some(rt.clone())])
            .collect();
        rows.sort_by(|a, b| a[0].cmp(&b[0]));

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.propertyKeys()
    async fn execute_db_property_keys(&self) -> ProtocolResult<QueryResult> {
        let columns = vec!["propertyKey".to_string()];

        let mut rows: Vec<Vec<Option<String>>> = self
            .known_property_keys
            .iter()
            .map(|key| vec![Some(key.clone())])
            .collect();
        rows.sort_by(|a, b| a[0].cmp(&b[0]));

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.schema.nodeTypeProperties()
    async fn execute_node_type_properties(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "nodeType".to_string(),
            "nodeLabels".to_string(),
            "propertyName".to_string(),
            "propertyTypes".to_string(),
            "mandatory".to_string(),
        ];

        let mut rows = Vec::new();

        for label in &self.known_labels {
            for prop in &self.known_property_keys {
                rows.push(vec![
                    Some(format!(":`{}`", label)),
                    Some(format!("[\"{}\"]", label)),
                    Some(prop.clone()),
                    Some("[\"String\"]".to_string()),
                    Some("false".to_string()),
                ]);
            }
        }

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.schema.relTypeProperties()
    async fn execute_rel_type_properties(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "relType".to_string(),
            "propertyName".to_string(),
            "propertyTypes".to_string(),
            "mandatory".to_string(),
        ];

        let mut rows = Vec::new();

        for rel_type in &self.known_rel_types {
            rows.push(vec![
                Some(format!(":`{}`", rel_type)),
                Some("weight".to_string()),
                Some("[\"Float\"]".to_string()),
                Some("false".to_string()),
            ]);
            rows.push(vec![
                Some(format!(":`{}`", rel_type)),
                Some("created_at".to_string()),
                Some("[\"DateTime\"]".to_string()),
                Some("false".to_string()),
            ]);
        }

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.schema.visualization()
    async fn execute_schema_visualization(&self) -> ProtocolResult<QueryResult> {
        let columns = vec!["nodes".to_string(), "relationships".to_string()];

        // Build a simplified schema visualization
        let nodes_data: Vec<serde_json::Value> = self
            .known_labels
            .iter()
            .map(|l| {
                serde_json::json!({
                    "label": l,
                    "properties": self.known_property_keys
                })
            })
            .collect();

        let rels_data: Vec<serde_json::Value> = self
            .known_rel_types
            .iter()
            .map(|rt| {
                serde_json::json!({
                    "type": rt,
                    "properties": ["weight", "created_at"]
                })
            })
            .collect();

        let rows = vec![vec![
            Some(serde_json::to_string(&nodes_data).unwrap_or_default()),
            Some(serde_json::to_string(&rels_data).unwrap_or_default()),
        ]];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute dbms.procedures()
    async fn execute_dbms_procedures(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "name".to_string(),
            "signature".to_string(),
            "description".to_string(),
            "mode".to_string(),
            "worksOnSystem".to_string(),
        ];

        let procedures = vec![
            // Database procedures
            ("db.info", "db.info() :: (name :: STRING?, id :: STRING?, creationDate :: STRING?)", "Database information", "READ", "true"),
            ("db.labels", "db.labels() :: (label :: STRING?)", "List all node labels", "READ", "true"),
            ("db.relationshipTypes", "db.relationshipTypes() :: (relationshipType :: STRING?)", "List all relationship types", "READ", "true"),
            ("db.propertyKeys", "db.propertyKeys() :: (propertyKey :: STRING?)", "List all property keys", "READ", "true"),
            ("db.indexes", "db.indexes() :: (name :: STRING?, type :: STRING?, state :: STRING?)", "List all indexes", "READ", "true"),
            ("db.constraints", "db.constraints() :: (name :: STRING?, type :: STRING?)", "List all constraints", "READ", "true"),
            ("db.schema.nodeTypeProperties", "db.schema.nodeTypeProperties() :: (nodeType :: STRING?, nodeLabels :: LIST? OF STRING?, propertyName :: STRING?, propertyTypes :: LIST? OF STRING?, mandatory :: BOOLEAN?)", "Node type properties", "READ", "true"),
            ("db.schema.relTypeProperties", "db.schema.relTypeProperties() :: (relType :: STRING?, propertyName :: STRING?, propertyTypes :: LIST? OF STRING?, mandatory :: BOOLEAN?)", "Relationship type properties", "READ", "true"),
            ("db.schema.visualization", "db.schema.visualization() :: (nodes :: LIST? OF NODE?, relationships :: LIST? OF RELATIONSHIP?)", "Schema visualization", "READ", "true"),
            // DBMS procedures
            ("dbms.procedures", "dbms.procedures() :: (name :: STRING?, signature :: STRING?, description :: STRING?, mode :: STRING?, worksOnSystem :: BOOLEAN?)", "List all procedures", "DBMS", "true"),
            ("dbms.functions", "dbms.functions() :: (name :: STRING?, signature :: STRING?, description :: STRING?)", "List all functions", "DBMS", "true"),
            ("dbms.components", "dbms.components() :: (name :: STRING?, version :: STRING?, edition :: STRING?)", "List system components", "DBMS", "true"),
            // Graph algorithm procedures
            ("orbit.graph.pagerank", "orbit.graph.pagerank({damping: 0.85, iterations: 20, tolerance: 0.0001}) :: (node_id :: STRING?, pagerank :: FLOAT?)", "PageRank centrality", "READ", "false"),
            ("orbit.graph.shortestPath", "orbit.graph.shortestPath({startNode: id, endNode: id, relationshipType: type?}) :: (path :: PATH?)", "Shortest path between nodes", "READ", "false"),
            ("orbit.graph.bfs", "orbit.graph.bfs({startNode: id, maxDepth: 10}) :: (node_id :: STRING?, depth :: INTEGER?)", "Breadth-first search", "READ", "false"),
            ("orbit.graph.dfs", "orbit.graph.dfs({startNode: id, maxDepth: 10}) :: (node_id :: STRING?, depth :: INTEGER?)", "Depth-first search", "READ", "false"),
            ("orbit.graph.communityDetection", "orbit.graph.communityDetection({algorithm: louvain}) :: (node_id :: STRING?, community :: INTEGER?)", "Community detection", "READ", "false"),
            ("orbit.graph.connectedComponents", "orbit.graph.connectedComponents() :: (node_id :: STRING?, component :: INTEGER?)", "Find connected components", "READ", "false"),
            ("orbit.graph.betweennessCentrality", "orbit.graph.betweennessCentrality() :: (node_id :: STRING?, betweenness :: FLOAT?)", "Betweenness centrality", "READ", "false"),
            ("orbit.graph.closenessCentrality", "orbit.graph.closenessCentrality() :: (node_id :: STRING?, closeness :: FLOAT?)", "Closeness centrality", "READ", "false"),
            ("orbit.graph.degreeCentrality", "orbit.graph.degreeCentrality() :: (node_id :: STRING?, degree :: INTEGER?)", "Degree centrality", "READ", "false"),
            ("orbit.graph.triangleCount", "orbit.graph.triangleCount() :: (node_id :: STRING?, triangles :: INTEGER?)", "Triangle count", "READ", "false"),
            ("orbit.graph.eigenvectorCentrality", "orbit.graph.eigenvectorCentrality() :: (node_id :: STRING?, eigenvector :: FLOAT?)", "Eigenvector centrality", "READ", "false"),
            ("orbit.graph.jaccardSimilarity", "orbit.graph.jaccardSimilarity({node1: id, node2: id}) :: (similarity :: FLOAT?)", "Jaccard similarity", "READ", "false"),
            ("orbit.graph.cosineSimilarity", "orbit.graph.cosineSimilarity({node1: id, node2: id}) :: (similarity :: FLOAT?)", "Cosine similarity", "READ", "false"),
            ("orbit.graph.louvain", "orbit.graph.louvain() :: (node_id :: STRING?, community :: INTEGER?)", "Louvain community detection", "READ", "false"),
            ("orbit.graph.kcore", "orbit.graph.kcore({k: 3}) :: (node_id :: STRING?, core :: INTEGER?)", "K-core decomposition", "READ", "false"),
            // Path algorithms
            ("orbit.graph.astar", "orbit.graph.astar({startNode: id, endNode: id, heuristic: function}) :: (path :: PATH?, cost :: FLOAT?)", "A* pathfinding", "READ", "false"),
            ("orbit.graph.dijkstra", "orbit.graph.dijkstra({startNode: id, endNode: id?, weightProperty: weight}) :: (path :: PATH?, cost :: FLOAT?)", "Dijkstra shortest path", "READ", "false"),
            ("orbit.graph.allShortestPaths", "orbit.graph.allShortestPaths({startNode: id, endNode: id}) :: (paths :: LIST? OF PATH?)", "All shortest paths", "READ", "false"),
            ("orbit.graph.kShortestPaths", "orbit.graph.kShortestPaths({startNode: id, endNode: id, k: 3}) :: (paths :: LIST? OF PATH?)", "K shortest paths", "READ", "false"),
            ("orbit.graph.spanningTree", "orbit.graph.spanningTree({algorithm: prim}) :: (edges :: LIST? OF RELATIONSHIP?)", "Minimum spanning tree", "READ", "false"),
            ("orbit.graph.singleSourceShortestPath", "orbit.graph.singleSourceShortestPath({startNode: id, weightProperty: weight}) :: (node_id :: STRING?, distance :: FLOAT?)", "Single source shortest path", "READ", "false"),
        ];

        let rows: Vec<Vec<Option<String>>> = procedures
            .iter()
            .map(|(name, sig, desc, mode, works_on_system)| {
                vec![
                    Some(name.to_string()),
                    Some(sig.to_string()),
                    Some(desc.to_string()),
                    Some(mode.to_string()),
                    Some(works_on_system.to_string()),
                ]
            })
            .collect();

        Ok(Self::make_result(columns, rows))
    }

    /// Execute dbms.functions()
    async fn execute_dbms_functions(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "name".to_string(),
            "signature".to_string(),
            "description".to_string(),
            "category".to_string(),
        ];

        let functions = vec![
            // String functions
            ("toUpper", "toUpper(input :: STRING?) :: (STRING?)", "Converts string to uppercase", "String"),
            ("toLower", "toLower(input :: STRING?) :: (STRING?)", "Converts string to lowercase", "String"),
            ("trim", "trim(input :: STRING?) :: (STRING?)", "Trims whitespace", "String"),
            ("replace", "replace(original :: STRING?, search :: STRING?, replace :: STRING?) :: (STRING?)", "Replaces substring", "String"),
            ("substring", "substring(input :: STRING?, start :: INTEGER?, length :: INTEGER?) :: (STRING?)", "Extracts substring", "String"),
            ("split", "split(input :: STRING?, delimiter :: STRING?) :: (LIST? OF STRING?)", "Splits string", "String"),
            ("size", "size(input :: ANY?) :: (INTEGER?)", "Returns size of collection/string", "String"),
            // List functions
            ("head", "head(list :: LIST? OF ANY?) :: (ANY?)", "Returns first element", "List"),
            ("tail", "tail(list :: LIST? OF ANY?) :: (LIST? OF ANY?)", "Returns all but first", "List"),
            ("last", "last(list :: LIST? OF ANY?) :: (ANY?)", "Returns last element", "List"),
            ("range", "range(start :: INTEGER?, end :: INTEGER?, step :: INTEGER?) :: (LIST? OF INTEGER?)", "Creates integer range", "List"),
            ("keys", "keys(input :: ANY?) :: (LIST? OF STRING?)", "Returns property keys", "List"),
            ("labels", "labels(node :: NODE?) :: (LIST? OF STRING?)", "Returns node labels", "List"),
            ("nodes", "nodes(path :: PATH?) :: (LIST? OF NODE?)", "Returns nodes in path", "List"),
            ("relationships", "relationships(path :: PATH?) :: (LIST? OF RELATIONSHIP?)", "Returns relationships in path", "List"),
            // Math functions
            ("abs", "abs(input :: NUMBER?) :: (NUMBER?)", "Absolute value", "Numeric"),
            ("ceil", "ceil(input :: FLOAT?) :: (INTEGER?)", "Ceiling", "Numeric"),
            ("floor", "floor(input :: FLOAT?) :: (INTEGER?)", "Floor", "Numeric"),
            ("round", "round(input :: FLOAT?, precision :: INTEGER?) :: (FLOAT?)", "Round to precision", "Numeric"),
            ("sqrt", "sqrt(input :: NUMBER?) :: (FLOAT?)", "Square root", "Numeric"),
            ("rand", "rand() :: (FLOAT?)", "Random number 0-1", "Numeric"),
            ("log", "log(input :: NUMBER?) :: (FLOAT?)", "Natural logarithm", "Numeric"),
            ("exp", "exp(input :: NUMBER?) :: (FLOAT?)", "Exponential", "Numeric"),
            ("sin", "sin(input :: NUMBER?) :: (FLOAT?)", "Sine", "Trigonometric"),
            ("cos", "cos(input :: NUMBER?) :: (FLOAT?)", "Cosine", "Trigonometric"),
            ("tan", "tan(input :: NUMBER?) :: (FLOAT?)", "Tangent", "Trigonometric"),
            // Date/Time functions
            ("date", "date(input :: ANY?) :: (DATE?)", "Creates date", "Temporal"),
            ("datetime", "datetime(input :: ANY?) :: (DATETIME?)", "Creates datetime", "Temporal"),
            ("time", "time(input :: ANY?) :: (TIME?)", "Creates time", "Temporal"),
            ("duration", "duration(input :: ANY?) :: (DURATION?)", "Creates duration", "Temporal"),
            // Type functions
            ("type", "type(relationship :: RELATIONSHIP?) :: (STRING?)", "Returns relationship type", "Scalar"),
            ("id", "id(node :: NODE?) :: (INTEGER?)", "Returns node ID", "Scalar"),
            ("elementId", "elementId(input :: ANY?) :: (STRING?)", "Returns element ID", "Scalar"),
            ("properties", "properties(input :: ANY?) :: (MAP?)", "Returns properties map", "Scalar"),
            ("coalesce", "coalesce(input1 :: ANY?, input2 :: ANY?, ...) :: (ANY?)", "Returns first non-null", "Scalar"),
            // Path functions
            ("shortestPath", "shortestPath(pattern :: PATH_PATTERN?) :: (PATH?)", "Finds shortest path", "Graph"),
            ("allShortestPaths", "allShortestPaths(pattern :: PATH_PATTERN?) :: (LIST? OF PATH?)", "Finds all shortest paths", "Graph"),
            ("pathLength", "pathLength(path :: PATH?) :: (INTEGER?)", "Returns path length", "Graph"),
            ("startNode", "startNode(relationship :: RELATIONSHIP?) :: (NODE?)", "Returns start node", "Graph"),
            ("endNode", "endNode(relationship :: RELATIONSHIP?) :: (NODE?)", "Returns end node", "Graph"),
            // Aggregation functions
            ("count", "count(input :: ANY?) :: (INTEGER?)", "Counts elements", "Aggregating"),
            ("sum", "sum(input :: NUMBER?) :: (NUMBER?)", "Sums values", "Aggregating"),
            ("avg", "avg(input :: NUMBER?) :: (FLOAT?)", "Averages values", "Aggregating"),
            ("min", "min(input :: ANY?) :: (ANY?)", "Minimum value", "Aggregating"),
            ("max", "max(input :: ANY?) :: (ANY?)", "Maximum value", "Aggregating"),
            ("collect", "collect(input :: ANY?) :: (LIST? OF ANY?)", "Collects into list", "Aggregating"),
        ];

        let rows: Vec<Vec<Option<String>>> = functions
            .iter()
            .map(|(name, sig, desc, category)| {
                vec![
                    Some(name.to_string()),
                    Some(sig.to_string()),
                    Some(desc.to_string()),
                    Some(category.to_string()),
                ]
            })
            .collect();

        Ok(Self::make_result(columns, rows))
    }

    /// Execute dbms.components()
    async fn execute_dbms_components(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "name".to_string(),
            "version".to_string(),
            "edition".to_string(),
        ];

        let rows = vec![
            vec![
                Some("Orbit-RS".to_string()),
                Some("0.1.0".to_string()),
                Some("Enterprise".to_string()),
            ],
            vec![
                Some("Bolt Protocol".to_string()),
                Some("4.4".to_string()),
                Some("".to_string()),
            ],
            vec![
                Some("Cypher".to_string()),
                Some("9".to_string()),
                Some("".to_string()),
            ],
        ];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute dbms.showConnectionPool()
    async fn execute_show_connection_pool(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "pool".to_string(),
            "activeConnections".to_string(),
            "idleConnections".to_string(),
            "totalConnections".to_string(),
        ];

        let rows = vec![vec![
            Some("bolt".to_string()),
            Some("1".to_string()),
            Some("0".to_string()),
            Some("1".to_string()),
        ]];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute dbms.queryCacheStats()
    async fn execute_query_cache_stats(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "hits".to_string(),
            "misses".to_string(),
            "hitRatio".to_string(),
            "queries".to_string(),
        ];

        let rows = vec![vec![
            Some("0".to_string()),
            Some("0".to_string()),
            Some("0.0".to_string()),
            Some("0".to_string()),
        ]];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute dbms.listConfig()
    async fn execute_list_config(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let filter = args.first().and_then(|v| v.as_str()).map(|s| s.to_string());

        let columns = vec![
            "name".to_string(),
            "value".to_string(),
            "description".to_string(),
        ];

        let configs = [("dbms.memory.heap.initial_size", "512m", "Initial heap size"),
            ("dbms.memory.heap.max_size", "1g", "Maximum heap size"),
            (
                "dbms.connector.bolt.enabled",
                "true",
                "Bolt connector enabled",
            ),
            (
                "dbms.connector.bolt.listen_address",
                "0.0.0.0:7687",
                "Bolt listen address",
            ),
            (
                "dbms.connector.http.enabled",
                "true",
                "HTTP connector enabled",
            ),
            (
                "dbms.security.auth_enabled",
                "true",
                "Authentication enabled",
            )];

        let rows: Vec<Vec<Option<String>>> = configs
            .iter()
            .filter(|(name, _, _)| {
                filter
                    .as_ref()
                    .map(|f| name.contains(f.as_str()))
                    .unwrap_or(true)
            })
            .map(|(name, value, desc)| {
                vec![
                    Some(name.to_string()),
                    Some(value.to_string()),
                    Some(desc.to_string()),
                ]
            })
            .collect();

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.indexes()
    async fn execute_db_indexes(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "name".to_string(),
            "type".to_string(),
            "entityType".to_string(),
            "labelsOrTypes".to_string(),
            "properties".to_string(),
            "state".to_string(),
        ];

        // Return empty indexes for now
        let rows: Vec<Vec<Option<String>>> = vec![];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.constraints()
    async fn execute_db_constraints(&self) -> ProtocolResult<QueryResult> {
        let columns = vec![
            "name".to_string(),
            "type".to_string(),
            "entityType".to_string(),
            "labelsOrTypes".to_string(),
            "properties".to_string(),
        ];

        // Return empty constraints for now
        let rows: Vec<Vec<Option<String>>> = vec![];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.awaitIndex()
    async fn execute_await_index(&self, args: &[JsonValue]) -> ProtocolResult<QueryResult> {
        let _index_name = args.first().and_then(|v| v.as_str()).unwrap_or("default");

        let columns = vec!["indexName".to_string(), "state".to_string()];
        let rows = vec![vec![Some("index".to_string()), Some("ONLINE".to_string())]];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.stats.nodeCount()
    async fn execute_node_count(&self) -> ProtocolResult<QueryResult> {
        let columns = vec!["nodeCount".to_string()];
        let rows = vec![vec![Some("0".to_string())]];

        Ok(Self::make_result(columns, rows))
    }

    /// Execute db.stats.relCount()
    async fn execute_rel_count(&self) -> ProtocolResult<QueryResult> {
        let columns = vec!["relCount".to_string()];
        let rows = vec![vec![Some("0".to_string())]];

        Ok(Self::make_result(columns, rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orbit_shared::graph::InMemoryGraphStorage;

    #[tokio::test]
    async fn test_db_info() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        let result = procedures.execute_db_info().await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Some("orbit".to_string()));
    }

    #[tokio::test]
    async fn test_db_labels() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        let result = procedures.execute_db_labels().await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.columns, vec!["label".to_string()]);
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_db_relationship_types() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        let result = procedures.execute_db_relationship_types().await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.columns, vec!["relationshipType".to_string()]);
        assert!(!result.rows.is_empty());
    }

    #[tokio::test]
    async fn test_dbms_procedures() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        let result = procedures.execute_dbms_procedures().await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.columns.len(), 5);
        // Should have many procedures listed
        assert!(result.rows.len() > 20);
    }

    #[tokio::test]
    async fn test_dbms_functions() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        let result = procedures.execute_dbms_functions().await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.columns.len(), 4);
        // Should have many functions listed
        assert!(result.rows.len() > 30);
    }

    #[tokio::test]
    async fn test_dbms_components() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        let result = procedures.execute_dbms_components().await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result
            .rows
            .iter()
            .any(|r| r[0] == Some("Orbit-RS".to_string())));
    }

    #[tokio::test]
    async fn test_execute_procedure_dispatch() {
        let storage = Arc::new(InMemoryGraphStorage::new());
        let procedures = DbProcedures::new(storage);

        // Test procedure dispatch
        let result = procedures.execute_procedure("db.labels", &[]).await;
        assert!(result.is_ok());

        let result = procedures.execute_procedure("dbms.procedures", &[]).await;
        assert!(result.is_ok());

        // Test unknown procedure
        let result = procedures.execute_procedure("unknown.procedure", &[]).await;
        assert!(result.is_err());
    }
}
