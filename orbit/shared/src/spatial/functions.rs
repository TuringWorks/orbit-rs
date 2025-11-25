//! SQL-style spatial functions for PostGIS compatibility.
//!
//! This module provides PostGIS-compatible spatial functions that can be
//! used across multiple query protocols. These functions serve as the foundation
//! for protocol-specific implementations (PostgreSQL, AQL, Cypher, etc.).

use super::{
    BoundingBox, CoordinateTransformer, LineString, LinearRing, Point, Polygon, SpatialError,
    SpatialGeometry, SpatialOperations,
};

/// PostGIS-compatible spatial functions.
pub struct SpatialFunctions {
    coordinate_transformer: CoordinateTransformer,
}

impl SpatialFunctions {
    /// Create a new spatial functions instance.
    pub fn new() -> Self {
        Self {
            coordinate_transformer: CoordinateTransformer::new(),
        }
    }

    // ============================================================================
    // Construction Functions
    // ============================================================================

    /// ST_Point - Create a point geometry.
    pub fn st_point(&self, x: f64, y: f64) -> SpatialGeometry {
        SpatialGeometry::Point(Point::new(x, y, None))
    }

    /// ST_MakePoint - Create a point with optional SRID.
    pub fn st_make_point(&self, x: f64, y: f64, srid: Option<i32>) -> SpatialGeometry {
        SpatialGeometry::Point(Point::new(x, y, srid))
    }

    /// ST_MakePoint - Create a 3D point with elevation.
    pub fn st_make_point_3d(&self, x: f64, y: f64, z: f64, srid: Option<i32>) -> SpatialGeometry {
        SpatialGeometry::Point(Point::with_elevation(x, y, z, srid))
    }

    /// ST_GeomFromText - Create geometry from Well-Known Text (WKT).
    pub fn st_geomfromtext(&self, wkt: &str, srid: Option<i32>) -> Result<SpatialGeometry, SpatialError> {
        // Simple WKT parser for common cases
        let wkt_upper = wkt.trim().to_uppercase();
        
        if wkt_upper.starts_with("POINT") {
            Self::parse_point_wkt(wkt, srid)
        } else if wkt_upper.starts_with("LINESTRING") {
            Self::parse_linestring_wkt(wkt, srid)
        } else if wkt_upper.starts_with("POLYGON") {
            Self::parse_polygon_wkt(wkt, srid)
        } else {
            Err(SpatialError::OperationError(
                format!("Unsupported WKT geometry type: {}", wkt_upper),
            ))
        }
    }

    /// Parse POINT WKT string.
    fn parse_point_wkt(wkt: &str, srid: Option<i32>) -> Result<SpatialGeometry, SpatialError> {
        // Extract coordinates from POINT(x y) or POINT(x y z)
        let coords: Vec<&str> = wkt
            .trim()
            .trim_start_matches("POINT")
            .trim_start_matches("(")
            .trim_end_matches(")")
            .split_whitespace()
            .collect();

        if coords.len() < 2 {
            return Err(SpatialError::OperationError("Invalid POINT WKT".to_string()));
        }

        let x: f64 = coords[0].parse().map_err(|_| {
            SpatialError::OperationError("Invalid X coordinate in POINT WKT".to_string())
        })?;
        let y: f64 = coords[1].parse().map_err(|_| {
            SpatialError::OperationError("Invalid Y coordinate in POINT WKT".to_string())
        })?;

        let point = if coords.len() >= 3 {
            let z: f64 = coords[2].parse().map_err(|_| {
                SpatialError::OperationError("Invalid Z coordinate in POINT WKT".to_string())
            })?;
            Point::with_elevation(x, y, z, srid)
        } else {
            Point::new(x, y, srid)
        };

        Ok(SpatialGeometry::Point(point))
    }

