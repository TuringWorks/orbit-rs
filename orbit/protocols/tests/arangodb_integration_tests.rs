//! ArangoDB Protocol Integration Tests
//!
//! Comprehensive tests for the ArangoDB HTTP API implementation including:
//! - AQL spatial functions
//! - HTTP cursor API
//! - Collection and document operations
//! - Spatial indexes

#![cfg(feature = "arangodb")]

use orbit_protocols::arangodb::{
    AQLSpatialExecutor, AQLSpatialParser, AQLSpatialResult, ArangoHttpProtocol, Document,
    SpatialCollection, SpatialIndex, SpatialIndexType,
};
use orbit_shared::spatial::{Point, SpatialGeometry};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

// ============================================================================
// AQLSpatialExecutor Tests
// ============================================================================

#[tokio::test]
async fn test_aql_executor_creation() {
    let executor = AQLSpatialExecutor::new();
    // Executor should be created successfully - verify by calling a function
    let result = executor
        .execute_spatial_function("GEO_POINT", vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
        ])
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_geo_point_function() {
    let executor = AQLSpatialExecutor::new();

    // GEO_POINT takes longitude, latitude
    let result = executor
        .execute_spatial_function("GEO_POINT", vec![
            AQLSpatialResult::Number(-122.4194),
            AQLSpatialResult::Number(37.7749),
        ])
        .await;
    assert!(result.is_ok());

    match result.unwrap() {
        AQLSpatialResult::Geometry(geom) => {
            // Should be a point geometry
            assert!(format!("{:?}", geom).contains("Point"));
        }
        other => panic!("Expected Geometry result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_distance_function() {
    let executor = AQLSpatialExecutor::new();

    // GEO_DISTANCE: San Francisco (37.7749, -122.4194) to New York (40.7128, -74.0060)
    // Expected distance: approximately 4135 km
    let result = executor
        .execute_spatial_function("GEO_DISTANCE", vec![
            AQLSpatialResult::Number(37.7749),
            AQLSpatialResult::Number(-122.4194),
            AQLSpatialResult::Number(40.7128),
            AQLSpatialResult::Number(-74.0060),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Number(distance) => {
            // Distance should be approximately 4135 km (in meters: 4,135,000)
            // The haversine_distance function returns meters
            assert!(
                distance > 4000000.0 && distance < 4500000.0,
                "Distance should be approximately 4135 km, got {} m",
                distance
            );
        }
        other => panic!("Expected Number result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_distance_same_point() {
    let executor = AQLSpatialExecutor::new();

    // Distance from a point to itself should be 0
    let result = executor
        .execute_spatial_function("GEO_DISTANCE", vec![
            AQLSpatialResult::Number(37.7749),
            AQLSpatialResult::Number(-122.4194),
            AQLSpatialResult::Number(37.7749),
            AQLSpatialResult::Number(-122.4194),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Number(distance) => {
            assert!(distance.abs() < 0.001, "Distance to same point should be ~0");
        }
        other => panic!("Expected Number result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_polygon_function() {
    let executor = AQLSpatialExecutor::new();

    // Create a simple square polygon as array of coordinate pairs
    let coords = vec![
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(10.0),
            AQLSpatialResult::Number(0.0),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(10.0),
            AQLSpatialResult::Number(10.0),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(10.0),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
        ]),
    ];

    let result = executor
        .execute_spatial_function("GEO_POLYGON", vec![AQLSpatialResult::Array(coords)])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Geometry(_geom) => {
            // Should return some geometry (implementation uses MultiPoint as placeholder)
        }
        other => panic!("Expected Geometry result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_linestring_function() {
    let executor = AQLSpatialExecutor::new();

    // Create a simple linestring
    let coords = vec![
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(5.0),
            AQLSpatialResult::Number(5.0),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(10.0),
            AQLSpatialResult::Number(0.0),
        ]),
    ];

    let result = executor
        .execute_spatial_function("GEO_LINESTRING", vec![AQLSpatialResult::Array(coords)])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Geometry(_geom) => {
            // Should return geometry
        }
        other => panic!("Expected Geometry result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_contains_function() {
    let executor = AQLSpatialExecutor::new();

    // Test if polygon contains a point - requires geometry args
    // Create two point geometries
    let point1 = SpatialGeometry::Point(Point::new(5.0, 5.0, None));
    let point2 = SpatialGeometry::Point(Point::new(5.0, 5.0, None));

    let result = executor
        .execute_spatial_function("GEO_CONTAINS", vec![
            AQLSpatialResult::Geometry(point1),
            AQLSpatialResult::Geometry(point2),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Boolean(_contains) => {
            // Just verify we got a boolean response
        }
        other => panic!("Expected Boolean result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_intersects_function() {
    let executor = AQLSpatialExecutor::new();

    // Test intersection of identical points
    let point1 = SpatialGeometry::Point(Point::new(5.0, 5.0, None));
    let point2 = SpatialGeometry::Point(Point::new(5.0, 5.0, None));

    let result = executor
        .execute_spatial_function("GEO_INTERSECTS", vec![
            AQLSpatialResult::Geometry(point1),
            AQLSpatialResult::Geometry(point2),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Boolean(intersects) => {
            assert!(intersects, "Identical points should intersect");
        }
        other => panic!("Expected Boolean result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_geo_equals_function() {
    let executor = AQLSpatialExecutor::new();

    // Test equality of identical points
    let point1 = SpatialGeometry::Point(Point::new(5.0, 5.0, None));
    let point2 = SpatialGeometry::Point(Point::new(5.0, 5.0, None));

    let result = executor
        .execute_spatial_function("GEO_EQUALS", vec![
            AQLSpatialResult::Geometry(point1),
            AQLSpatialResult::Geometry(point2),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Boolean(equals) => {
            assert!(equals, "Identical points should be equal");
        }
        other => panic!("Expected Boolean result, got {:?}", other),
    }
}

// ============================================================================
// Collection and Document Tests
// ============================================================================

#[tokio::test]
async fn test_add_collection() {
    let executor = AQLSpatialExecutor::new();

    // add_collection takes (String, Vec<String>)
    executor
        .add_collection("test_locations".to_string(), vec!["location".to_string()])
        .await;
    // Collection should be added successfully
}

#[tokio::test]
async fn test_add_document_to_collection() {
    let executor = AQLSpatialExecutor::new();

    // Create collection first
    executor
        .add_collection("places".to_string(), vec!["coords".to_string()])
        .await;

    // Create document with spatial_fields as HashMap<String, SpatialGeometry>
    let mut data = Map::new();
    data.insert("name".to_string(), json!("San Francisco"));

    let mut spatial_fields = HashMap::new();
    spatial_fields.insert(
        "coords".to_string(),
        SpatialGeometry::Point(Point::new(-122.4194, 37.7749, None)),
    );

    let doc = Document {
        id: "places/sf".to_string(),
        data,
        spatial_fields,
    };

    let result = executor.add_document("places", doc).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_add_multiple_documents() {
    let executor = AQLSpatialExecutor::new();

    // Create collection
    executor
        .add_collection("cities".to_string(), vec!["location".to_string()])
        .await;

    // Add multiple documents
    let cities = vec![
        ("sf", "San Francisco", -122.4194, 37.7749),
        ("nyc", "New York", -74.0060, 40.7128),
        ("la", "Los Angeles", -118.2437, 34.0522),
    ];

    for (id, name, lng, lat) in cities {
        let mut data = Map::new();
        data.insert("name".to_string(), json!(name));

        let mut spatial_fields = HashMap::new();
        spatial_fields.insert(
            "location".to_string(),
            SpatialGeometry::Point(Point::new(lng, lat, None)),
        );

        let doc = Document {
            id: format!("cities/{}", id),
            data,
            spatial_fields,
        };

        let result = executor.add_document("cities", doc).await;
        assert!(result.is_ok(), "Failed to add document {}", id);
    }
}

// ============================================================================
// Spatial Index Tests
// ============================================================================

#[test]
fn test_spatial_index_creation() {
    let index = SpatialIndex {
        name: "geo_idx".to_string(),
        collection: "locations".to_string(),
        field: "coords".to_string(),
        index_type: SpatialIndexType::Geo,
        geometry_types: vec!["Point".to_string()],
    };

    assert_eq!(index.name, "geo_idx");
    assert!(matches!(index.index_type, SpatialIndexType::Geo));
}

#[test]
fn test_spatial_index_geojson_type() {
    let index = SpatialIndex {
        name: "geojson_idx".to_string(),
        collection: "areas".to_string(),
        field: "boundary".to_string(),
        index_type: SpatialIndexType::GeoJson,
        geometry_types: vec!["Polygon".to_string(), "MultiPolygon".to_string()],
    };

    assert_eq!(index.collection, "areas");
    assert!(matches!(index.index_type, SpatialIndexType::GeoJson));
    assert_eq!(index.geometry_types.len(), 2);
}

#[tokio::test]
async fn test_create_spatial_index_on_executor() {
    let executor = AQLSpatialExecutor::new();

    // Create collection first
    executor
        .add_collection("indexed_places".to_string(), vec!["location".to_string()])
        .await;

    // Create spatial index using the executor API
    // create_spatial_index takes (String, String, SpatialIndexType)
    let result = executor
        .create_spatial_index(
            "indexed_places".to_string(),
            "location".to_string(),
            SpatialIndexType::Geo,
        )
        .await;

    assert!(result.is_ok());
    let index_name = result.unwrap();
    assert!(index_name.contains("indexed_places"));
}

// ============================================================================
// AQL Parser Tests
// ============================================================================

#[tokio::test]
async fn test_aql_parser_creation() {
    let parser = AQLSpatialParser::new();
    let result = parser.execute_query("RETURN GEO_POINT(0, 0)").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_aql_parser_geo_distance_query() {
    let parser = AQLSpatialParser::new();

    let query = "RETURN GEO_DISTANCE(37.7749, -122.4194, 40.7128, -74.0060)";
    let result = parser.execute_query(query).await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Number(distance) => {
            // Distance in meters (approximately 4135 km = 4,135,000 m)
            assert!(distance > 4000000.0 && distance < 4500000.0);
        }
        other => panic!("Expected Number result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_aql_parser_geo_point_query() {
    let parser = AQLSpatialParser::new();

    let query = "RETURN GEO_POINT(-122.4194, 37.7749)";
    let result = parser.execute_query(query).await;

    assert!(result.is_ok());
}

// ============================================================================
// HTTP Protocol Tests
// ============================================================================

#[tokio::test]
async fn test_http_protocol_creation() {
    let protocol = ArangoHttpProtocol::new();
    // Protocol should be created successfully
    let result = protocol
        .handle_cursor_request(r#"{"query": "RETURN 1"}"#)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_http_cursor_request_geo_distance() {
    let protocol = ArangoHttpProtocol::new();

    let request = r#"{
        "query": "RETURN GEO_DISTANCE(37.7749, -122.4194, 40.7128, -74.0060)",
        "count": true
    }"#;

    let response = protocol.handle_cursor_request(request).await;
    assert!(response.is_ok());

    let response_json: Value = serde_json::from_str(&response.unwrap()).unwrap();
    assert_eq!(response_json["error"], false);
    assert_eq!(response_json["code"], 201);
}

#[tokio::test]
async fn test_http_cursor_request_geo_point() {
    let protocol = ArangoHttpProtocol::new();

    let request = r#"{
        "query": "RETURN GEO_POINT(-122.4194, 37.7749)"
    }"#;

    let response = protocol.handle_cursor_request(request).await;
    assert!(response.is_ok());

    let response_json: Value = serde_json::from_str(&response.unwrap()).unwrap();
    assert_eq!(response_json["error"], false);
    assert_eq!(response_json["hasMore"], false);
}

#[tokio::test]
async fn test_http_cursor_request_with_bind_vars() {
    let protocol = ArangoHttpProtocol::new();

    let request = r#"{
        "query": "RETURN GEO_DISTANCE(@lat1, @lng1, @lat2, @lng2)",
        "bindVars": {
            "lat1": 37.7749,
            "lng1": -122.4194,
            "lat2": 40.7128,
            "lng2": -74.0060
        }
    }"#;

    let response = protocol.handle_cursor_request(request).await;
    // Bind vars not fully implemented yet, but request should not error
    assert!(response.is_ok());
}

#[tokio::test]
async fn test_http_cursor_request_invalid_json() {
    let protocol = ArangoHttpProtocol::new();

    let request = "{ invalid json }";

    let response = protocol.handle_cursor_request(request).await;
    assert!(response.is_err());
}

#[tokio::test]
async fn test_http_cursor_request_with_batch_size() {
    let protocol = ArangoHttpProtocol::new();

    let request = r#"{
        "query": "RETURN GEO_POINT(0, 0)",
        "batchSize": 100
    }"#;

    let response = protocol.handle_cursor_request(request).await;
    assert!(response.is_ok());
}

// ============================================================================
// Result Type Tests
// ============================================================================

#[test]
fn test_aql_result_boolean() {
    let result = AQLSpatialResult::Boolean(true);
    match result {
        AQLSpatialResult::Boolean(b) => assert!(b),
        _ => panic!("Expected Boolean"),
    }
}

#[test]
fn test_aql_result_number() {
    let result = AQLSpatialResult::Number(123.456);
    match result {
        AQLSpatialResult::Number(n) => assert!((n - 123.456).abs() < 0.001),
        _ => panic!("Expected Number"),
    }
}

#[test]
fn test_aql_result_string() {
    let result = AQLSpatialResult::String("test string".to_string());
    match result {
        AQLSpatialResult::String(s) => assert_eq!(s, "test string"),
        _ => panic!("Expected String"),
    }
}

#[test]
fn test_aql_result_array() {
    let result = AQLSpatialResult::Array(vec![
        AQLSpatialResult::Number(1.0),
        AQLSpatialResult::Number(2.0),
        AQLSpatialResult::Number(3.0),
    ]);

    match result {
        AQLSpatialResult::Array(arr) => assert_eq!(arr.len(), 3),
        _ => panic!("Expected Array"),
    }
}

#[test]
fn test_aql_result_null() {
    let result = AQLSpatialResult::Null;
    match result {
        AQLSpatialResult::Null => {}
        _ => panic!("Expected Null"),
    }
}

#[test]
fn test_aql_result_documents() {
    let mut data = Map::new();
    data.insert("name".to_string(), json!("Test Doc"));

    let doc = Document {
        id: "test/1".to_string(),
        data,
        spatial_fields: HashMap::new(),
    };

    let result = AQLSpatialResult::Documents(vec![doc]);
    match result {
        AQLSpatialResult::Documents(docs) => {
            assert_eq!(docs.len(), 1);
            assert_eq!(docs[0].id, "test/1");
        }
        _ => panic!("Expected Documents"),
    }
}

// ============================================================================
// Document Tests
// ============================================================================

#[test]
fn test_document_creation() {
    let mut data = Map::new();
    data.insert("name".to_string(), json!("Test Location"));
    data.insert("country".to_string(), json!("USA"));

    let mut spatial_fields = HashMap::new();
    spatial_fields.insert(
        "coords".to_string(),
        SpatialGeometry::Point(Point::new(-122.4, 37.7, None)),
    );

    let doc = Document {
        id: "locations/test".to_string(),
        data,
        spatial_fields,
    };

    assert_eq!(doc.id, "locations/test");
    assert_eq!(doc.data.get("name").unwrap(), &json!("Test Location"));
    assert_eq!(doc.spatial_fields.len(), 1);
}

#[test]
fn test_document_with_geojson_in_data() {
    let mut data = Map::new();
    data.insert("name".to_string(), json!("Golden Gate Park"));
    data.insert(
        "boundary".to_string(),
        json!({
            "type": "Polygon",
            "coordinates": [[
                [-122.51, 37.77],
                [-122.45, 37.77],
                [-122.45, 37.76],
                [-122.51, 37.76],
                [-122.51, 37.77]
            ]]
        }),
    );

    let doc = Document {
        id: "parks/ggp".to_string(),
        data,
        spatial_fields: HashMap::new(),
    };

    assert!(doc.data.contains_key("boundary"));
}

// ============================================================================
// SpatialCollection Tests
// ============================================================================

#[test]
fn test_spatial_collection_creation() {
    let collection = SpatialCollection {
        name: "test_collection".to_string(),
        documents: HashMap::new(),
        geo_fields: vec!["location".to_string(), "boundary".to_string()],
    };

    assert_eq!(collection.name, "test_collection");
    assert_eq!(collection.geo_fields.len(), 2);
    assert!(collection.documents.is_empty());
}

#[test]
fn test_spatial_collection_with_documents() {
    let mut data = Map::new();
    data.insert("name".to_string(), json!("Doc 1"));

    let doc = Document {
        id: "col/1".to_string(),
        data,
        spatial_fields: HashMap::new(),
    };

    let mut documents = HashMap::new();
    documents.insert("col/1".to_string(), doc);

    let collection = SpatialCollection {
        name: "with_docs".to_string(),
        documents,
        geo_fields: vec![],
    };

    assert_eq!(collection.documents.len(), 1);
}

// ============================================================================
// NEAR and WITHIN Query Tests
// ============================================================================

#[tokio::test]
async fn test_near_query_with_documents() {
    let executor = AQLSpatialExecutor::new();

    // Add a collection with some locations
    executor
        .add_collection("locations".to_string(), vec!["position".to_string()])
        .await;

    // Add a document with spatial field
    let mut data = Map::new();
    data.insert("name".to_string(), json!("San Francisco"));

    let mut spatial_fields = HashMap::new();
    spatial_fields.insert(
        "position".to_string(),
        SpatialGeometry::Point(Point::new(-122.4194, 37.7749, None)),
    );

    let doc = Document {
        id: "loc/sf".to_string(),
        data,
        spatial_fields,
    };

    executor.add_document("locations", doc).await.unwrap();

    // Query NEAR the same location
    let result = executor
        .execute_spatial_function("NEAR", vec![
            AQLSpatialResult::String("locations".to_string()),
            AQLSpatialResult::Number(37.7749),
            AQLSpatialResult::Number(-122.4194),
            AQLSpatialResult::Number(10.0), // limit
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Documents(docs) => {
            assert_eq!(docs.len(), 1);
            assert_eq!(docs[0].id, "loc/sf");
        }
        other => panic!("Expected Documents result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_within_query() {
    let executor = AQLSpatialExecutor::new();

    // Add a collection
    executor
        .add_collection("places".to_string(), vec!["coords".to_string()])
        .await;

    // Add a document
    let mut data = Map::new();
    data.insert("name".to_string(), json!("Test Place"));

    let mut spatial_fields = HashMap::new();
    spatial_fields.insert(
        "coords".to_string(),
        SpatialGeometry::Point(Point::new(-122.4, 37.7, None)),
    );

    let doc = Document {
        id: "places/1".to_string(),
        data,
        spatial_fields,
    };

    executor.add_document("places", doc).await.unwrap();

    // Query WITHIN with large radius
    let result = executor
        .execute_spatial_function("WITHIN", vec![
            AQLSpatialResult::String("places".to_string()),
            AQLSpatialResult::Number(37.7),
            AQLSpatialResult::Number(-122.4),
            AQLSpatialResult::Number(1000.0), // radius in meters
        ])
        .await;

    assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_antipodal_distance() {
    let executor = AQLSpatialExecutor::new();

    // Distance between two antipodal points (opposite sides of Earth)
    // Should be approximately half of Earth's circumference (~20,000 km)
    let result = executor
        .execute_spatial_function("GEO_DISTANCE", vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(180.0),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Number(distance) => {
            // Half circumference should be ~20,000 km = 20,000,000 m
            assert!(
                distance > 19000000.0 && distance < 21000000.0,
                "Antipodal distance should be ~20,000 km, got {} m",
                distance
            );
        }
        _ => panic!("Expected Number"),
    }
}

#[tokio::test]
async fn test_equator_distance() {
    let executor = AQLSpatialExecutor::new();

    // Distance along the equator from 0,0 to 0,90 (quarter circumference)
    // Should be approximately 10,000 km
    let result = executor
        .execute_spatial_function("GEO_DISTANCE", vec![
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(0.0),
            AQLSpatialResult::Number(90.0),
        ])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Number(distance) => {
            // Quarter circumference should be ~10,000 km = 10,000,000 m
            assert!(
                distance > 9000000.0 && distance < 11000000.0,
                "Quarter circumference should be ~10,000 km, got {} m",
                distance
            );
        }
        _ => panic!("Expected Number"),
    }
}

#[tokio::test]
async fn test_multipoint_function() {
    let executor = AQLSpatialExecutor::new();

    let points = vec![
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(-122.4),
            AQLSpatialResult::Number(37.7),
        ]),
        AQLSpatialResult::Array(vec![
            AQLSpatialResult::Number(-74.0),
            AQLSpatialResult::Number(40.7),
        ]),
    ];

    let result = executor
        .execute_spatial_function("GEO_MULTIPOINT", vec![AQLSpatialResult::Array(points)])
        .await;

    assert!(result.is_ok());
    match result.unwrap() {
        AQLSpatialResult::Geometry(geom) => {
            assert!(format!("{:?}", geom).contains("MultiPoint"));
        }
        other => panic!("Expected Geometry result, got {:?}", other),
    }
}
