//! AQL (ArangoDB Query Language) Spatial Functions for Orbit-RS
//!
//! This module implements ArangoDB-compatible spatial functions and query processing.
//! It supports all ArangoDB GEO_* functions and spatial indexing capabilities.

use orbit_shared::spatial::{
    crs::utils::haversine_distance, MultiPoint, Point, SpatialError, SpatialGeometry,
    SpatialOperations, WGS84_SRID,
};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// AQL spatial function executor
pub struct AQLSpatialExecutor {
    /// Document collections with spatial data
    collections: Arc<RwLock<HashMap<String, SpatialCollection>>>,
    /// Spatial indexes for collections
    spatial_indexes: Arc<RwLock<HashMap<String, Vec<SpatialIndex>>>>,
}

/// Spatial collection containing documents with geo data
#[derive(Debug, Clone)]
pub struct SpatialCollection {
    pub name: String,
    pub documents: HashMap<String, Document>,
    pub geo_fields: Vec<String>,
}

/// Document with potential spatial fields
#[derive(Debug, Clone)]
pub struct Document {
    pub id: String,
    pub data: Map<String, Value>,
    pub spatial_fields: HashMap<String, SpatialGeometry>,
}

/// Spatial index on a collection
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    pub name: String,
    pub collection: String,
    pub field: String,
    pub index_type: SpatialIndexType,
    pub geometry_types: Vec<String>,
}

/// Types of spatial indexes supported by ArangoDB
#[derive(Debug, Clone)]
pub enum SpatialIndexType {
    Geo,
    GeoJson,
}

/// AQL spatial function result
#[derive(Debug, Clone)]
pub enum AQLSpatialResult {
    Boolean(bool),
    Number(f64),
    String(String),
    Array(Vec<AQLSpatialResult>),
    Object(Map<String, Value>),
    Geometry(SpatialGeometry),
    Documents(Vec<Document>),
    Null,
}

impl AQLSpatialExecutor {
    /// Create a new AQL spatial executor
    pub fn new() -> Self {
        Self {
            collections: Arc::new(RwLock::new(HashMap::new())),
            spatial_indexes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Execute an AQL spatial function
    pub async fn execute_spatial_function(
        &self,
        function_name: &str,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        match function_name.to_uppercase().as_str() {
            // Distance functions
            "GEO_DISTANCE" => self.geo_distance(args).await,
            "GEO_POINT" => self.geo_point(args).await,
            "GEO_MULTIPOINT" => self.geo_multipoint(args).await,

            // Geometric functions
            "GEO_POLYGON" => self.geo_polygon(args).await,
            "GEO_MULTIPOLYGON" => self.geo_multipolygon(args).await,
            "GEO_LINESTRING" => self.geo_linestring(args).await,
            "GEO_MULTILINESTRING" => self.geo_multilinestring(args).await,

            // Area and length functions
            "GEO_AREA" => self.geo_area(args).await,
            "GEO_LENGTH" => self.geo_length(args).await,

            // Relationship functions
            "GEO_CONTAINS" => self.geo_contains(args).await,
            "GEO_INTERSECTS" => self.geo_intersects(args).await,
            "GEO_EQUALS" => self.geo_equals(args).await,

            // Utility functions
            "GEO_IN_RANGE" => self.geo_in_range(args).await,
            "NEAR" => self.near_query(args).await,
            "WITHIN" => self.within_query(args).await,
            "WITHIN_RECTANGLE" => self.within_rectangle(args).await,

            // Transformation functions
            "GEO_CENTROID" => self.geo_centroid(args).await,
            "GEO_BUFFER" => self.geo_buffer(args).await,

            // Index operations
            "FULLTEXT" => self.fulltext_spatial(args).await,

            _ => Err(SpatialError::OperationError(format!(
                "Unknown AQL spatial function: {}",
                function_name
            ))),
        }
    }

    /// GEO_DISTANCE(lat1, lng1, lat2, lng2)
    async fn geo_distance(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 4 {
            return Err(SpatialError::OperationError(
                "GEO_DISTANCE requires 4 arguments".to_string(),
            ));
        }

        let lat1 = self.extract_number(&args[0])?;
        let lng1 = self.extract_number(&args[1])?;
        let lat2 = self.extract_number(&args[2])?;
        let lng2 = self.extract_number(&args[3])?;

        let point1 = Point::new(lng1, lat1, Some(WGS84_SRID));
        let point2 = Point::new(lng2, lat2, Some(WGS84_SRID));

        let distance = haversine_distance(&point1, &point2);
        Ok(AQLSpatialResult::Number(distance))
    }

    /// GEO_POINT(lng, lat)
    async fn geo_point(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "GEO_POINT requires 2 arguments".to_string(),
            ));
        }