    /// Parse LINESTRING WKT string.
    fn parse_linestring_wkt(wkt: &str, srid: Option<i32>) -> Result<SpatialGeometry, SpatialError> {
        // Extract coordinates from LINESTRING(x1 y1, x2 y2, ...)
        let coords_str = wkt
            .trim()
            .trim_start_matches("LINESTRING")
            .trim_start_matches("(")
            .trim_end_matches(")");

        let mut points = Vec::new();
        for coord_pair in coords_str.split(',') {
            let coords: Vec<&str> = coord_pair.trim().split_whitespace().collect();
            if coords.len() >= 2 {
                let x: f64 = coords[0].parse().map_err(|_| {
                    SpatialError::OperationError("Invalid coordinate in LINESTRING WKT".to_string())
                })?;
                let y: f64 = coords[1].parse().map_err(|_| {
                    SpatialError::OperationError("Invalid coordinate in LINESTRING WKT".to_string())
                })?;
                points.push(Point::new(x, y, srid));
            }
        }

        if points.len() < 2 {
            return Err(SpatialError::OperationError(
                "LINESTRING must have at least 2 points".to_string(),
            ));
        }

        let linestring = LineString::new(points, srid)?;
        Ok(SpatialGeometry::LineString(linestring))
    }

    /// Parse POLYGON WKT string.
    fn parse_polygon_wkt(wkt: &str, srid: Option<i32>) -> Result<SpatialGeometry, SpatialError> {
        // Extract rings from POLYGON((x1 y1, x2 y2, ...), (hole1...), ...)
        let rings_str = wkt
            .trim()
            .trim_start_matches("POLYGON")
            .trim_start_matches("(")
            .trim_end_matches(")");

        // Simple parser - split by "),(" to get rings
        let ring_strs: Vec<&str> = rings_str
            .split("),(")
            .map(|s| s.trim().trim_start_matches('(').trim_end_matches(')'))
            .collect();

        if ring_strs.is_empty() {
            return Err(SpatialError::OperationError("POLYGON must have at least one ring".to_string()));
        }

        // Parse exterior ring
        let mut exterior_points = Vec::new();
        for coord_pair in ring_strs[0].split(',') {
            let coords: Vec<&str> = coord_pair.trim().split_whitespace().collect();
            if coords.len() >= 2 {
                let x: f64 = coords[0].parse().map_err(|_| {
                    SpatialError::OperationError("Invalid coordinate in POLYGON WKT".to_string())
                })?;
                let y: f64 = coords[1].parse().map_err(|_| {
                    SpatialError::OperationError("Invalid coordinate in POLYGON WKT".to_string())
                })?;
                exterior_points.push(Point::new(x, y, srid));
            }
        }

        let exterior_ring = LinearRing::new(exterior_points)?;

        // Parse interior rings (holes)
        let mut interior_rings = Vec::new();
        for ring_str in ring_strs.iter().skip(1) {
            let mut interior_points = Vec::new();
            for coord_pair in ring_str.split(',') {
                let coords: Vec<&str> = coord_pair.trim().split_whitespace().collect();
                if coords.len() >= 2 {
                    let x: f64 = coords[0].parse().map_err(|_| {
                        SpatialError::OperationError("Invalid coordinate in POLYGON WKT".to_string())
                    })?;
                    let y: f64 = coords[1].parse().map_err(|_| {
                        SpatialError::OperationError("Invalid coordinate in POLYGON WKT".to_string())
                    })?;
                    interior_points.push(Point::new(x, y, srid));
                }
            }
            interior_rings.push(LinearRing::new(interior_points)?);
        }

        let polygon = Polygon::new(exterior_ring, interior_rings, srid)?;
        Ok(SpatialGeometry::Polygon(polygon))
    }

    // ============================================================================
    // Measurement Functions
    // ============================================================================

