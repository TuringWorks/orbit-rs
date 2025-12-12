//! Spatial functions for Neo4j Cypher queries
//!
//! Implements spatial functions like point(), distance(), and withinBBox()

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::neo4j::spatial_types::{Point, Point2D, Point3D, SpatialReferenceSystem};
use serde_json::Value;
use std::collections::HashMap;

/// Create a Point from coordinates
///
/// Supports both 2D and 3D points in Cartesian and Geographic coordinate systems.
///
/// # Arguments
/// * `params` - Map containing coordinates and optional SRID
///   - For 2D Cartesian: {x: number, y: number} or {x: number, y: number, srid: 7203}
///   - For 3D Cartesian: {x: number, y: number, z: number} or {x: number, y: number, z: number, srid: 9157}
///   - For 2D Geographic: {longitude: number, latitude: number} or {longitude: number, latitude: number, srid: 4326}
///   - For 3D Geographic: {longitude: number, latitude: number, height: number} or {longitude: number, latitude: number, height: number, srid: 4979}
pub fn point(params: &HashMap<String, Value>) -> ProtocolResult<Point> {
    // Check if SRID is explicitly provided
    let explicit_srid = params
        .get("srid")
        .and_then(|v| v.as_i64())
        .map(SpatialReferenceSystem::from_srid)
        .transpose()?;

    // Try to parse as geographic (longitude/latitude)
    if let (Some(lon), Some(lat)) = (
        params.get("longitude").and_then(|v| v.as_f64()),
        params.get("latitude").and_then(|v| v.as_f64()),
    ) {
        // Check for height (3D)
        if let Some(height) = params.get("height").and_then(|v| v.as_f64()) {
            let srid = explicit_srid.unwrap_or(SpatialReferenceSystem::WGS84_3D);
            if !srid.is_geographic() || !srid.is_3d() {
                return Err(ProtocolError::CypherError(
                    "SRID mismatch for 3D geographic point".to_string(),
                ));
            }
            Ok(Point::Point3D(Point3D {
                srid,
                x: lon,
                y: lat,
                z: height,
            }))
        } else {
            // 2D geographic
            let srid = explicit_srid.unwrap_or(SpatialReferenceSystem::WGS84_2D);
            if !srid.is_geographic() || !srid.is_2d() {
                return Err(ProtocolError::CypherError(
                    "SRID mismatch for 2D geographic point".to_string(),
                ));
            }
            Ok(Point::Point2D(Point2D {
                srid,
                x: lon,
                y: lat,
            }))
        }
    }
    // Try to parse as Cartesian (x/y/z)
    else if let (Some(x), Some(y)) = (
        params.get("x").and_then(|v| v.as_f64()),
        params.get("y").and_then(|v| v.as_f64()),
    ) {
        // Check for z (3D)
        if let Some(z) = params.get("z").and_then(|v| v.as_f64()) {
            let srid = explicit_srid.unwrap_or(SpatialReferenceSystem::Cartesian3D);
            if !srid.is_cartesian() || !srid.is_3d() {
                return Err(ProtocolError::CypherError(
                    "SRID mismatch for 3D Cartesian point".to_string(),
                ));
            }
            Ok(Point::Point3D(Point3D { srid, x, y, z }))
        } else {
            // 2D Cartesian
            let srid = explicit_srid.unwrap_or(SpatialReferenceSystem::Cartesian2D);
            if !srid.is_cartesian() || !srid.is_2d() {
                return Err(ProtocolError::CypherError(
                    "SRID mismatch for 2D Cartesian point".to_string(),
                ));
            }
            Ok(Point::Point2D(Point2D { srid, x, y }))
        }
    } else {
        Err(ProtocolError::CypherError(
            "Invalid point parameters: must provide either (x,y[,z]) or (longitude,latitude[,height])".to_string(),
        ))
    }
}

/// Calculate distance between two points
///
/// For geographic points (WGS84), uses Haversine formula for great-circle distance.
/// For Cartesian points, uses Euclidean distance.
///
/// # Arguments
/// * `point1` - First point
/// * `point2` - Second point
///
/// # Returns
/// Distance in meters for geographic points, or coordinate units for Cartesian points
pub fn distance(point1: &Point, point2: &Point) -> ProtocolResult<f64> {
    // Points must be in the same coordinate system
    if point1.srid() != point2.srid() {
        return Err(ProtocolError::CypherError(
            "Cannot calculate distance between points with different SRIDs".to_string(),
        ));
    }

    match (point1, point2) {
        (Point::Point2D(p1), Point::Point2D(p2)) => {
            if p1.srid.is_geographic() {
                Ok(haversine_distance(p1.x, p1.y, p2.x, p2.y))
            } else {
                Ok(euclidean_distance_2d(p1.x, p1.y, p2.x, p2.y))
            }
        }
        (Point::Point3D(p1), Point::Point3D(p2)) => {
            if p1.srid.is_geographic() {
                // For 3D geographic, use Haversine for horizontal distance
                // and add vertical distance
                let horizontal = haversine_distance(p1.x, p1.y, p2.x, p2.y);
                let vertical = (p1.z - p2.z).abs();
                Ok((horizontal.powi(2) + vertical.powi(2)).sqrt())
            } else {
                Ok(euclidean_distance_3d(p1.x, p1.y, p1.z, p2.x, p2.y, p2.z))
            }
        }
        _ => Err(ProtocolError::CypherError(
            "Cannot calculate distance between 2D and 3D points".to_string(),
        )),
    }
}