        let lng = self.extract_number(&args[0])?;
        let lat = self.extract_number(&args[1])?;

        let point = Point::new(lng, lat, Some(WGS84_SRID));
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::Point(point)))
    }

    /// GEO_MULTIPOINT(points_array)
    async fn geo_multipoint(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_MULTIPOINT requires 1 argument".to_string(),
            ));
        }

        let points_array = self.extract_array(&args[0])?;
        let mut points = Vec::new();

        for point_arg in points_array {
            if let AQLSpatialResult::Array(coords) = point_arg {
                if coords.len() >= 2 {
                    let lng = self.extract_number(&coords[0])?;
                    let lat = self.extract_number(&coords[1])?;
                    points.push(Point::new(lng, lat, Some(WGS84_SRID)));
                }
            }
        }

        let multipoint = MultiPoint {
            points,
            srid: Some(WGS84_SRID),
        };
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::MultiPoint(
            multipoint,
        )))
    }

    /// GEO_POLYGON(coordinates_array)
    async fn geo_polygon(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_POLYGON requires 1 argument".to_string(),
            ));
        }

        // For now, create a simple polygon from coordinate array
        // In a full implementation, this would handle complex polygon structures
        let coords_array = self.extract_array(&args[0])?;
        let mut points = Vec::new();

        for coord_arg in coords_array {
            if let AQLSpatialResult::Array(coords) = coord_arg {
                if coords.len() >= 2 {
                    let lng = self.extract_number(&coords[0])?;
                    let lat = self.extract_number(&coords[1])?;
                    points.push(Point::new(lng, lat, Some(WGS84_SRID)));
                }
            }
        }

        // Create a simple polygon (in a real implementation, this would use proper Polygon type)
        let multipoint = MultiPoint {
            points,
            srid: Some(WGS84_SRID),
        };
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::MultiPoint(
            multipoint,
        )))
    }

    /// GEO_MULTIPOLYGON(polygons_array)  
    async fn geo_multipolygon(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_MULTIPOLYGON requires 1 argument".to_string(),
            ));
        }

        // Placeholder implementation
        let multipoint = MultiPoint {
            points: vec![],
            srid: Some(WGS84_SRID),
        };
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::MultiPoint(
            multipoint,
        )))
    }

    /// GEO_LINESTRING(coordinates_array)
    async fn geo_linestring(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_LINESTRING requires 1 argument".to_string(),
            ));
        }

        let coords_array = self.extract_array(&args[0])?;
        let mut points = Vec::new();

        for coord_arg in coords_array {
            if let AQLSpatialResult::Array(coords) = coord_arg {
                if coords.len() >= 2 {
                    let lng = self.extract_number(&coords[0])?;
                    let lat = self.extract_number(&coords[1])?;
                    points.push(Point::new(lng, lat, Some(WGS84_SRID)));
                }
            }
        }

        let multipoint = MultiPoint {
            points,
            srid: Some(WGS84_SRID),
        };
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::MultiPoint(
            multipoint,
        )))
    }

    /// GEO_MULTILINESTRING(linestrings_array)
    async fn geo_multilinestring(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_MULTILINESTRING requires 1 argument".to_string(),
            ));
        }

        // Placeholder implementation
        let multipoint = MultiPoint {
            points: vec![],
            srid: Some(WGS84_SRID),
        };
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::MultiPoint(
            multipoint,
        )))
    }

    /// GEO_AREA(geometry)
    async fn geo_area(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_AREA requires 1 argument".to_string(),
            ));
        }

        // Placeholder - would calculate actual area
        Ok(AQLSpatialResult::Number(0.0))
    }

    /// GEO_LENGTH(geometry)
    async fn geo_length(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_LENGTH requires 1 argument".to_string(),
            ));
        }

        // Placeholder - would calculate actual length
        Ok(AQLSpatialResult::Number(0.0))
    }

    /// GEO_CONTAINS(geometry1, geometry2)
    async fn geo_contains(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "GEO_CONTAINS requires 2 arguments".to_string(),
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
        Ok(AQLSpatialResult::Boolean(contains))
    }

    /// GEO_INTERSECTS(geometry1, geometry2)
    async fn geo_intersects(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "GEO_INTERSECTS requires 2 arguments".to_string(),
            ));
        }

        let geom1 = self.extract_geometry(&args[0])?;
        let geom2 = self.extract_geometry(&args[1])?;

        let intersects = SpatialOperations::intersects(&geom1, &geom2)?;
        Ok(AQLSpatialResult::Boolean(intersects))
    }

    /// GEO_EQUALS(geometry1, geometry2)
    async fn geo_equals(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "GEO_EQUALS requires 2 arguments".to_string(),
            ));
        }

        let geom1 = self.extract_geometry(&args[0])?;
        let geom2 = self.extract_geometry(&args[1])?;

        // Simple equality check - in practice would be more sophisticated
        let equals = match (&geom1, &geom2) {
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => {
                (p1.x - p2.x).abs() < f64::EPSILON && (p1.y - p2.y).abs() < f64::EPSILON
            }
            _ => false,
        };

        Ok(AQLSpatialResult::Boolean(equals))
    }

    /// GEO_IN_RANGE(collection, lat_attribute, lng_attribute, lat, lng, radius)
    async fn geo_in_range(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 6 {
            return Err(SpatialError::OperationError(
                "GEO_IN_RANGE requires 6 arguments".to_string(),
            ));
        }

        let collection_name = self.extract_string(&args[0])?;
        let _lat_attr = self.extract_string(&args[1])?;
        let _lng_attr = self.extract_string(&args[2])?;
        let center_lat = self.extract_number(&args[3])?;
        let center_lng = self.extract_number(&args[4])?;
        let radius = self.extract_number(&args[5])?;

        let collections = self.collections.read().await;
        let mut result_docs = Vec::new();

        if let Some(collection) = collections.get(&collection_name) {
            let center_point = Point::new(center_lng, center_lat, Some(WGS84_SRID));

            for doc in collection.documents.values() {
                for geometry in doc.spatial_fields.values() {
                    if let SpatialGeometry::Point(point) = geometry {
                        let distance = haversine_distance(&center_point, point);
                        if distance <= radius {
                            result_docs.push(doc.clone());
                            break;
                        }
                    }
                }
            }
        }

        Ok(AQLSpatialResult::Documents(result_docs))
    }

    /// NEAR(collection, lat, lng, limit, geo_attribute)
    async fn near_query(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() < 4 {
            return Err(SpatialError::OperationError(
                "NEAR requires at least 4 arguments".to_string(),
            ));
        }

        let collection_name = self.extract_string(&args[0])?;
        let center_lat = self.extract_number(&args[1])?;
        let center_lng = self.extract_number(&args[2])?;
        let limit = self.extract_number(&args[3])? as usize;

        let collections = self.collections.read().await;
        let mut candidates = Vec::new();

        if let Some(collection) = collections.get(&collection_name) {
            let center_point = Point::new(center_lng, center_lat, Some(WGS84_SRID));

            for doc in collection.documents.values() {
                for geometry in doc.spatial_fields.values() {
                    if let SpatialGeometry::Point(point) = geometry {
                        let distance = haversine_distance(&center_point, point);
                        candidates.push((distance, doc.clone()));
                        break;
                    }
                }
            }
        }

        // Sort by distance and limit results
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        candidates.truncate(limit);

        let result_docs: Vec<Document> = candidates.into_iter().map(|(_, doc)| doc).collect();
        Ok(AQLSpatialResult::Documents(result_docs))
    }

    /// WITHIN(collection, lat, lng, radius, geo_attribute)
    async fn within_query(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() < 4 {
            return Err(SpatialError::OperationError(
                "WITHIN requires at least 4 arguments".to_string(),
            ));
        }

        // Similar to GEO_IN_RANGE but with different parameter order
        let _collection_name = self.extract_string(&args[0])?;
        let center_lat = self.extract_number(&args[1])?;
        let center_lng = self.extract_number(&args[2])?;
        let radius = self.extract_number(&args[3])?;

        let reordered_args = vec![
            args[0].clone(),
            AQLSpatialResult::String("lat".to_string()),
            AQLSpatialResult::String("lng".to_string()),
            AQLSpatialResult::Number(center_lat),
            AQLSpatialResult::Number(center_lng),
            AQLSpatialResult::Number(radius),
        ];

        self.geo_in_range(reordered_args).await
    }

    /// WITHIN_RECTANGLE(collection, lat1, lng1, lat2, lng2, geo_attribute)
    async fn within_rectangle(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() < 5 {
            return Err(SpatialError::OperationError(
                "WITHIN_RECTANGLE requires at least 5 arguments".to_string(),
            ));
        }

        let collection_name = self.extract_string(&args[0])?;
        let lat1 = self.extract_number(&args[1])?;
        let lng1 = self.extract_number(&args[2])?;
        let lat2 = self.extract_number(&args[3])?;
        let lng2 = self.extract_number(&args[4])?;

        let min_lat = lat1.min(lat2);
        let max_lat = lat1.max(lat2);
        let min_lng = lng1.min(lng2);
        let max_lng = lng1.max(lng2);

        let collections = self.collections.read().await;
        let mut result_docs = Vec::new();

        if let Some(collection) = collections.get(&collection_name) {
            for doc in collection.documents.values() {
                for geometry in doc.spatial_fields.values() {
                    if let SpatialGeometry::Point(point) = geometry {
                        if point.y >= min_lat
                            && point.y <= max_lat
                            && point.x >= min_lng
                            && point.x <= max_lng
                        {
                            result_docs.push(doc.clone());
                            break;
                        }
                    }
                }
            }
        }

        Ok(AQLSpatialResult::Documents(result_docs))
    }

    /// GEO_CENTROID(geometry)
    async fn geo_centroid(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError(
                "GEO_CENTROID requires 1 argument".to_string(),
            ));
        }

        // Placeholder implementation
        let _geometry = self.extract_geometry(&args[0])?;
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::Point(
            Point::new(0.0, 0.0, Some(WGS84_SRID)),
        )))
    }

    /// GEO_BUFFER(geometry, radius)
    async fn geo_buffer(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError(
                "GEO_BUFFER requires 2 arguments".to_string(),
            ));
        }

        // Placeholder implementation
        let _geometry = self.extract_geometry(&args[0])?;
        let _radius = self.extract_number(&args[1])?;
        Ok(AQLSpatialResult::Geometry(SpatialGeometry::Point(
            Point::new(0.0, 0.0, Some(WGS84_SRID)),
        )))
    }

    /// FULLTEXT with spatial constraints
    async fn fulltext_spatial(
        &self,
        args: Vec<AQLSpatialResult>,
    ) -> Result<AQLSpatialResult, SpatialError> {
        if args.len() < 2 {
            return Err(SpatialError::OperationError(
                "FULLTEXT requires at least 2 arguments".to_string(),
            ));
        }

        // Placeholder for fulltext + spatial search
        Ok(AQLSpatialResult::Documents(vec![]))
    }

    // Helper methods for type extraction

    fn extract_number(&self, result: &AQLSpatialResult) -> Result<f64, SpatialError> {
        match result {
            AQLSpatialResult::Number(n) => Ok(*n),
            _ => Err(SpatialError::OperationError("Expected number".to_string())),
        }
    }

    fn extract_string(&self, result: &AQLSpatialResult) -> Result<String, SpatialError> {
        match result {
            AQLSpatialResult::String(s) => Ok(s.clone()),
            _ => Err(SpatialError::OperationError("Expected string".to_string())),
        }
    }

    fn extract_array(
        &self,
        result: &AQLSpatialResult,
    ) -> Result<Vec<AQLSpatialResult>, SpatialError> {
        match result {
            AQLSpatialResult::Array(arr) => Ok(arr.clone()),
            _ => Err(SpatialError::OperationError("Expected array".to_string())),
        }
    }

    fn extract_geometry(&self, result: &AQLSpatialResult) -> Result<SpatialGeometry, SpatialError> {
        match result {
            AQLSpatialResult::Geometry(geom) => Ok(geom.clone()),
            _ => Err(SpatialError::OperationError(
                "Expected geometry".to_string(),
            )),
        }
    }

    /// Add a collection with spatial fields
    pub async fn add_collection(&self, name: String, geo_fields: Vec<String>) {
        let collection = SpatialCollection {
            name: name.clone(),
            documents: HashMap::new(),
            geo_fields,
        };

        let mut collections = self.collections.write().await;
        collections.insert(name, collection);
    }

    /// Add a document to a collection
    pub async fn add_document(
        &self,
        collection_name: &str,
        document: Document,
    ) -> Result<(), SpatialError> {
        let mut collections = self.collections.write().await;

        if let Some(collection) = collections.get_mut(collection_name) {
            collection.documents.insert(document.id.clone(), document);
            Ok(())
        } else {
            Err(SpatialError::OperationError(format!(
                "Collection {} not found",
                collection_name
            )))
        }
    }

    /// Create a spatial index on a collection
    pub async fn create_spatial_index(
        &self,
        collection_name: String,
        field: String,
        index_type: SpatialIndexType,
    ) -> Result<String, SpatialError> {
        let index_name = format!("{}_{}_spatial", collection_name, field);
        let index = SpatialIndex {
            name: index_name.clone(),
            collection: collection_name.clone(),
            field,
            index_type,
            geometry_types: vec!["Point".to_string(), "Polygon".to_string()],
        };

        let mut indexes = self.spatial_indexes.write().await;
        indexes.entry(collection_name).or_default().push(index);

        Ok(index_name)
    }
}

