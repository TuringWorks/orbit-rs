//! Spatial type definitions and encoding for Neo4j Bolt protocol
//!
//! Supports Point types in both Cartesian and Geographic coordinate systems.

use crate::protocols::error::{ProtocolError, ProtocolResult};
use crate::protocols::neo4j::bolt_types::PackStreamValue;
use serde::{Deserialize, Serialize};

/// Spatial Reference System Identifier (SRID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialReferenceSystem {
    /// 2D Cartesian (SRID 7203)
    Cartesian2D = 7203,
    /// 3D Cartesian (SRID 9157)
    Cartesian3D = 9157,
    /// 2D Geographic WGS84 (SRID 4326)
    WGS84_2D = 4326,
    /// 3D Geographic WGS84 (SRID 4979)
    WGS84_3D = 4979,
}

impl SpatialReferenceSystem {
    /// Create from SRID value
    pub fn from_srid(srid: i64) -> ProtocolResult<Self> {
        match srid {
            7203 => Ok(Self::Cartesian2D),
            9157 => Ok(Self::Cartesian3D),
            4326 => Ok(Self::WGS84_2D),
            4979 => Ok(Self::WGS84_3D),
            _ => Err(ProtocolError::CypherError(format!(
                "Unknown SRID: {}",
                srid
            ))),
        }
    }

    /// Get SRID value
    pub fn srid(&self) -> i64 {
        *self as i64
    }

    /// Check if this is a 2D coordinate system
    pub fn is_2d(&self) -> bool {
        matches!(self, Self::Cartesian2D | Self::WGS84_2D)
    }

    /// Check if this is a 3D coordinate system
    pub fn is_3d(&self) -> bool {
        matches!(self, Self::Cartesian3D | Self::WGS84_3D)
    }

    /// Check if this is a Cartesian coordinate system
    pub fn is_cartesian(&self) -> bool {
        matches!(self, Self::Cartesian2D | Self::Cartesian3D)
    }

    /// Check if this is a Geographic (WGS84) coordinate system
    pub fn is_geographic(&self) -> bool {
        matches!(self, Self::WGS84_2D | Self::WGS84_3D)
    }
}

/// Point in 2D space (Cartesian or Geographic)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    /// Spatial reference system
    pub srid: SpatialReferenceSystem,
    /// X coordinate (or longitude for geographic)
    pub x: f64,
    /// Y coordinate (or latitude for geographic)
    pub y: f64,
}

impl Point2D {
    /// Create a new 2D Cartesian point
    pub fn cartesian(x: f64, y: f64) -> Self {
        Self {
            srid: SpatialReferenceSystem::Cartesian2D,
            x,
            y,
        }
    }

    /// Create a new 2D Geographic point (WGS84)
    pub fn geographic(longitude: f64, latitude: f64) -> Self {
        Self {
            srid: SpatialReferenceSystem::WGS84_2D,
            x: longitude,
            y: latitude,
        }
    }

    /// Get longitude (for geographic points)
    pub fn longitude(&self) -> Option<f64> {
        if self.srid.is_geographic() {
            Some(self.x)
        } else {
            None
        }
    }

    /// Get latitude (for geographic points)
    pub fn latitude(&self) -> Option<f64> {
        if self.srid.is_geographic() {
            Some(self.y)
        } else {
            None
        }
    }
}

/// Point in 3D space (Cartesian or Geographic)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    /// Spatial reference system
    pub srid: SpatialReferenceSystem,
    /// X coordinate (or longitude for geographic)
    pub x: f64,
    /// Y coordinate (or latitude for geographic)
    pub y: f64,
    /// Z coordinate (or height for geographic)
    pub z: f64,
}

impl Point3D {
    /// Create a new 3D Cartesian point
    pub fn cartesian(x: f64, y: f64, z: f64) -> Self {
        Self {
            srid: SpatialReferenceSystem::Cartesian3D,
            x,
            y,
            z,
        }
    }

    /// Create a new 3D Geographic point (WGS84)
    pub fn geographic(longitude: f64, latitude: f64, height: f64) -> Self {
        Self {
            srid: SpatialReferenceSystem::WGS84_3D,
            x: longitude,
            y: latitude,
            z: height,
        }
    }

    /// Get longitude (for geographic points)
    pub fn longitude(&self) -> Option<f64> {
        if self.srid.is_geographic() {
            Some(self.x)
        } else {
            None
        }
    }

    /// Get latitude (for geographic points)
    pub fn latitude(&self) -> Option<f64> {
        if self.srid.is_geographic() {
            Some(self.y)
        } else {
            None
        }
    }

    /// Get height (for geographic points)
    pub fn height(&self) -> Option<f64> {
        if self.srid.is_geographic() {
            Some(self.z)
        } else {
            None
        }
    }
}

/// Unified Point type that can be either 2D or 3D
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Point {
    /// 2D point
    Point2D(Point2D),
    /// 3D point
    Point3D(Point3D),
}

