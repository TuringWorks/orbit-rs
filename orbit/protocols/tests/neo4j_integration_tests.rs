//! Neo4j/Bolt Protocol Integration Tests
//!
//! Comprehensive tests for the Neo4j Bolt protocol implementation including:
//! - Cypher spatial functions
//! - Graph node and relationship operations
//! - Spatial indexes

#![cfg(feature = "neo4j")]

use orbit_protocols::neo4j::{
    CypherSpatialExecutor, CypherSpatialParser, CypherSpatialResult, Node, Path, Relationship,
    SpatialIndex,
};
use orbit_shared::spatial::{Point, SpatialGeometry};
use serde_json::{json, Map};
use std::collections::{HashMap, HashSet};

// ============================================================================
// CypherSpatialExecutor Tests
// ============================================================================

#[tokio::test]
async fn test_cypher_executor_creation() {
    let executor = CypherSpatialExecutor::new();
    // Executor should be created successfully - verify by adding a node
    let node = Node {
        id: 1,
        labels: vec!["Test".to_string()],
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };
    executor.add_node(node).await;
}

#[tokio::test]
async fn test_execute_point_function() {
    let executor = CypherSpatialExecutor::new();

    // point() takes a Map with x and y coordinates (Neo4j style: point({x: ..., y: ...}))
    let mut point_map = Map::new();
    point_map.insert("x".to_string(), json!(-122.4194));
    point_map.insert("y".to_string(), json!(37.7749));

    let result = executor
        .execute_spatial_function("point", vec![CypherSpatialResult::Map(point_map)])
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_execute_distance_function() {
    let executor = CypherSpatialExecutor::new();

    // distance() takes two Point arguments (CypherSpatialResult::Point)
    // SF point: longitude -122.4194, latitude 37.7749
    let sf_point = Point::new(-122.4194, 37.7749, None);
    // NYC point: longitude -74.0060, latitude 40.7128
    let nyc_point = Point::new(-74.0060, 40.7128, None);

    let result = executor
        .execute_spatial_function("distance", vec![
            CypherSpatialResult::Point(sf_point),
            CypherSpatialResult::Point(nyc_point),
        ])
        .await;

    assert!(result.is_ok());
    if let Ok(CypherSpatialResult::Number(distance)) = result {
        // Distance should be approximately 4135 km (4,135,000 meters)
        // haversine_distance returns meters
        assert!(
            distance > 4000000.0 && distance < 4500000.0,
            "Distance should be approximately 4135 km, got {} m",
            distance
        );
    }
}

#[tokio::test]
async fn test_execute_distance_same_point() {
    let executor = CypherSpatialExecutor::new();

    // Distance from a point to itself should be 0
    let point = Point::new(-122.4194, 37.7749, None);

    let result = executor
        .execute_spatial_function("distance", vec![
            CypherSpatialResult::Point(point.clone()),
            CypherSpatialResult::Point(point),
        ])
        .await;

    assert!(result.is_ok());
    if let Ok(CypherSpatialResult::Number(distance)) = result {
        assert!(distance.abs() < 0.001, "Distance to same point should be ~0");
    }
}

// ============================================================================
// Node and Relationship Tests
// ============================================================================

#[test]
fn test_node_creation() {
    let mut properties = Map::new();
    properties.insert("name".to_string(), json!("Alice"));
    properties.insert("age".to_string(), json!(30));

    let node = Node {
        id: 1,
        labels: vec!["Person".to_string()],
        properties,
        spatial_properties: HashMap::new(),
    };

    assert_eq!(node.id, 1);
    assert_eq!(node.labels, vec!["Person".to_string()]);
    assert_eq!(node.properties.get("name").unwrap(), &json!("Alice"));
}

#[test]
fn test_node_with_multiple_labels() {
    let node = Node {
        id: 2,
        labels: vec!["Person".to_string(), "Employee".to_string(), "Manager".to_string()],
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    assert_eq!(node.labels.len(), 3);
    assert!(node.labels.contains(&"Employee".to_string()));
}

#[test]
fn test_node_with_spatial_property() {
    let mut spatial_properties = HashMap::new();
    spatial_properties.insert(
        "location".to_string(),
        SpatialGeometry::Point(Point::new(-122.4194, 37.7749, None)),
    );

    let node = Node {
        id: 3,
        labels: vec!["Location".to_string()],
        properties: Map::new(),
        spatial_properties,
    };

    assert!(node.spatial_properties.contains_key("location"));
}

#[test]
fn test_relationship_creation() {
    let relationship = Relationship {
        id: 100,
        start_node: 1,
        end_node: 2,
        relationship_type: "KNOWS".to_string(),
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    assert_eq!(relationship.id, 100);
    assert_eq!(relationship.start_node, 1);
    assert_eq!(relationship.end_node, 2);
    assert_eq!(relationship.relationship_type, "KNOWS");
}

#[test]
fn test_relationship_with_properties() {
    let mut properties = Map::new();
    properties.insert("since".to_string(), json!(2020));
    properties.insert("weight".to_string(), json!(1.5));

    let relationship = Relationship {
        id: 101,
        start_node: 1,
        end_node: 2,
        relationship_type: "FRIENDS_WITH".to_string(),
        properties,
        spatial_properties: HashMap::new(),
    };

    assert_eq!(relationship.properties.get("since").unwrap(), &json!(2020));
}

#[test]
fn test_path_creation() {
    let node1 = Node {
        id: 1,
        labels: vec!["Person".to_string()],
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    let node2 = Node {
        id: 2,
        labels: vec!["Person".to_string()],
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    let rel = Relationship {
        id: 100,
        start_node: 1,
        end_node: 2,
        relationship_type: "KNOWS".to_string(),
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    let path = Path {
        nodes: vec![node1, node2],
        relationships: vec![rel],
        total_distance: Some(10.5),
    };

    assert_eq!(path.nodes.len(), 2);
    assert_eq!(path.relationships.len(), 1);
    assert_eq!(path.total_distance, Some(10.5));
}

// ============================================================================
// Spatial Index Tests
// ============================================================================

#[test]
fn test_spatial_index_creation() {
    let index = SpatialIndex {
        name: "location_idx".to_string(),
        label: Some("Place".to_string()),
        property: "location".to_string(),
        geometry_type: "POINT".to_string(),
        indexed_nodes: HashSet::new(),
    };

    assert_eq!(index.name, "location_idx");
    assert_eq!(index.label, Some("Place".to_string()));
    assert_eq!(index.property, "location");
}

#[test]
fn test_spatial_index_with_nodes() {
    let mut indexed_nodes = HashSet::new();
    indexed_nodes.insert(1);
    indexed_nodes.insert(2);
    indexed_nodes.insert(3);

    let index = SpatialIndex {
        name: "geo_idx".to_string(),
        label: Some("Location".to_string()),
        property: "coords".to_string(),
        geometry_type: "POINT".to_string(),
        indexed_nodes,
    };

    assert_eq!(index.indexed_nodes.len(), 3);
    assert!(index.indexed_nodes.contains(&2));
}

// ============================================================================
// Graph Operations Tests
// ============================================================================

#[tokio::test]
async fn test_add_node_to_executor() {
    let executor = CypherSpatialExecutor::new();

    let mut properties = Map::new();
    properties.insert("name".to_string(), json!("Test Location"));

    let mut spatial_properties = HashMap::new();
    spatial_properties.insert(
        "location".to_string(),
        SpatialGeometry::Point(Point::new(-122.4, 37.7, None)),
    );

    let node = Node {
        id: 1,
        labels: vec!["Location".to_string()],
        properties,
        spatial_properties,
    };

    executor.add_node(node).await;
    // Node should be added successfully
}

#[tokio::test]
async fn test_add_relationship_to_executor() {
    let executor = CypherSpatialExecutor::new();

    // Add two nodes first
    let node1 = Node {
        id: 1,
        labels: vec!["Person".to_string()],
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };
    let node2 = Node {
        id: 2,
        labels: vec!["Person".to_string()],
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    executor.add_node(node1).await;
    executor.add_node(node2).await;

    // Add relationship
    let rel = Relationship {
        id: 100,
        start_node: 1,
        end_node: 2,
        relationship_type: "KNOWS".to_string(),
        properties: Map::new(),
        spatial_properties: HashMap::new(),
    };

    executor.add_relationship(rel).await;
    // Relationship should be added successfully
}

#[tokio::test]
async fn test_create_spatial_index_on_executor() {
    let executor = CypherSpatialExecutor::new();

    // Create spatial index using the executor API
    // create_spatial_index takes (name, label, property, geometry_type)
    let result = executor
        .create_spatial_index(
            "test_idx".to_string(),
            Some("Location".to_string()),
            "coords".to_string(),
            "POINT".to_string(),
        )
        .await;

    assert!(result.is_ok());
    // Index should be created successfully
}

// ============================================================================
// CypherSpatialParser Tests
// ============================================================================

#[tokio::test]
async fn test_parser_creation() {
    let parser = CypherSpatialParser::new();
    // Parser should be created successfully
    let _ = parser.executor();
}

#[tokio::test]
async fn test_parser_execute_query_point() {
    let parser = CypherSpatialParser::new();

    let result = parser.execute_query("RETURN point({x: -122.4, y: 37.7})").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_parser_execute_query_distance() {
    let parser = CypherSpatialParser::new();

    let query = "RETURN distance(point({x: -122.4, y: 37.7}), point({x: -74.0, y: 40.7}))";
    let result = parser.execute_query(query).await;
    assert!(result.is_ok());
}

// ============================================================================
// Result Type Tests
// ============================================================================

#[test]
fn test_cypher_result_boolean() {
    let result = CypherSpatialResult::Boolean(true);
    match result {
        CypherSpatialResult::Boolean(b) => assert!(b),
        _ => panic!("Expected Boolean"),
    }
}

#[test]
fn test_cypher_result_number() {
    let result = CypherSpatialResult::Number(42.5);
    match result {
        CypherSpatialResult::Number(n) => assert!((n - 42.5).abs() < 0.001),
        _ => panic!("Expected Number"),
    }
}

#[test]
fn test_cypher_result_string() {
    let result = CypherSpatialResult::String("test".to_string());
    match result {
        CypherSpatialResult::String(s) => assert_eq!(s, "test"),
        _ => panic!("Expected String"),
    }
}

#[test]
fn test_cypher_result_point() {
    let point = Point::new(-122.4, 37.7, None);
    let result = CypherSpatialResult::Point(point.clone());
    match result {
        CypherSpatialResult::Point(p) => {
            assert!((p.x - (-122.4)).abs() < 0.001);
            assert!((p.y - 37.7).abs() < 0.001);
        }
        _ => panic!("Expected Point"),
    }
}

#[test]
fn test_cypher_result_null() {
    let result = CypherSpatialResult::Null;
    match result {
        CypherSpatialResult::Null => {}
        _ => panic!("Expected Null"),
    }
}

#[test]
fn test_cypher_result_list() {
    let list = vec![
        CypherSpatialResult::Number(1.0),
        CypherSpatialResult::Number(2.0),
        CypherSpatialResult::Number(3.0),
    ];
    let result = CypherSpatialResult::List(list);
    match result {
        CypherSpatialResult::List(l) => assert_eq!(l.len(), 3),
        _ => panic!("Expected List"),
    }
}

#[test]
fn test_cypher_result_map() {
    let mut map = Map::new();
    map.insert("key".to_string(), json!("value"));
    let result = CypherSpatialResult::Map(map);
    match result {
        CypherSpatialResult::Map(m) => {
            assert!(m.contains_key("key"));
        }
        _ => panic!("Expected Map"),
    }
}