/// Check if a point is within a bounding box
///
/// # Arguments
/// * `point` - Point to check
/// * `lower_left` - Lower-left corner of bounding box
/// * `upper_right` - Upper-right corner of bounding box
pub fn within_bbox(point: &Point, lower_left: &Point, upper_right: &Point) -> ProtocolResult<bool> {
    // All points must be in the same coordinate system
    if point.srid() != lower_left.srid() || point.srid() != upper_right.srid() {
        return Err(ProtocolError::CypherError(
            "All points must have the same SRID for bounding box check".to_string(),
        ));
    }

    match (point, lower_left, upper_right) {
        (Point::Point2D(p), Point::Point2D(ll), Point::Point2D(ur)) => {
            Ok(p.x >= ll.x && p.x <= ur.x && p.y >= ll.y && p.y <= ur.y)
        }
        (Point::Point3D(p), Point::Point3D(ll), Point::Point3D(ur)) => Ok(p.x >= ll.x
            && p.x <= ur.x
            && p.y >= ll.y
            && p.y <= ur.y
            && p.z >= ll.z
            && p.z <= ur.z),
        _ => Err(ProtocolError::CypherError(
            "Bounding box points must have same dimensionality".to_string(),
        )),
    }
}

/// Calculate Haversine distance between two geographic points
///
/// Returns distance in meters
fn haversine_distance(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    const EARTH_RADIUS_METERS: f64 = 6371000.0; // Earth's radius in meters

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_METERS * c
}

/// Calculate Euclidean distance in 2D
fn euclidean_distance_2d(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}

/// Calculate Euclidean distance in 3D
fn euclidean_distance_3d(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> f64 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2) + (z2 - z1).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_creation_cartesian_2d() {
        let mut params = HashMap::new();
        params.insert("x".to_string(), Value::Number(10.into()));
        params.insert("y".to_string(), Value::Number(20.into()));

        let point = point(&params).unwrap();
        assert!(point.is_2d());
        assert_eq!(point.srid(), SpatialReferenceSystem::Cartesian2D);
    }

    #[test]
    fn test_point_creation_geographic_2d() {
        let mut params = HashMap::new();
        params.insert("longitude".to_string(), Value::from(-122.4194));
        params.insert("latitude".to_string(), Value::from(37.7749));

        let point = point(&params).unwrap();
        assert!(point.is_2d());
        assert_eq!(point.srid(), SpatialReferenceSystem::WGS84_2D);
    }

    #[test]
    fn test_point_creation_cartesian_3d() {
        let mut params = HashMap::new();
        params.insert("x".to_string(), Value::from(1.0));
        params.insert("y".to_string(), Value::from(2.0));
        params.insert("z".to_string(), Value::from(3.0));

        let point = point(&params).unwrap();
        assert!(point.is_3d());
        assert_eq!(point.srid(), SpatialReferenceSystem::Cartesian3D);
    }

    #[test]
    fn test_distance_cartesian_2d() {
        let p1 = Point::Point2D(Point2D::cartesian(0.0, 0.0));
        let p2 = Point::Point2D(Point2D::cartesian(3.0, 4.0));

        let dist = distance(&p1, &p2).unwrap();
        assert!((dist - 5.0).abs() < 0.001); // 3-4-5 triangle
    }

    #[test]
    fn test_distance_geographic_2d() {
        // San Francisco to Los Angeles (approx 559 km)
        let sf = Point::Point2D(Point2D::geographic(-122.4194, 37.7749));
        let la = Point::Point2D(Point2D::geographic(-118.2437, 34.0522));

        let dist = distance(&sf, &la).unwrap();
        assert!(dist > 500_000.0 && dist < 600_000.0); // Approximately 559 km
    }

    #[test]
    fn test_within_bbox_2d() {
        let point = Point::Point2D(Point2D::cartesian(5.0, 5.0));
        let lower_left = Point::Point2D(Point2D::cartesian(0.0, 0.0));
        let upper_right = Point::Point2D(Point2D::cartesian(10.0, 10.0));

        assert!(within_bbox(&point, &lower_left, &upper_right).unwrap());

        let outside = Point::Point2D(Point2D::cartesian(15.0, 5.0));
        assert!(!within_bbox(&outside, &lower_left, &upper_right).unwrap());
    }

    #[test]
    fn test_within_bbox_3d() {
        let point = Point::Point3D(Point3D::cartesian(5.0, 5.0, 5.0));
        let lower_left = Point::Point3D(Point3D::cartesian(0.0, 0.0, 0.0));
        let upper_right = Point::Point3D(Point3D::cartesian(10.0, 10.0, 10.0));

        assert!(within_bbox(&point, &lower_left, &upper_right).unwrap());
    }

    #[test]
    fn test_haversine_distance() {
        // Test known distance: London to Paris (approx 344 km)
        let london_lon = -0.1278;
        let london_lat = 51.5074;
        let paris_lon = 2.3522;
        let paris_lat = 48.8566;

        let dist = haversine_distance(london_lon, london_lat, paris_lon, paris_lat);
        assert!(dist > 300_000.0 && dist < 400_000.0); // Approximately 344 km
    }
}
