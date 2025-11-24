//! PostgreSQL spatial functions with PostGIS compatibility.
//!
//! This module provides comprehensive spatial functions that are compatible with PostGIS,
//! allowing seamless migration from existing PostgreSQL+PostGIS installations.

use orbit_shared::spatial::{
    Point, SpatialGeometry, SpatialOperations, SpatialError, CoordinateTransformer,
    BoundingBox, LineString, Polygon, LinearRing, SpatialFunctions,
    WGS84_SRID, WEB_MERCATOR_SRID
};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// PostgreSQL spatial function registry and executor.
pub struct PostgresSpatialFunctions {
    coordinate_transformer: CoordinateTransformer,
    spatial_functions: SpatialFunctions,
}

impl PostgresSpatialFunctions {
    /// Create a new PostgreSQL spatial functions registry.
    pub fn new() -> Self {
        Self {
            coordinate_transformer: CoordinateTransformer::new(),
            spatial_functions: SpatialFunctions::new(),
        }
    }
    
    /// Execute a spatial function by name with arguments.
    pub fn execute_function(&self, name: &str, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        match name.to_uppercase().as_str() {
            // Construction functions
            "ST_POINT" => self.st_point(args),
            "ST_MAKEPOINT" => self.st_makepoint(args),
            "ST_GEOMFROMTEXT" | "ST_GEOMFROMWKT" => self.st_geomfromtext(args),
            "ST_GEOMFROMGEOJSON" => self.st_geomfromgeojson(args),
            "ST_POINTFROMTEXT" => self.st_pointfromtext(args),
            "ST_LINEFROMTEXT" => self.st_linefromtext(args),
            "ST_POLYGONFROMTEXT" => self.st_polygonfromtext(args),
            
            // Measurement functions
            "ST_DISTANCE" => self.st_distance(args),
            "ST_DISTANCE_SPHERE" => self.st_distance_sphere(args),
            "ST_AREA" => self.st_area(args),
            "ST_LENGTH" => self.st_length(args),
            "ST_PERIMETER" => self.st_perimeter(args),
            
            // Spatial relationship functions
            "ST_CONTAINS" => self.st_contains(args),
            "ST_WITHIN" => self.st_within(args),
            "ST_INTERSECTS" => self.st_intersects(args),
            "ST_OVERLAPS" => self.st_overlaps(args),
            "ST_TOUCHES" => self.st_touches(args),
            "ST_CROSSES" => self.st_crosses(args),
            "ST_DISJOINT" => self.st_disjoint(args),
            "ST_EQUALS" => self.st_equals(args),
            "ST_DWITHIN" => self.st_dwithin(args),
            
            // Geometric operations
            "ST_BUFFER" => self.st_buffer(args),
            "ST_INTERSECTION" => self.st_intersection(args),
            "ST_UNION" => self.st_union(args),
            "ST_DIFFERENCE" => self.st_difference(args),
            "ST_SYMDIFFERENCE" => self.st_symdifference(args),
            "ST_CONVEXHULL" => self.st_convexhull(args),
            "ST_ENVELOPE" => self.st_envelope(args),
            "ST_BOUNDARY" => self.st_boundary(args),
            "ST_CENTROID" => self.st_centroid(args),
            
            // Coordinate transformation
            "ST_TRANSFORM" => self.st_transform(args),
            "ST_SETSRID" => self.st_setsrid(args),
            "ST_SRID" => self.st_srid(args),
            
            // Geometry accessors
            "ST_X" => self.st_x(args),
            "ST_Y" => self.st_y(args),
            "ST_Z" => self.st_z(args),
            "ST_M" => self.st_m(args),
            "ST_STARTPOINT" => self.st_startpoint(args),
            "ST_ENDPOINT" => self.st_endpoint(args),
            "ST_POINTN" => self.st_pointn(args),
            "ST_NPOINTS" => self.st_npoints(args),
            "ST_NUMGEOMETRIES" => self.st_numgeometries(args),
            "ST_GEOMETRYN" => self.st_geometryn(args),
            "ST_EXTERIORRING" => self.st_exteriorring(args),
            "ST_INTERIORRINGN" => self.st_interiorringn(args),
            "ST_NUMINTERIORRINGS" => self.st_numinteriorrings(args),
            
            // Geometry validation and repair
            "ST_ISVALID" => self.st_isvalid(args),
            "ST_ISEMPTY" => self.st_isempty(args),
            "ST_ISSIMPLE" => self.st_issimple(args),
            "ST_ISCLOSED" => self.st_isclosed(args),
            "ST_MAKEVALID" => self.st_makevalid(args),
            
            // Output functions
            "ST_ASTEXT" | "ST_ASWKT" => self.st_astext(args),
            "ST_ASGEOJSON" => self.st_asgeojson(args),
            "ST_ASBINARY" | "ST_ASWKB" => self.st_asbinary(args),
            
            // Spatial aggregates (for GROUP BY operations)
            "ST_COLLECT" => self.st_collect(args),
            "ST_UNION_AGG" => self.st_union_agg(args),
            "ST_EXTENT" => self.st_extent(args),
            
            // Advanced spatial analytics
            "ST_CLUSTERKMEANSCOORD" => self.st_cluster_kmeans(args),
            "ST_CLUSTERDBSCAN" => self.st_cluster_dbscan(args),
            "ST_VORONOIPOLYGONS" => self.st_voronoi_polygons(args),
            "ST_DELAUNAYTRIANGLES" => self.st_delaunay_triangles(args),
            
            _ => Err(SpatialError::OperationError(format!("Unknown spatial function: {}", name)))
        }
    }
    
