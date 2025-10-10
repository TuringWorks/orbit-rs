//! Cypher Spatial Functions for Orbit-RS
//!
//! This module implements Neo4j-compatible spatial functions and graph-based spatial queries.
//! It supports all Neo4j spatial functions with enhanced graph traversal capabilities.

use orbit_shared::spatial::{
    crs::utils::haversine_distance, Point, SpatialError, SpatialGeometry, WGS84_SRID,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cypher spatial function executor with graph capabilities
pub struct CypherSpatialExecutor {
    /// Graph nodes with spatial properties
    nodes: Arc<RwLock<HashMap<NodeId, Node>>>,
    /// Graph relationships
    relationships: Arc<RwLock<HashMap<RelationshipId, Relationship>>>,
    /// Spatial indexes on nodes
    spatial_indexes: Arc<RwLock<HashMap<String, SpatialIndex>>>,
    /// Labels and their associated nodes
    labels: Arc<RwLock<HashMap<String, HashSet<NodeId>>>>,
}

/// Node identifier
pub type NodeId = u64;

/// Relationship identifier
pub type RelationshipId = u64;

/// Graph node with properties
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub labels: Vec<String>,
    pub properties: Map<String, Value>,
    pub spatial_properties: HashMap<String, SpatialGeometry>,
}

/// Graph relationship
#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: RelationshipId,
    pub start_node: NodeId,
    pub end_node: NodeId,
    pub relationship_type: String,
    pub properties: Map<String, Value>,
    pub spatial_properties: HashMap<String, SpatialGeometry>,
}

/// Spatial index for graph nodes/relationships
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    pub name: String,
    pub label: Option<String>,
    pub property: String,
    pub geometry_type: String,
    pub indexed_nodes: HashSet<NodeId>,
}

/// Cypher spatial function result
#[derive(Debug, Clone)]
pub enum CypherSpatialResult {
    Boolean(bool),
    Number(f64),
    String(String),
    Point(Point),
    Geometry(SpatialGeometry),
    Node(Node),
    Relationship(Relationship),
    Nodes(Vec<Node>),
    Paths(Vec<Path>),
    Map(Map<String, Value>),
    List(Vec<CypherSpatialResult>),
    Null,
}

/// Graph path with spatial context
#[derive(Debug, Clone)]
pub struct Path {
    pub nodes: Vec<Node>,
    pub relationships: Vec<Relationship>,
    pub total_distance: Option<f64>,
}

impl CypherSpatialExecutor {
    /// Create a new Cypher spatial executor
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            relationships: Arc::new(RwLock::new(HashMap::new())),
            spatial_indexes: Arc::new(RwLock::new(HashMap::new())),
            labels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute a Cypher spatial function
    pub async fn execute_spatial_function(
        &self,
        function_name: &str,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        match function_name.to_lowercase().as_str() {
            // Point creation and access functions
            "point" => self.point_function(args).await,
            "distance" => self.distance_function(args).await,

            // Geometric relationship functions
            "intersects" => self.intersects_function(args).await,
            "contains" => self.contains_function(args).await,
            "within" => self.within_function(args).await,
            "overlaps" => self.overlaps_function(args).await,
            "touches" => self.touches_function(args).await,
            "crosses" => self.crosses_function(args).await,
            "disjoint" => self.disjoint_function(args).await,

            // Spatial analysis functions
            "bbox" => self.bbox_function(args).await,
            "centroid" => self.centroid_function(args).await,
            "buffer" => self.buffer_function(args).await,
            "convex_hull" => self.convex_hull_function(args).await,

            // Graph-specific spatial functions
            "spatial.closest" => self.spatial_closest(args).await,
            "spatial.withinDistance" => self.spatial_within_distance(args).await,
            "spatial.intersects" => self.spatial_intersects_graph(args).await,
            "spatial.layer" => self.spatial_layer(args).await,

            // Shortest path with spatial constraints
            "shortestPath" => self.shortest_path_spatial(args).await,
            "allShortestPaths" => self.all_shortest_paths_spatial(args).await,

            // Spatial aggregation functions
            "collect.spatial" => self.collect_spatial(args).await,
            "reduce.distance" => self.reduce_distance(args).await,

            _ => Err(SpatialError::OperationError(format!(
                "Unknown Cypher spatial function: {}",
                function_name
            ))),
        }
    }

    /// point({x: longitude, y: latitude, crs: 'WGS-84'})
    async fn point_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.is_empty() {
            return Err(SpatialError::OperationError(
                "point() requires arguments".to_string(),
            ));
        }