    /// ST_Distance - Calculate distance between geometries.
    pub fn st_distance(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<f64, SpatialError> {
        SpatialOperations::distance(geom1, geom2)
    }

    /// ST_Distance_Sphere - Calculate great circle distance between two points (in meters).
    pub fn st_distance_sphere(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<f64, SpatialError> {
        match (geom1, geom2) {
            (SpatialGeometry::Point(p1), SpatialGeometry::Point(p2)) => {
                Ok(super::crs::utils::haversine_distance(p1, p2))
            }
            _ => Err(SpatialError::OperationError(
                "ST_Distance_Sphere only supports POINT geometries".to_string(),
            )),
        }
    }

    /// ST_Area - Calculate area of geometry.
    pub fn st_area(&self, geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        SpatialOperations::area(geometry)
    }

    /// ST_Length - Calculate length/perimeter of geometry.
    pub fn st_length(&self, geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        SpatialOperations::length(geometry)
    }

    /// ST_Perimeter - Calculate perimeter of polygon (alias for ST_Length).
    pub fn st_perimeter(&self, geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        SpatialOperations::length(geometry)
    }

    // ============================================================================
    // Spatial Relationship Functions
    // ============================================================================

    /// ST_Within - Test if geometry1 is within geometry2.
    pub fn st_within(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::within(geom1, geom2)
    }

    /// ST_Contains - Test if geometry1 contains geometry2.
    pub fn st_contains(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::contains(geom1, geom2)
    }

    /// ST_Intersects - Test if geometries intersect.
    pub fn st_intersects(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::intersects(geom1, geom2)
    }

    /// ST_Overlaps - Test if geometries overlap.
    pub fn st_overlaps(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::overlaps(geom1, geom2)
    }

    /// ST_Touches - Test if geometries touch.
    pub fn st_touches(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::touches(geom1, geom2)
    }

    /// ST_Crosses - Test if geometry1 crosses geometry2.
    pub fn st_crosses(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::crosses(geom1, geom2)
    }

    /// ST_Disjoint - Test if geometries are disjoint.
    pub fn st_disjoint(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::disjoint(geom1, geom2)
    }

    /// ST_Equals - Test if geometries are equal.
    pub fn st_equals(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
    ) -> Result<bool, SpatialError> {
        SpatialOperations::equals(geom1, geom2)
    }

    /// ST_DWithin - Test if geometries are within distance.
    pub fn st_dwithin(
        &self,
        geom1: &SpatialGeometry,
        geom2: &SpatialGeometry,
        distance: f64,
    ) -> Result<bool, SpatialError> {
        let dist = SpatialOperations::distance(geom1, geom2)?;
        Ok(dist <= distance)
    }

    // ============================================================================
    // Geometry Accessor Functions
    // ============================================================================

    /// ST_X - Get X coordinate of point.
    pub fn st_x(&self, geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        match geometry {
            SpatialGeometry::Point(p) => Ok(p.x),
            _ => Err(SpatialError::OperationError(
                "ST_X only works on POINT geometries".to_string(),
            )),
        }
    }

    /// ST_Y - Get Y coordinate of point.
    pub fn st_y(&self, geometry: &SpatialGeometry) -> Result<f64, SpatialError> {
        match geometry {
            SpatialGeometry::Point(p) => Ok(p.y),
            _ => Err(SpatialError::OperationError(
                "ST_Y only works on POINT geometries".to_string(),
            )),
        }
    }

    /// ST_Z - Get Z coordinate (elevation) of point.
    pub fn st_z(&self, geometry: &SpatialGeometry) -> Result<Option<f64>, SpatialError> {
        match geometry {
            SpatialGeometry::Point(p) => Ok(p.z),
            _ => Err(SpatialError::OperationError(
                "ST_Z only works on POINT geometries".to_string(),
            )),
        }
    }

    /// ST_SRID - Get SRID of geometry.
    pub fn st_srid(&self, geometry: &SpatialGeometry) -> Option<i32> {
        geometry.srid()
    }

    /// ST_SetSRID - Set SRID of geometry.
    pub fn st_setsrid(&self, geometry: &mut SpatialGeometry, srid: i32) {
        geometry.set_srid(Some(srid));
    }

    /// ST_Envelope - Get bounding box of geometry.
    pub fn st_envelope(&self, geometry: &SpatialGeometry) -> Result<BoundingBox, SpatialError> {
        SpatialOperations::bounding_box(geometry)
    }

    /// ST_IsEmpty - Test if geometry is empty.
    pub fn st_isempty(&self, geometry: &SpatialGeometry) -> bool {
        geometry.is_empty()
    }

    // ============================================================================
    // Coordinate Transformation Functions
    // ============================================================================

    /// ST_Transform - Transform geometry to different CRS.
    pub fn st_transform(
        &self,
        geometry: &SpatialGeometry,
        target_srid: i32,
    ) -> Result<SpatialGeometry, SpatialError> {
        match geometry {
            SpatialGeometry::Point(p) => {
                let transformed = self
                    .coordinate_transformer
                    .transform_point(p, target_srid)?;
                Ok(SpatialGeometry::Point(transformed))
            }
            _ => Err(SpatialError::OperationError(
                "ST_Transform not yet implemented for this geometry type".to_string(),
            )),
        }
    }
}

impl Default for SpatialFunctions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_st_point() {
        let funcs = SpatialFunctions::new();
        let point = funcs.st_point(1.0, 2.0);
        match point {
            SpatialGeometry::Point(p) => {
                assert_eq!(p.x, 1.0);
                assert_eq!(p.y, 2.0);
            }
            _ => panic!("Expected Point geometry"),
        }
    }

    #[test]
    fn test_st_make_point() {
        let funcs = SpatialFunctions::new();
        let point = funcs.st_make_point(1.0, 2.0, Some(4326));
        match point {
            SpatialGeometry::Point(p) => {
                assert_eq!(p.x, 1.0);
                assert_eq!(p.y, 2.0);
                assert_eq!(p.srid, Some(4326));
            }
            _ => panic!("Expected Point geometry"),
        }
    }

    #[test]
    fn test_st_geomfromtext_point() {
        let funcs = SpatialFunctions::new();
        let geom = funcs.st_geomfromtext("POINT(1 2)", Some(4326)).unwrap();
        match geom {
            SpatialGeometry::Point(p) => {
                assert_eq!(p.x, 1.0);
                assert_eq!(p.y, 2.0);
            }
            _ => panic!("Expected Point geometry"),
        }
    }

    #[test]
    fn test_st_geomfromtext_linestring() {
        let funcs = SpatialFunctions::new();
        let geom = funcs
            .st_geomfromtext("LINESTRING(0 0, 1 1, 2 2)", Some(4326))
            .unwrap();
        match geom {
            SpatialGeometry::LineString(ls) => {
                assert_eq!(ls.points.len(), 3);
            }
            _ => panic!("Expected LineString geometry"),
        }
    }

    #[test]
    fn test_st_distance() {
        let funcs = SpatialFunctions::new();
        let p1 = funcs.st_point(0.0, 0.0);
        let p2 = funcs.st_point(3.0, 4.0);
        let distance = funcs.st_distance(&p1, &p2).unwrap();
        assert_eq!(distance, 5.0);
    }

    #[test]
    fn test_st_within() {
        let funcs = SpatialFunctions::new();
        let inner = funcs.st_point(2.0, 2.0);
        let outer = funcs
            .st_geomfromtext("POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))", Some(4326))
            .unwrap();
        assert!(funcs.st_within(&inner, &outer).unwrap());
    }

    #[test]
    fn test_st_contains() {
        let funcs = SpatialFunctions::new();
        let outer = funcs
            .st_geomfromtext("POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))", Some(4326))
            .unwrap();
        let inner = funcs.st_point(2.0, 2.0);
        assert!(funcs.st_contains(&outer, &inner).unwrap());
    }

    #[test]
    fn test_st_dwithin() {
        let funcs = SpatialFunctions::new();
        let p1 = funcs.st_point(0.0, 0.0);
        let p2 = funcs.st_point(3.0, 4.0);
        assert!(funcs.st_dwithin(&p1, &p2, 6.0).unwrap());
        assert!(!funcs.st_dwithin(&p1, &p2, 4.0).unwrap());
    }

    #[test]
    fn test_st_x_y() {
        let funcs = SpatialFunctions::new();
        let point = funcs.st_point(1.5, 2.5);
        assert_eq!(funcs.st_x(&point).unwrap(), 1.5);
        assert_eq!(funcs.st_y(&point).unwrap(), 2.5);
    }

    #[test]
    fn test_st_srid() {
        let funcs = SpatialFunctions::new();
        let point = funcs.st_make_point(1.0, 2.0, Some(4326));
        assert_eq!(funcs.st_srid(&point), Some(4326));
    }
}