impl Point {
    /// Get the SRID
    pub fn srid(&self) -> SpatialReferenceSystem {
        match self {
            Point::Point2D(p) => p.srid,
            Point::Point3D(p) => p.srid,
        }
    }

    /// Check if this is a 2D point
    pub fn is_2d(&self) -> bool {
        matches!(self, Point::Point2D(_))
    }

    /// Check if this is a 3D point
    pub fn is_3d(&self) -> bool {
        matches!(self, Point::Point3D(_))
    }
}

/// Encode a 2D Point as a Bolt Point2D structure
///
/// Bolt Point2D structure (signature 0x58):
/// - Field 0: Integer (SRID)
/// - Field 1: Float (X coordinate)
/// - Field 2: Float (Y coordinate)
pub fn encode_point_2d(point: &Point2D) -> PackStreamValue {
    PackStreamValue::Structure {
        signature: 0x58,
        fields: vec![
            PackStreamValue::Integer(point.srid.srid()),
            PackStreamValue::Float(point.x),
            PackStreamValue::Float(point.y),
        ],
    }
}

/// Decode a Bolt Point2D structure
pub fn decode_point_2d(value: &PackStreamValue) -> ProtocolResult<Point2D> {
    if let PackStreamValue::Structure { signature, fields } = value {
        if *signature != 0x58 || fields.len() < 3 {
            return Err(ProtocolError::CypherError(
                "Invalid Point2D structure".to_string(),
            ));
        }

        let srid = if let PackStreamValue::Integer(i) = &fields[0] {
            SpatialReferenceSystem::from_srid(*i)?
        } else {
            return Err(ProtocolError::CypherError("Invalid SRID".to_string()));
        };

        if !srid.is_2d() {
            return Err(ProtocolError::CypherError(
                "SRID is not for 2D point".to_string(),
            ));
        }

        let x = if let PackStreamValue::Float(f) = &fields[1] {
            *f
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid X coordinate".to_string(),
            ));
        };

        let y = if let PackStreamValue::Float(f) = &fields[2] {
            *f
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid Y coordinate".to_string(),
            ));
        };

        Ok(Point2D { srid, x, y })
    } else {
        Err(ProtocolError::CypherError(
            "Expected Point2D structure".to_string(),
        ))
    }
}

/// Encode a 3D Point as a Bolt Point3D structure
///
/// Bolt Point3D structure (signature 0x59):
/// - Field 0: Integer (SRID)
/// - Field 1: Float (X coordinate)
/// - Field 2: Float (Y coordinate)
/// - Field 3: Float (Z coordinate)
pub fn encode_point_3d(point: &Point3D) -> PackStreamValue {
    PackStreamValue::Structure {
        signature: 0x59,
        fields: vec![
            PackStreamValue::Integer(point.srid.srid()),
            PackStreamValue::Float(point.x),
            PackStreamValue::Float(point.y),
            PackStreamValue::Float(point.z),
        ],
    }
}

/// Decode a Bolt Point3D structure
pub fn decode_point_3d(value: &PackStreamValue) -> ProtocolResult<Point3D> {
    if let PackStreamValue::Structure { signature, fields } = value {
        if *signature != 0x59 || fields.len() < 4 {
            return Err(ProtocolError::CypherError(
                "Invalid Point3D structure".to_string(),
            ));
        }

        let srid = if let PackStreamValue::Integer(i) = &fields[0] {
            SpatialReferenceSystem::from_srid(*i)?
        } else {
            return Err(ProtocolError::CypherError("Invalid SRID".to_string()));
        };

        if !srid.is_3d() {
            return Err(ProtocolError::CypherError(
                "SRID is not for 3D point".to_string(),
            ));
        }

        let x = if let PackStreamValue::Float(f) = &fields[1] {
            *f
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid X coordinate".to_string(),
            ));
        };

        let y = if let PackStreamValue::Float(f) = &fields[2] {
            *f
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid Y coordinate".to_string(),
            ));
        };

        let z = if let PackStreamValue::Float(f) = &fields[3] {
            *f
        } else {
            return Err(ProtocolError::CypherError(
                "Invalid Z coordinate".to_string(),
            ));
        };

        Ok(Point3D { srid, x, y, z })
    } else {
        Err(ProtocolError::CypherError(
            "Expected Point3D structure".to_string(),
        ))
    }
}

/// Encode a Point (either 2D or 3D)
pub fn encode_point(point: &Point) -> PackStreamValue {
    match point {
        Point::Point2D(p) => encode_point_2d(p),
        Point::Point3D(p) => encode_point_3d(p),
    }
}