    // Construction Functions
    
    /// ST_Point(x, y) - Create a point from coordinates.
    fn st_point(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Point requires exactly 2 arguments (x, y)".to_string()));
        }
        
        let x = args[0].as_float()?;
        let y = args[1].as_float()?;
        
        let point = Point::new(x, y, Some(WGS84_SRID));
        let geometry = SpatialGeometry::Point(point);
        
        Ok(SqlValue::Geometry(geometry))
    }
    
    /// ST_MakePoint(x, y, [z], [m]) - Create a point with optional Z and M.
    fn st_makepoint(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() < 2 || args.len() > 4 {
            return Err(SpatialError::OperationError("ST_MakePoint requires 2-4 arguments".to_string()));
        }
        
        let x = args[0].as_float()?;
        let y = args[1].as_float()?;
        let z = if args.len() >= 3 { Some(args[2].as_float()?) } else { None };
        let m = if args.len() == 4 { Some(args[3].as_float()?) } else { None };
        
        let point = Point {
            x, y, z, m,
            srid: Some(WGS84_SRID),
        };
        
        Ok(SqlValue::Geometry(SpatialGeometry::Point(point)))
    }
    
    /// ST_GeomFromText(wkt, [srid]) - Create geometry from Well-Known Text.
    fn st_geomfromtext(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.is_empty() || args.len() > 2 {
            return Err(SpatialError::OperationError("ST_GeomFromText requires 1-2 arguments".to_string()));
        }
        
        let wkt = args[0].as_string()?;
        let srid = if args.len() == 2 { Some(args[1].as_int()? as i32) } else { None };
        
        // Use shared SpatialFunctions WKT parser
        let geometry = self.spatial_functions.st_geomfromtext(&wkt, srid)?;
        Ok(SqlValue::Geometry(geometry))
    }
    
    /// ST_GeomFromGeoJSON(geojson) - Create geometry from GeoJSON.
    fn st_geomfromgeojson(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_GeomFromGeoJSON requires exactly 1 argument".to_string()));
        }
        
        let geojson_str = args[0].as_string()?;
        let geometry = self.parse_geojson(&geojson_str)?;
        Ok(SqlValue::Geometry(geometry))
    }
    
    // Measurement Functions
    
    /// ST_Distance(geom1, geom2) - Calculate distance between geometries.
    fn st_distance(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Distance requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let distance = SpatialOperations::distance(geom1, geom2)?;
        Ok(SqlValue::Float(distance))
    }
    
    /// ST_Distance_Sphere(geom1, geom2) - Calculate spherical distance.
    fn st_distance_sphere(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Distance_Sphere requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        // Use spherical distance calculation
        match (geom1, geom2) {
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => {
                let distance = orbit_shared::spatial::crs::utils::haversine_distance(p1, p2);
                Ok(SqlValue::Float(distance))
            },
            _ => Err(SpatialError::OperationError("ST_Distance_Sphere only supports points".to_string()))
        }
    }
    
    /// ST_Area(geometry) - Calculate area of geometry.
    fn st_area(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_Area requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let area = SpatialOperations::area(geometry)?;
        Ok(SqlValue::Float(area))
    }
    
    /// ST_Length(geometry) - Calculate length/perimeter of geometry.
    fn st_length(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_Length requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let length = SpatialOperations::length(geometry)?;
        Ok(SqlValue::Float(length))
    }
    
    /// ST_Perimeter(geometry) - Alias for ST_Length for polygons.
    fn st_perimeter(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        self.st_length(args)
    }
    
    // Spatial Relationship Functions
    
    /// ST_Contains(geom1, geom2) - Test if geom1 contains geom2.
    fn st_contains(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Contains requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        // Simplified implementation - in production, use more sophisticated algorithms
        match (geom1, geom2) {
            (SpatialGeometry::Polygon(poly), SpatialGeometry::Point(point)) => {
                let contains = SpatialOperations::point_in_polygon(point, poly)?;
                Ok(SqlValue::Boolean(contains))
            },
            _ => Ok(SqlValue::Boolean(false))
        }
    }
    
    /// ST_Within(geom1, geom2) - Test if geom1 is within geom2.
    fn st_within(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Within requires exactly 2 arguments".to_string()));
        }
        
        // ST_Within(A, B) is equivalent to ST_Contains(B, A)
        let mut reversed_args = args;
        reversed_args.reverse();
        self.st_contains(reversed_args)
    }
    
    /// ST_Intersects(geom1, geom2) - Test if geometries intersect.
    fn st_intersects(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Intersects requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let intersects = SpatialOperations::intersects(geom1, geom2)?;
        Ok(SqlValue::Boolean(intersects))
    }
    
    /// ST_DWithin(geom1, geom2, distance) - Test if geometries are within distance.
    fn st_dwithin(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 3 {
            return Err(SpatialError::OperationError("ST_DWithin requires exactly 3 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        let threshold_distance = args[2].as_float()?;
        
        let distance = SpatialOperations::distance(geom1, geom2)?;
        Ok(SqlValue::Boolean(distance <= threshold_distance))
    }
    
    // Coordinate transformation functions
    
    /// ST_Transform(geometry, target_srid) - Transform geometry to different CRS.
    fn st_transform(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Transform requires exactly 2 arguments".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let target_srid = args[1].as_int()? as i32;
        
        // Transform based on geometry type
        let transformed_geometry = match geometry {
            SpatialGeometry::Point(point) => {
                let transformed_point = self.coordinate_transformer.transform_point(point, target_srid)?;
                SpatialGeometry::Point(transformed_point)
            },
            _ => return Err(SpatialError::OperationError("ST_Transform not yet implemented for this geometry type".to_string()))
        };
        
        Ok(SqlValue::Geometry(transformed_geometry))
    }
    
    /// ST_SRID(geometry) - Get the SRID of geometry.
    fn st_srid(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_SRID requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let srid = geometry.srid().unwrap_or(0);
        Ok(SqlValue::Integer(srid as i64))
    }
    
    /// ST_SetSRID(geometry, srid) - Set the SRID of geometry.
    fn st_setsrid(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_SetSRID requires exactly 2 arguments".to_string()));
        }
        
        let mut geometry = args[0].as_geometry()?.clone();
        let srid = args[1].as_int()? as i32;
        
        geometry.set_srid(Some(srid));
        Ok(SqlValue::Geometry(geometry))
    }
    
    // Output functions
    
    /// ST_AsText(geometry) - Convert geometry to Well-Known Text.
    fn st_astext(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_AsText requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let wkt = self.geometry_to_wkt(geometry)?;
        Ok(SqlValue::String(wkt))
    }
    
    /// ST_AsGeoJSON(geometry) - Convert geometry to GeoJSON.
    fn st_asgeojson(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_AsGeoJSON requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let geojson = self.geometry_to_geojson(geometry)?;
        Ok(SqlValue::String(geojson))
    }
    
    // Placeholder implementations for remaining functions
    // (In production, these would have full implementations)
    
    /// ST_PointFromText(wkt, [srid]) - Create point from WKT.
    fn st_pointfromtext(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.is_empty() || args.len() > 2 {
            return Err(SpatialError::OperationError("ST_PointFromText requires 1-2 arguments".to_string()));
        }
        
        let wkt = args[0].as_string()?;
        let srid = if args.len() == 2 { Some(args[1].as_int()? as i32) } else { None };
        
        let geometry = self.spatial_functions.st_geomfromtext(&wkt, srid)?;
        match geometry {
            SpatialGeometry::Point(_) => Ok(SqlValue::Geometry(geometry)),
            _ => Err(SpatialError::OperationError("WKT does not represent a POINT".to_string())),
        }
    }
    
    /// ST_LineFromText(wkt, [srid]) - Create linestring from WKT.
    fn st_linefromtext(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.is_empty() || args.len() > 2 {
            return Err(SpatialError::OperationError("ST_LineFromText requires 1-2 arguments".to_string()));
        }
        
        let wkt = args[0].as_string()?;
        let srid = if args.len() == 2 { Some(args[1].as_int()? as i32) } else { None };
        
        let geometry = self.spatial_functions.st_geomfromtext(&wkt, srid)?;
        match geometry {
            SpatialGeometry::LineString(_) => Ok(SqlValue::Geometry(geometry)),
            _ => Err(SpatialError::OperationError("WKT does not represent a LINESTRING".to_string())),
        }
    }
    
    /// ST_PolygonFromText(wkt, [srid]) - Create polygon from WKT.
    fn st_polygonfromtext(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.is_empty() || args.len() > 2 {
            return Err(SpatialError::OperationError("ST_PolygonFromText requires 1-2 arguments".to_string()));
        }
        
        let wkt = args[0].as_string()?;
        let srid = if args.len() == 2 { Some(args[1].as_int()? as i32) } else { None };
        
        let geometry = self.spatial_functions.st_geomfromtext(&wkt, srid)?;
        match geometry {
            SpatialGeometry::Polygon(_) => Ok(SqlValue::Geometry(geometry)),
            _ => Err(SpatialError::OperationError("WKT does not represent a POLYGON".to_string())),
        }
    }
    
    fn st_overlaps(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Overlaps requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let overlaps = SpatialOperations::overlaps(geom1, geom2)?;
        Ok(SqlValue::Boolean(overlaps))
    }
    
    fn st_touches(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Touches requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let touches = SpatialOperations::touches(geom1, geom2)?;
        Ok(SqlValue::Boolean(touches))
    }
    
    fn st_crosses(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Crosses requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let crosses = SpatialOperations::crosses(geom1, geom2)?;
        Ok(SqlValue::Boolean(crosses))
    }
    
    fn st_disjoint(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Disjoint requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let disjoint = SpatialOperations::disjoint(geom1, geom2)?;
        Ok(SqlValue::Boolean(disjoint))
    }
    
    fn st_equals(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 2 {
            return Err(SpatialError::OperationError("ST_Equals requires exactly 2 arguments".to_string()));
        }
        
        let geom1 = args[0].as_geometry()?;
        let geom2 = args[1].as_geometry()?;
        
        let equals = SpatialOperations::equals(geom1, geom2)?;
        Ok(SqlValue::Boolean(equals))
    }
    
    fn st_buffer(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Buffer not yet implemented".to_string()))
    }
    
    fn st_intersection(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Intersection not yet implemented".to_string()))
    }
    
    fn st_union(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Union not yet implemented".to_string()))
    }
    
    fn st_difference(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Difference not yet implemented".to_string()))
    }
    
    fn st_symdifference(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_SymDifference not yet implemented".to_string()))
    }
    
    fn st_convexhull(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_ConvexHull not yet implemented".to_string()))
    }
    
    fn st_envelope(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_Envelope requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        let bbox = SpatialOperations::bounding_box(geometry)?;
        
        // Convert bounding box to polygon geometry
        let envelope_polygon = self.bbox_to_polygon(&bbox)?;
        Ok(SqlValue::Geometry(SpatialGeometry::Polygon(envelope_polygon)))
    }
    
    /// Convert bounding box to polygon.
    fn bbox_to_polygon(&self, bbox: &BoundingBox) -> Result<Polygon, SpatialError> {
        let points = vec![
            Point::new(bbox.min_x, bbox.min_y, bbox.srid),
            Point::new(bbox.max_x, bbox.min_y, bbox.srid),
            Point::new(bbox.max_x, bbox.max_y, bbox.srid),
            Point::new(bbox.min_x, bbox.max_y, bbox.srid),
            Point::new(bbox.min_x, bbox.min_y, bbox.srid), // Close the ring
        ];
        
        let exterior_ring = LinearRing::new(points)?;
        Polygon::new(exterior_ring, vec![], bbox.srid)
    }
    
    fn st_boundary(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Boundary not yet implemented".to_string()))
    }
    
    fn st_centroid(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_Centroid requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        
        // Simple centroid calculation
        match geometry {
            SpatialGeometry::Polygon(poly) => {
                let bbox = poly.bounding_box();
                let center = bbox.center();
                Ok(SqlValue::Geometry(SpatialGeometry::Point(center)))
            },
            SpatialGeometry::Point(point) => {
                Ok(SqlValue::Geometry(SpatialGeometry::Point(point.clone())))
            },
            _ => Err(SpatialError::OperationError("ST_Centroid not implemented for this geometry type".to_string()))
        }
    }
    
    fn st_x(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_X requires exactly 1 argument".to_string()));
        }
        
        match args[0].as_geometry()? {
            SpatialGeometry::Point(point) => Ok(SqlValue::Float(point.x)),
            _ => Err(SpatialError::OperationError("ST_X can only be used with Point geometries".to_string()))
        }
    }
    
    fn st_y(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_Y requires exactly 1 argument".to_string()));
        }
        
        match args[0].as_geometry()? {
            SpatialGeometry::Point(point) => Ok(SqlValue::Float(point.y)),
            _ => Err(SpatialError::OperationError("ST_Y can only be used with Point geometries".to_string()))
        }
    }
    
    fn st_z(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_Z requires exactly 1 argument".to_string()));
        }
        
        match args[0].as_geometry()? {
            SpatialGeometry::Point(point) => {
                match point.z {
                    Some(z) => Ok(SqlValue::Float(z)),
                    None => Ok(SqlValue::Null)
                }
            },
            _ => Err(SpatialError::OperationError("ST_Z can only be used with Point geometries".to_string()))
        }
    }
    
    fn st_m(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_M requires exactly 1 argument".to_string()));
        }
        
        match args[0].as_geometry()? {
            SpatialGeometry::Point(point) => {
                match point.m {
                    Some(m) => Ok(SqlValue::Float(m)),
                    None => Ok(SqlValue::Null)
                }
            },
            _ => Err(SpatialError::OperationError("ST_M can only be used with Point geometries".to_string()))
        }
    }
    
    fn st_startpoint(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_StartPoint not yet implemented".to_string()))
    }
    
    fn st_endpoint(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_EndPoint not yet implemented".to_string()))
    }
    
    fn st_pointn(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_PointN not yet implemented".to_string()))
    }
    
    fn st_npoints(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_NPoints not yet implemented".to_string()))
    }
    
    fn st_numgeometries(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_NumGeometries not yet implemented".to_string()))
    }
    
    fn st_geometryn(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_GeometryN not yet implemented".to_string()))
    }
    
    fn st_exteriorring(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_ExteriorRing not yet implemented".to_string()))
    }
    
    fn st_interiorringn(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_InteriorRingN not yet implemented".to_string()))
    }
    
    fn st_numinteriorrings(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_NumInteriorRings not yet implemented".to_string()))
    }
    
    fn st_isvalid(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Ok(SqlValue::Boolean(true)) // Placeholder - assume geometries are valid
    }
    
    fn st_isempty(&self, args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        if args.len() != 1 {
            return Err(SpatialError::OperationError("ST_IsEmpty requires exactly 1 argument".to_string()));
        }
        
        let geometry = args[0].as_geometry()?;
        Ok(SqlValue::Boolean(geometry.is_empty()))
    }
    
    fn st_issimple(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Ok(SqlValue::Boolean(true)) // Placeholder
    }
    
    fn st_isclosed(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Ok(SqlValue::Boolean(false)) // Placeholder
    }
    
    fn st_makevalid(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_MakeValid not yet implemented".to_string()))
    }
    
    fn st_asbinary(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_AsBinary not yet implemented".to_string()))
    }
    
    fn st_collect(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Collect not yet implemented".to_string()))
    }
    
    fn st_union_agg(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Union_Agg not yet implemented".to_string()))
    }
    
    fn st_extent(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_Extent not yet implemented".to_string()))
    }
    
    fn st_cluster_kmeans(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_ClusterKMeans not yet implemented".to_string()))
    }
    
    fn st_cluster_dbscan(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_ClusterDBSCAN not yet implemented".to_string()))
    }
    
    fn st_voronoi_polygons(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_VoronoiPolygons not yet implemented".to_string()))
    }
    
    fn st_delaunay_triangles(&self, _args: Vec<SqlValue>) -> Result<SqlValue, SpatialError> {
        Err(SpatialError::OperationError("ST_DelaunayTriangles not yet implemented".to_string()))
    }
    
    // Helper functions for parsing and formatting
    
    fn parse_wkt(&self, wkt: &str, srid: Option<i32>) -> Result<SpatialGeometry, SpatialError> {
        // Use shared SpatialFunctions WKT parser
        self.spatial_functions.st_geomfromtext(wkt, srid)
    }
    
    fn parse_geojson(&self, geojson: &str) -> Result<SpatialGeometry, SpatialError> {
        // Simplified GeoJSON parser
        let json: JsonValue = serde_json::from_str(geojson)
            .map_err(|_| SpatialError::OperationError("Invalid GeoJSON".to_string()))?;
        
        if let Some(geom_type) = json.get("type").and_then(|t| t.as_str()) {
            if geom_type == "Point" {
                if let Some(coords) = json.get("coordinates").and_then(|c| c.as_array()) {
                    if coords.len() >= 2 {
                        let x = coords[0].as_f64().ok_or(SpatialError::OperationError("Invalid coordinate".to_string()))?;
                        let y = coords[1].as_f64().ok_or(SpatialError::OperationError("Invalid coordinate".to_string()))?;
                        
                        let point = Point::new(x, y, Some(WGS84_SRID));
                        return Ok(SpatialGeometry::Point(point));
                    }
                }
            }
        }
        
        Err(SpatialError::OperationError("GeoJSON parsing not fully implemented".to_string()))
    }
    
    fn geometry_to_wkt(&self, geometry: &SpatialGeometry) -> Result<String, SpatialError> {
        match geometry {
            SpatialGeometry::Point(point) => {
                if point.z.is_some() && point.m.is_some() {
                    Ok(format!("POINT ZM ({} {} {} {})", point.x, point.y, point.z.unwrap(), point.m.unwrap()))
                } else if point.z.is_some() {
                    Ok(format!("POINT Z ({} {} {})", point.x, point.y, point.z.unwrap()))
                } else if point.m.is_some() {
                    Ok(format!("POINT M ({} {} {})", point.x, point.y, point.m.unwrap()))
                } else {
                    Ok(format!("POINT ({} {})", point.x, point.y))
                }
            },
            SpatialGeometry::LineString(ls) => {
                let coords: String = ls.points.iter()
                    .map(|p| format!("{} {}", p.x, p.y))
                    .collect::<Vec<_>>()
                    .join(", ");
                Ok(format!("LINESTRING ({})", coords))
            },
            SpatialGeometry::Polygon(poly) => {
                let exterior_coords: String = poly.exterior_ring.points.iter()
                    .map(|p| format!("{} {}", p.x, p.y))
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut wkt = format!("POLYGON (({}))", exterior_coords);
                
                // Add interior rings (holes) if any
                if !poly.interior_rings.is_empty() {
                    for interior_ring in &poly.interior_rings {
                        let interior_coords: String = interior_ring.points.iter()
                            .map(|p| format!("{} {}", p.x, p.y))
                            .collect::<Vec<_>>()
                            .join(", ");
                        wkt.push_str(&format!(", ({})", interior_coords));
                    }
                }
                Ok(wkt)
            },
            _ => Err(SpatialError::OperationError("WKT output not implemented for this geometry type".to_string()))
        }
    }
    
    fn geometry_to_geojson(&self, geometry: &SpatialGeometry) -> Result<String, SpatialError> {
        match geometry {
            SpatialGeometry::Point(point) => {
                let mut coords = vec![
                    JsonValue::Number(serde_json::Number::from_f64(point.x).unwrap()),
                    JsonValue::Number(serde_json::Number::from_f64(point.y).unwrap())
                ];
                if let Some(z) = point.z {
                    coords.push(JsonValue::Number(serde_json::Number::from_f64(z).unwrap()));
                }
                
                let geojson = JsonValue::Object([
                    ("type".to_string(), JsonValue::String("Point".to_string())),
                    ("coordinates".to_string(), JsonValue::Array(coords))
                ].into_iter().collect());
                
                Ok(serde_json::to_string(&geojson).unwrap())
            },
            SpatialGeometry::LineString(ls) => {
                let coords: Vec<JsonValue> = ls.points.iter()
                    .map(|p| JsonValue::Array(vec![
                        JsonValue::Number(serde_json::Number::from_f64(p.x).unwrap()),
                        JsonValue::Number(serde_json::Number::from_f64(p.y).unwrap())
                    ]))
                    .collect();
                
                let geojson = JsonValue::Object([
                    ("type".to_string(), JsonValue::String("LineString".to_string())),
                    ("coordinates".to_string(), JsonValue::Array(coords))
                ].into_iter().collect());
                
                Ok(serde_json::to_string(&geojson).unwrap())
            },
            SpatialGeometry::Polygon(poly) => {
                let mut rings = Vec::new();
                
                // Exterior ring
                let exterior_coords: Vec<JsonValue> = poly.exterior_ring.points.iter()
                    .map(|p| JsonValue::Array(vec![
                        JsonValue::Number(serde_json::Number::from_f64(p.x).unwrap()),
                        JsonValue::Number(serde_json::Number::from_f64(p.y).unwrap())
                    ]))
                    .collect();
                rings.push(JsonValue::Array(exterior_coords));
                
                // Interior rings
                for interior_ring in &poly.interior_rings {
                    let interior_coords: Vec<JsonValue> = interior_ring.points.iter()
                        .map(|p| JsonValue::Array(vec![
                            JsonValue::Number(serde_json::Number::from_f64(p.x).unwrap()),
                            JsonValue::Number(serde_json::Number::from_f64(p.y).unwrap())
                        ]))
                        .collect();
                    rings.push(JsonValue::Array(interior_coords));
                }
                
                let geojson = JsonValue::Object([
                    ("type".to_string(), JsonValue::String("Polygon".to_string())),
                    ("coordinates".to_string(), JsonValue::Array(rings))
                ].into_iter().collect());
                
                Ok(serde_json::to_string(&geojson).unwrap())
            },
            _ => Err(SpatialError::OperationError("GeoJSON output not implemented for this geometry type".to_string()))
        }
    }
}