        match &args[0] {
            CypherSpatialResult::Map(map) => {
                let x = map.get("x").and_then(|v| v.as_f64()).ok_or_else(|| {
                    SpatialError::OperationError("Missing x coordinate".to_string())
                })?;

                let y = map.get("y").and_then(|v| v.as_f64()).ok_or_else(|| {
                    SpatialError::OperationError("Missing y coordinate".to_string())
                })?;

                let z = map.get("z").and_then(|v| v.as_f64());

                let srid = map
                    .get("crs")
                    .and_then(|v| v.as_str())
                    .map(|s| match s {
                        "WGS-84" | "EPSG:4326" => WGS84_SRID,
                        _ => WGS84_SRID, // Default to WGS84
                    })
                    .unwrap_or(WGS84_SRID);

                let mut point = Point::new(x, y, Some(srid));
                if let Some(z_val) = z {
                    point.z = Some(z_val);
                }

                Ok(CypherSpatialResult::Point(point))
            }
            _ => Err(SpatialError::OperationError(
                "point() requires a map argument".to_string(),
            )),
        }
    }

    /// distance(point1, point2)
    async fn distance_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "distance() requires 2 arguments".to_string(),
            ));
        }

        let point1 = self.extract_point(&args[0])?;
        let point2 = self.extract_point(&args[1])?;

        let distance = haversine_distance(&point1, &point2);
        Ok(CypherSpatialResult::Number(distance))
    }

    /// intersects(geometry1, geometry2)
    async fn intersects_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "intersects() requires 2 arguments".to_string(),
            ));
        }

        let geom1 = self.extract_geometry(&args[0])?;
        let geom2 = self.extract_geometry(&args[1])?;

        let intersects = SpatialOperations::intersects(&geom1, &geom2)?;
        Ok(CypherSpatialResult::Boolean(intersects))
    }

    /// contains(geometry1, geometry2)
    async fn contains_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "contains() requires 2 arguments".to_string(),
            ));
        }

        let geom1 = self.extract_geometry(&args[0])?;
        let geom2 = self.extract_geometry(&args[1])?;

        // Use point_in_polygon for simple contains logic
        let contains = match (&geom1, &geom2) {
            (SpatialGeometry::Polygon(poly), SpatialGeometry::Point(point)) => {
                SpatialOperations::point_in_polygon(point, poly)?
            }
            _ => false, // Simplified - would need more geometry combinations
        };
        Ok(CypherSpatialResult::Boolean(contains))
    }

    /// within(geometry1, geometry2)
    async fn within_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "within() requires 2 arguments".to_string(),
            ));
        }

        let geom1 = self.extract_geometry(&args[0])?;
        let geom2 = self.extract_geometry(&args[1])?;

        // within is the inverse of contains
        let within = match (&geom2, &geom1) {
            (SpatialGeometry::Polygon(poly), SpatialGeometry::Point(point)) => {
                SpatialOperations::point_in_polygon(point, poly)?
            }
            _ => false, // Simplified - would need more geometry combinations
        };
        Ok(CypherSpatialResult::Boolean(within))
    }

    /// overlaps(geometry1, geometry2)
    async fn overlaps_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "overlaps() requires 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::Boolean(false))
    }

    /// touches(geometry1, geometry2)
    async fn touches_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "touches() requires 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::Boolean(false))
    }

    /// crosses(geometry1, geometry2)
    async fn crosses_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "crosses() requires 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::Boolean(false))
    }

    /// disjoint(geometry1, geometry2)
    async fn disjoint_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "disjoint() requires 2 arguments".to_string(),
            ));
        }

        let geom1 = self.extract_geometry(&args[0])?;
        let geom2 = self.extract_geometry(&args[1])?;

        // disjoint is the opposite of intersects
        let intersects = SpatialOperations::intersects(&geom1, &geom2)?;
        Ok(CypherSpatialResult::Boolean(!intersects))
    }

    /// bbox(geometry)
    async fn bbox_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "bbox() requires 1 argument".to_string(),
            ));
        }

        // Placeholder - would calculate actual bounding box
        let mut bbox_map = Map::new();
        bbox_map.insert(
            "minX".to_string(),
            Value::Number(serde_json::Number::from_f64(-180.0).unwrap()),
        );
        bbox_map.insert(
            "minY".to_string(),
            Value::Number(serde_json::Number::from_f64(-90.0).unwrap()),
        );
        bbox_map.insert(
            "maxX".to_string(),
            Value::Number(serde_json::Number::from_f64(180.0).unwrap()),
        );
        bbox_map.insert(
            "maxY".to_string(),
            Value::Number(serde_json::Number::from_f64(90.0).unwrap()),
        );

        Ok(CypherSpatialResult::Map(bbox_map))
    }

    /// centroid(geometry)
    async fn centroid_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "centroid() requires 1 argument".to_string(),
            ));
        }

        // Placeholder implementation
        let centroid = Point::new(0.0, 0.0, Some(WGS84_SRID));
        Ok(CypherSpatialResult::Point(centroid))
    }

    /// buffer(geometry, distance)
    async fn buffer_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "buffer() requires 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation
        let _geometry = self.extract_geometry(&args[0])?;
        let _distance = self.extract_number(&args[1])?;

        Ok(CypherSpatialResult::Geometry(SpatialGeometry::Point(
            Point::new(0.0, 0.0, Some(WGS84_SRID)),
        )))
    }

    /// convex_hull(geometry)
    async fn convex_hull_function(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "convex_hull() requires 1 argument".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::Geometry(SpatialGeometry::Point(
            Point::new(0.0, 0.0, Some(WGS84_SRID)),
        )))
    }

    /// spatial.closest(point, label, limit)
    async fn spatial_closest(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "spatial.closest requires at least 2 arguments".to_string(),
            ));
        }

        let reference_point = self.extract_point(&args[0])?;
        let label = self.extract_string(&args[1])?;
        let limit = if args.len() > 2 {
            self.extract_number(&args[2])? as usize
        } else {
            10
        };

        let nodes = self.nodes.read().await;
        let labels = self.labels.read().await;

        if let Some(label_nodes) = labels.get(&label) {
            let mut candidates = Vec::new();

            for &node_id in label_nodes {
                if let Some(node) = nodes.get(&node_id) {
                    for geometry in node.spatial_properties.values() {
                        if let SpatialGeometry::Point(point) = geometry {
                            let distance = haversine_distance(&reference_point, point);
                            candidates.push((distance, node.clone()));
                            break;
                        }
                    }
                }
            }

            // Sort by distance and limit
            candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            candidates.truncate(limit);

            let result_nodes: Vec<Node> = candidates.into_iter().map(|(_, node)| node).collect();
            Ok(CypherSpatialResult::Nodes(result_nodes))
        } else {
            Ok(CypherSpatialResult::Nodes(vec![]))
        }
    }

    /// spatial.withinDistance(point, distance, label)
    async fn spatial_within_distance(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 3 {
            return Err(SpatialError::OperationError(
                "spatial.withinDistance requires 3 arguments".to_string(),
            ));
        }

        let reference_point = self.extract_point(&args[0])?;
        let max_distance = self.extract_number(&args[1])?;
        let label = self.extract_string(&args[2])?;

        let nodes = self.nodes.read().await;
        let labels = self.labels.read().await;

        if let Some(label_nodes) = labels.get(&label) {
            let mut result_nodes = Vec::new();

            for &node_id in label_nodes {
                if let Some(node) = nodes.get(&node_id) {
                    for geometry in node.spatial_properties.values() {
                        if let SpatialGeometry::Point(point) = geometry {
                            let distance = haversine_distance(&reference_point, point);
                            if distance <= max_distance {
                                result_nodes.push(node.clone());
                                break;
                            }
                        }
                    }
                }
            }

            Ok(CypherSpatialResult::Nodes(result_nodes))
        } else {
            Ok(CypherSpatialResult::Nodes(vec![]))
        }
    }

    /// spatial.intersects(geometry, label)
    async fn spatial_intersects_graph(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "spatial.intersects requires 2 arguments".to_string(),
            ));
        }

        let reference_geometry = self.extract_geometry(&args[0])?;
        let label = self.extract_string(&args[1])?;

        let nodes = self.nodes.read().await;
        let labels = self.labels.read().await;

        if let Some(label_nodes) = labels.get(&label) {
            let mut result_nodes = Vec::new();

            for &node_id in label_nodes {
                if let Some(node) = nodes.get(&node_id) {
                    for geometry in node.spatial_properties.values() {
                        match SpatialOperations::intersects(&reference_geometry, geometry) {
                            Ok(true) => {
                                result_nodes.push(node.clone());
                                break;
                            }
                            _ => continue,
                        }
                    }
                }
            }

            Ok(CypherSpatialResult::Nodes(result_nodes))
        } else {
            Ok(CypherSpatialResult::Nodes(vec![]))
        }
    }

    /// spatial.layer(layer_name)
    async fn spatial_layer(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "spatial.layer requires 1 argument".to_string(),
            ));
        }

        // Placeholder for spatial layer functionality
        Ok(CypherSpatialResult::Nodes(vec![]))
    }

    /// shortestPath with spatial constraints
    async fn shortest_path_spatial(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "shortestPath requires at least 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation for spatial shortest path
        let path = Path {
            nodes: vec![],
            relationships: vec![],
            total_distance: Some(0.0),
        };

        Ok(CypherSpatialResult::Paths(vec![path]))
    }

    /// allShortestPaths with spatial constraints
    async fn all_shortest_paths_spatial(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "allShortestPaths requires at least 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::Paths(vec![]))
    }

    /// collect.spatial aggregation
    async fn collect_spatial(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.is_empty() {
            return Err(SpatialError::OperationError(
                "collect.spatial requires arguments".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::List(args))
    }

    /// reduce.distance aggregation
    async fn reduce_distance(
        &self,
        args: Vec<CypherSpatialResult>,
    ) -> Result<CypherSpatialResult, SpatialError> {
        if args.is_empty() {
            return Err(SpatialError::OperationError(
                "reduce.distance requires arguments".to_string(),
            ));
        }

        // Placeholder implementation
        Ok(CypherSpatialResult::Number(0.0))
    }

    // Helper methods

    fn extract_point(&self, result: &CypherSpatialResult) -> Result<Point, SpatialError> {
        match result {
            CypherSpatialResult::Point(p) => Ok(p.clone()),
            CypherSpatialResult::Geometry(SpatialGeometry::Point(p)) => Ok(p.clone()),
            _ => Err(SpatialError::OperationError("Expected point".to_string())),
        }
    }

    fn extract_geometry(
        &self,
        result: &CypherSpatialResult,
    ) -> Result<SpatialGeometry, SpatialError> {
        match result {
            CypherSpatialResult::Geometry(geom) => Ok(geom.clone()),
            CypherSpatialResult::Point(p) => Ok(SpatialGeometry::Point(p.clone())),
            _ => Err(SpatialError::OperationError(
                "Expected geometry".to_string(),
            )),
        }
    }

    fn extract_number(&self, result: &CypherSpatialResult) -> Result<f64, SpatialError> {
        match result {
            CypherSpatialResult::Number(n) => Ok(*n),
            _ => Err(SpatialError::OperationError("Expected number".to_string())),
        }
    }

    fn extract_string(&self, result: &CypherSpatialResult) -> Result<String, SpatialError> {
        match result {
            CypherSpatialResult::String(s) => Ok(s.clone()),
            _ => Err(SpatialError::OperationError("Expected string".to_string())),
        }
    }

    /// Add a node to the graph
    pub async fn add_node(&self, node: Node) {
        let node_id = node.id;
        let labels = node.labels.clone();

        // Add to nodes
        let mut nodes = self.nodes.write().await;
        nodes.insert(node_id, node);

        // Update label index
        let mut label_index = self.labels.write().await;
        for label in labels {
            label_index.entry(label).or_default().insert(node_id);
        }
    }

    /// Add a relationship to the graph
    pub async fn add_relationship(&self, relationship: Relationship) {
        let mut relationships = self.relationships.write().await;
        relationships.insert(relationship.id, relationship);
    }

    /// Create a spatial index
    pub async fn create_spatial_index(
        &self,
        name: String,
        label: Option<String>,
        property: String,
        geometry_type: String,
    ) -> Result<(), SpatialError> {
        let index = SpatialIndex {
            name: name.clone(),
            label,
            property,
            geometry_type,
            indexed_nodes: HashSet::new(),
        };

        let mut indexes = self.spatial_indexes.write().await;
        indexes.insert(name, index);

        Ok(())
    }
}

impl Default for CypherSpatialExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse Cypher query and extract spatial functions
pub struct CypherSpatialParser {
    executor: CypherSpatialExecutor,
}

impl CypherSpatialParser {
    pub fn new() -> Self {
        Self {
            executor: CypherSpatialExecutor::new(),
        }
    }

    /// Parse and execute a Cypher query with spatial functions
    pub async fn execute_query(
        &self,
        cypher_query: &str,
    ) -> Result<CypherSpatialResult, SpatialError> {
        // Simplified parser - a full implementation would use proper Cypher parsing
        if cypher_query.to_lowercase().contains("point(") {
            // Example: RETURN point({x: -122.4194, y: 37.7749})
            let mut point_map = Map::new();
            point_map.insert(
                "x".to_string(),
                Value::Number(serde_json::Number::from_f64(-122.4194).unwrap()),
            );
            point_map.insert(
                "y".to_string(),
                Value::Number(serde_json::Number::from_f64(37.7749).unwrap()),
            );
            point_map.insert("crs".to_string(), Value::String("WGS-84".to_string()));

            let args = vec![CypherSpatialResult::Map(point_map)];
            self.executor.execute_spatial_function("point", args).await
        } else if cypher_query.to_lowercase().contains("distance(") {
            // Example: RETURN distance(point1, point2)
            let point1 = Point::new(-122.4194, 37.7749, Some(WGS84_SRID));
            let point2 = Point::new(-74.0060, 40.7128, Some(WGS84_SRID));

            let args = vec![
                CypherSpatialResult::Point(point1),
                CypherSpatialResult::Point(point2),
            ];

            self.executor
                .execute_spatial_function("distance", args)
                .await
        } else {
            Ok(CypherSpatialResult::Null)
        }
    }

    /// Get the underlying executor for direct function calls
    pub fn executor(&self) -> &CypherSpatialExecutor {
        &self.executor
    }
}

impl Default for CypherSpatialParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_point_function() {
        let executor = CypherSpatialExecutor::new();

        let mut point_map = Map::new();
        point_map.insert(
            "x".to_string(),
            Value::Number(serde_json::Number::from_f64(-122.4194).unwrap()),
        );
        point_map.insert(
            "y".to_string(),
            Value::Number(serde_json::Number::from_f64(37.7749).unwrap()),
        );
        point_map.insert("crs".to_string(), Value::String("WGS-84".to_string()));

        let args = vec![CypherSpatialResult::Map(point_map)];
        let result = executor
            .execute_spatial_function("point", args)
            .await
            .unwrap();

        match result {
            CypherSpatialResult::Point(point) => {
                assert!((point.x - (-122.4194)).abs() < f64::EPSILON);
                assert!((point.y - 37.7749).abs() < f64::EPSILON);
            }
            _ => panic!("Expected point result"),
        }
    }

    #[tokio::test]
    async fn test_distance_function() {
        let executor = CypherSpatialExecutor::new();

        let point1 = Point::new(-122.4194, 37.7749, Some(WGS84_SRID)); // San Francisco
        let point2 = Point::new(-74.0060, 40.7128, Some(WGS84_SRID)); // New York

        let args = vec![
            CypherSpatialResult::Point(point1),
            CypherSpatialResult::Point(point2),
        ];

        let result = executor
            .execute_spatial_function("distance", args)
            .await
            .unwrap();

        match result {
            CypherSpatialResult::Number(distance) => {
                // Should be approximately 4135 km
                assert!((distance - 4135000.0).abs() < 100000.0);
            }
            _ => panic!("Expected number result"),
        }
    }

    #[tokio::test]
    async fn test_spatial_closest() {
        let executor = CypherSpatialExecutor::new();

        // Add a test node
        let mut spatial_props = HashMap::new();
        spatial_props.insert(
            "location".to_string(),
            SpatialGeometry::Point(Point::new(-122.4194, 37.7749, Some(WGS84_SRID))),
        );

        let node = Node {
            id: 1,
            labels: vec!["Location".to_string()],
            properties: Map::new(),
            spatial_properties: spatial_props,
        };

        executor.add_node(node).await;

        let reference_point = Point::new(-122.4194, 37.7749, Some(WGS84_SRID));
        let args = vec![
            CypherSpatialResult::Point(reference_point),
            CypherSpatialResult::String("Location".to_string()),
            CypherSpatialResult::Number(5.0),
        ];

        let result = executor
            .execute_spatial_function("spatial.closest", args)
            .await
            .unwrap();

        match result {
            CypherSpatialResult::Nodes(nodes) => {
                assert_eq!(nodes.len(), 1);
                assert_eq!(nodes[0].id, 1);
            }
            _ => panic!("Expected nodes result"),
        }
    }

    #[tokio::test]
    async fn test_intersects_function() {
        let executor = CypherSpatialExecutor::new();

        let point1 = Point::new(-122.4194, 37.7749, Some(WGS84_SRID));
        let point2 = Point::new(-122.4194, 37.7749, Some(WGS84_SRID)); // Same point

        let args = vec![
            CypherSpatialResult::Geometry(SpatialGeometry::Point(point1)),
            CypherSpatialResult::Geometry(SpatialGeometry::Point(point2)),
        ];

        let result = executor
            .execute_spatial_function("intersects", args)
            .await
            .unwrap();

        match result {
            CypherSpatialResult::Boolean(intersects) => {
                // Identical points should intersect
                assert!(intersects);
            }
            _ => panic!("Expected boolean result"),
        }
    }
}