/// Decode a Point structure (auto-detects 2D vs 3D based on signature)
pub fn decode_point(value: &PackStreamValue) -> ProtocolResult<Point> {
    if let PackStreamValue::Structure { signature, .. } = value {
        match *signature {
            0x58 => Ok(Point::Point2D(decode_point_2d(value)?)),
            0x59 => Ok(Point::Point3D(decode_point_3d(value)?)),
            _ => Err(ProtocolError::CypherError(format!(
                "Invalid Point signature: 0x{:02X}",
                signature
            ))),
        }
    } else {
        Err(ProtocolError::CypherError(
            "Expected Point structure".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_2d_cartesian() {
        let point = Point2D::cartesian(10.5, 20.3);
        assert_eq!(point.srid, SpatialReferenceSystem::Cartesian2D);
        assert_eq!(point.x, 10.5);
        assert_eq!(point.y, 20.3);
        assert!(point.longitude().is_none());
        assert!(point.latitude().is_none());
    }

    #[test]
    fn test_point_2d_geographic() {
        let point = Point2D::geographic(-122.4194, 37.7749); // San Francisco
        assert_eq!(point.srid, SpatialReferenceSystem::WGS84_2D);
        assert_eq!(point.longitude(), Some(-122.4194));
        assert_eq!(point.latitude(), Some(37.7749));
    }

    #[test]
    fn test_point_3d_cartesian() {
        let point = Point3D::cartesian(1.0, 2.0, 3.0);
        assert_eq!(point.srid, SpatialReferenceSystem::Cartesian3D);
        assert_eq!(point.x, 1.0);
        assert_eq!(point.y, 2.0);
        assert_eq!(point.z, 3.0);
    }

    #[test]
    fn test_point_3d_geographic() {
        let point = Point3D::geographic(-122.4194, 37.7749, 100.0);
        assert_eq!(point.srid, SpatialReferenceSystem::WGS84_3D);
        assert_eq!(point.longitude(), Some(-122.4194));
        assert_eq!(point.latitude(), Some(37.7749));
        assert_eq!(point.height(), Some(100.0));
    }

    #[test]
    fn test_encode_decode_point_2d_cartesian() {
        let point = Point2D::cartesian(10.5, 20.3);
        let encoded = encode_point_2d(&point);

        if let PackStreamValue::Structure { signature, fields } = &encoded {
            assert_eq!(*signature, 0x58);
            assert_eq!(fields.len(), 3);
            assert_eq!(fields[0], PackStreamValue::Integer(7203));
            assert_eq!(fields[1], PackStreamValue::Float(10.5));
            assert_eq!(fields[2], PackStreamValue::Float(20.3));
        } else {
            panic!("Expected Structure");
        }

        let decoded = decode_point_2d(&encoded).unwrap();
        assert_eq!(decoded, point);
    }

    #[test]
    fn test_encode_decode_point_2d_geographic() {
        let point = Point2D::geographic(-122.4194, 37.7749);
        let encoded = encode_point_2d(&point);

        if let PackStreamValue::Structure { signature, fields } = &encoded {
            assert_eq!(*signature, 0x58);
            assert_eq!(fields[0], PackStreamValue::Integer(4326));
        } else {
            panic!("Expected Structure");
        }

        let decoded = decode_point_2d(&encoded).unwrap();
        assert_eq!(decoded, point);
    }

    #[test]
    fn test_encode_decode_point_3d_cartesian() {
        let point = Point3D::cartesian(1.0, 2.0, 3.0);
        let encoded = encode_point_3d(&point);

        if let PackStreamValue::Structure { signature, fields } = &encoded {
            assert_eq!(*signature, 0x59);
            assert_eq!(fields.len(), 4);
            assert_eq!(fields[0], PackStreamValue::Integer(9157));
        } else {
            panic!("Expected Structure");
        }

        let decoded = decode_point_3d(&encoded).unwrap();
        assert_eq!(decoded, point);
    }

    #[test]
    fn test_encode_decode_point_3d_geographic() {
        let point = Point3D::geographic(-122.4194, 37.7749, 100.0);
        let encoded = encode_point_3d(&point);

        if let PackStreamValue::Structure { signature, fields } = &encoded {
            assert_eq!(*signature, 0x59);
            assert_eq!(fields[0], PackStreamValue::Integer(4979));
        } else {
            panic!("Expected Structure");
        }

        let decoded = decode_point_3d(&encoded).unwrap();
        assert_eq!(decoded, point);
    }

    #[test]
    fn test_unified_point_encoding() {
        let point_2d = Point::Point2D(Point2D::cartesian(10.0, 20.0));
        let encoded_2d = encode_point(&point_2d);
        let decoded_2d = decode_point(&encoded_2d).unwrap();
        assert_eq!(decoded_2d, point_2d);

        let point_3d = Point::Point3D(Point3D::cartesian(1.0, 2.0, 3.0));
        let encoded_3d = encode_point(&point_3d);
        let decoded_3d = decode_point(&encoded_3d).unwrap();
        assert_eq!(decoded_3d, point_3d);
    }

    #[test]
    fn test_srid_validation() {
        assert!(SpatialReferenceSystem::from_srid(7203).is_ok());
        assert!(SpatialReferenceSystem::from_srid(9157).is_ok());
        assert!(SpatialReferenceSystem::from_srid(4326).is_ok());
        assert!(SpatialReferenceSystem::from_srid(4979).is_ok());
        assert!(SpatialReferenceSystem::from_srid(9999).is_err());
    }
}