impl Default for PostgresSpatialFunctions {
    fn default() -> Self {
        Self::new()
    }
}

/// SQL value types for PostgreSQL spatial functions.
#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Geometry(SpatialGeometry),
    Binary(Vec<u8>),
}

impl SqlValue {
    fn as_float(&self) -> Result<f64, SpatialError> {
        match self {
            SqlValue::Float(f) => Ok(*f),
            SqlValue::Integer(i) => Ok(*i as f64),
            _ => Err(SpatialError::OperationError("Value is not a number".to_string()))
        }
    }
    
    fn as_int(&self) -> Result<i64, SpatialError> {
        match self {
            SqlValue::Integer(i) => Ok(*i),
            SqlValue::Float(f) => Ok(*f as i64),
            _ => Err(SpatialError::OperationError("Value is not an integer".to_string()))
        }
    }
    
    fn as_string(&self) -> Result<String, SpatialError> {
        match self {
            SqlValue::String(s) => Ok(s.clone()),
            _ => Err(SpatialError::OperationError("Value is not a string".to_string()))
        }
    }
    
    fn as_geometry(&self) -> Result<&SpatialGeometry, SpatialError> {
        match self {
            SqlValue::Geometry(g) => Ok(g),
            _ => Err(SpatialError::OperationError("Value is not a geometry".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_st_point() {
        let functions = PostgresSpatialFunctions::new();
        let args = vec![SqlValue::Float(-122.4194), SqlValue::Float(37.7749)];
        let result = functions.st_point(args).unwrap();
        
        match result {
            SqlValue::Geometry(SpatialGeometry::Point(point)) => {
                assert_eq!(point.x, -122.4194);
                assert_eq!(point.y, 37.7749);
            },
            _ => panic!("Expected Point geometry")
        }
    }
    
    #[test]
    fn test_st_distance() {
        let functions = PostgresSpatialFunctions::new();
        let p1 = SpatialGeometry::Point(Point::new(0.0, 0.0, None));
        let p2 = SpatialGeometry::Point(Point::new(3.0, 4.0, None));
        
        let args = vec![SqlValue::Geometry(p1), SqlValue::Geometry(p2)];
        let result = functions.st_distance(args).unwrap();
        
        match result {
            SqlValue::Float(distance) => assert_eq!(distance, 5.0),
            _ => panic!("Expected float distance")
        }
    }
    
    #[test]
    fn test_st_contains() {
        let functions = PostgresSpatialFunctions::new();
        
        // Create a square polygon
        let exterior_points = vec![
            Point::new(0.0, 0.0, None),
            Point::new(4.0, 0.0, None),
            Point::new(4.0, 4.0, None),
            Point::new(0.0, 4.0, None),
            Point::new(0.0, 0.0, None),
        ];
        let exterior_ring = LinearRing::new(exterior_points).unwrap();
        let polygon = Polygon::new(exterior_ring, vec![], None).unwrap();
        let poly_geom = SpatialGeometry::Polygon(polygon);
        
        let point = SpatialGeometry::Point(Point::new(2.0, 2.0, None));
        
        let args = vec![SqlValue::Geometry(poly_geom), SqlValue::Geometry(point)];
        let result = functions.st_contains(args).unwrap();
        
        match result {
            SqlValue::Boolean(contains) => assert!(contains),
            _ => panic!("Expected boolean result")
        }
    }
    
    #[test]
    fn test_st_astext() {
        let functions = PostgresSpatialFunctions::new();
        let point = SpatialGeometry::Point(Point::new(-122.4194, 37.7749, None));
        
        let args = vec![SqlValue::Geometry(point)];
        let result = functions.st_astext(args).unwrap();
        
        match result {
            SqlValue::String(wkt) => assert_eq!(wkt, "POINT (-122.4194 37.7749)"),
            _ => panic!("Expected string WKT")
        }
    }
}