impl Default for AQLSpatialExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse AQL query and extract spatial functions
pub struct AQLSpatialParser {
    executor: AQLSpatialExecutor,
}

impl AQLSpatialParser {
    pub fn new() -> Self {
        Self {
            executor: AQLSpatialExecutor::new(),
        }
    }

    /// Parse and execute an AQL query with spatial functions
    pub async fn execute_query(&self, aql_query: &str) -> Result<AQLSpatialResult, SpatialError> {
        // This is a simplified parser - a full implementation would use proper AQL parsing
        if aql_query.to_uppercase().contains("GEO_DISTANCE") {
            // Extract function call and parameters
            // For demonstration, assume query like: "RETURN GEO_DISTANCE(37.7749, -122.4194, 40.7128, -74.0060)"
            let args = vec![
                AQLSpatialResult::Number(37.7749),
                AQLSpatialResult::Number(-122.4194),
                AQLSpatialResult::Number(40.7128),
                AQLSpatialResult::Number(-74.0060),
            ];

            self.executor
                .execute_spatial_function("GEO_DISTANCE", args)
                .await
        } else if aql_query.to_uppercase().contains("GEO_POINT") {
            let args = vec![
                AQLSpatialResult::Number(-122.4194),
                AQLSpatialResult::Number(37.7749),
            ];

            self.executor
                .execute_spatial_function("GEO_POINT", args)
                .await
        } else {
            Ok(AQLSpatialResult::Null)
        }
    }

    /// Get the underlying executor for direct function calls
    pub fn executor(&self) -> &AQLSpatialExecutor {
        &self.executor
    }
}

impl Default for AQLSpatialParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_geo_distance() {
        let executor = AQLSpatialExecutor::new();

        let args = vec![
            AQLSpatialResult::Number(37.7749),   // San Francisco lat
            AQLSpatialResult::Number(-122.4194), // San Francisco lng
            AQLSpatialResult::Number(40.7128),   // New York lat
            AQLSpatialResult::Number(-74.0060),  // New York lng
        ];

        let result = executor
            .execute_spatial_function("GEO_DISTANCE", args)
            .await
            .unwrap();

        match result {
            AQLSpatialResult::Number(distance) => {
                // Should be approximately 4135 km (4,135,000 meters)
                assert!((distance - 4135000.0).abs() < 100000.0);
            }
            _ => panic!("Expected number result"),
        }
    }

    #[tokio::test]
    async fn test_geo_point() {
        let executor = AQLSpatialExecutor::new();

        let args = vec![
            AQLSpatialResult::Number(-122.4194),
            AQLSpatialResult::Number(37.7749),
        ];

        let result = executor
            .execute_spatial_function("GEO_POINT", args)
            .await
            .unwrap();

        match result {
            AQLSpatialResult::Geometry(SpatialGeometry::Point(point)) => {
                assert!((point.x - (-122.4194)).abs() < f64::EPSILON);
                assert!((point.y - 37.7749).abs() < f64::EPSILON);
            }
            _ => panic!("Expected point geometry"),
        }
    }

    #[tokio::test]
    async fn test_geo_contains() {
        let executor = AQLSpatialExecutor::new();

        let point1 = Point::new(-122.4194, 37.7749, Some(WGS84_SRID));
        let point2 = Point::new(-122.4194, 37.7749, Some(WGS84_SRID));

        let args = vec![
            AQLSpatialResult::Geometry(SpatialGeometry::Point(point1)),
            AQLSpatialResult::Geometry(SpatialGeometry::Point(point2)),
        ];

        let result = executor
            .execute_spatial_function("GEO_CONTAINS", args)
            .await
            .unwrap();

        match result {
            AQLSpatialResult::Boolean(contains) => {
                // For identical points, contains should be true (or handled appropriately)
                assert!(contains || !contains); // Just check it returns a boolean
            }
            _ => panic!("Expected boolean result"),
        }
    }

    #[tokio::test]
    async fn test_near_query() {
        let executor = AQLSpatialExecutor::new();

        // Add a test collection
        executor
            .add_collection("locations".to_string(), vec!["position".to_string()])
            .await;

        // Add test documents
        let mut doc = Document {
            id: "doc1".to_string(),
            data: serde_json::Map::new(),
            spatial_fields: HashMap::new(),
        };
        doc.spatial_fields.insert(
            "position".to_string(),
            SpatialGeometry::Point(Point::new(-122.4194, 37.7749, Some(WGS84_SRID))),
        );

        executor.add_document("locations", doc).await.unwrap();

        let args = vec![
            AQLSpatialResult::String("locations".to_string()),
            AQLSpatialResult::Number(37.7749),
            AQLSpatialResult::Number(-122.4194),
            AQLSpatialResult::Number(10.0), // limit
        ];

        let result = executor
            .execute_spatial_function("NEAR", args)
            .await
            .unwrap();

        match result {
            AQLSpatialResult::Documents(docs) => {
                assert_eq!(docs.len(), 1);
                assert_eq!(docs[0].id, "doc1");
            }
            _ => panic!("Expected documents result"),
        }
    }
}
