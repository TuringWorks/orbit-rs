//! Coordinate Reference System (CRS) support for spatial data.
//!
//! This module provides comprehensive coordinate reference system support,
//! including EPSG code handling, coordinate transformations, and common
//! projections like WGS84, Web Mercator, and UTM zones.

use super::{Point, SpatialError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::PI;

/// Coordinate Reference System definition with EPSG support.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoordinateReferenceSystem {
    /// EPSG SRID code (e.g., 4326 for WGS84)
    pub srid: i32,
    /// Authority name (typically "EPSG")
    pub authority: String,
    /// Authority-specific code
    pub code: i32,
    /// PROJ.4 projection string
    pub proj4_string: String,
    /// Well-Known Text (WKT) definition
    pub wkt: String,
    /// Human-readable name
    pub name: String,
    /// Units (e.g., "degree", "meter")
    pub units: String,
    /// Whether this is a geographic (lat/lon) or projected coordinate system
    pub is_geographic: bool,
}

/// Common EPSG coordinate systems with their definitions.
pub struct EPSGRegistry {
    systems: HashMap<i32, CoordinateReferenceSystem>,
}

/// Coordinate transformation parameters between different CRS.
#[derive(Debug, Clone)]
pub struct TransformationParameters {
    pub from_srid: i32,
    pub to_srid: i32,
    pub dx: f64,    // Translation in X
    pub dy: f64,    // Translation in Y
    pub dz: f64,    // Translation in Z
    pub rx: f64,    // Rotation around X axis (radians)
    pub ry: f64,    // Rotation around Y axis (radians)
    pub rz: f64,    // Rotation around Z axis (radians)
    pub scale: f64, // Scale factor
}

impl CoordinateReferenceSystem {
    /// Create a new CRS definition.
    pub fn new(
        srid: i32,
        authority: String,
        code: i32,
        proj4_string: String,
        wkt: String,
        name: String,
        units: String,
        is_geographic: bool,
    ) -> Self {
        Self {
            srid,
            authority,
            code,
            proj4_string,
            wkt,
            name,
            units,
            is_geographic,
        }
    }

    /// Get the axis order for this CRS (important for some coordinate systems).
    pub fn axis_order(&self) -> AxisOrder {
        match self.srid {
            4326 => AxisOrder::LonLat, // WGS84
            3857 => AxisOrder::XY,     // Web Mercator
            _ => AxisOrder::XY,        // Default to XY order
        }
    }
}

/// Axis order enumeration for coordinate systems.
#[derive(Debug, Clone, PartialEq)]
pub enum AxisOrder {
    /// X, Y order (easting, northing)
    XY,
    /// Longitude, Latitude order
    LonLat,
    /// Latitude, Longitude order (some EPSG definitions)
    LatLon,
}

impl EPSGRegistry {
    /// Create a new EPSG registry with common coordinate systems.
    pub fn new() -> Self {
        let mut systems = HashMap::new();

        // WGS84 Geographic
        systems.insert(4326, CoordinateReferenceSystem::new(
            4326,
            "EPSG".to_string(),
            4326,
            "+proj=longlat +datum=WGS84 +no_defs".to_string(),
            r#"GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563,AUTHORITY["EPSG","7030"]],AUTHORITY["EPSG","6326"]],PRIMEM["Greenwich",0,AUTHORITY["EPSG","8901"]],UNIT["degree",0.0174532925199433,AUTHORITY["EPSG","9122"]],AUTHORITY["EPSG","4326"]]"#.to_string(),
            "WGS 84".to_string(),
            "degree".to_string(),
            true,
        ));

        // Web Mercator (Google Maps, OpenStreetMap)
        systems.insert(3857, CoordinateReferenceSystem::new(
            3857,
            "EPSG".to_string(),
            3857,
            "+proj=merc +a=6378137 +b=6378137 +lat_ts=0.0 +lon_0=0.0 +x_0=0.0 +y_0=0 +k=1.0 +units=m +nadgrids=@null +wktext +no_defs".to_string(),
            r#"PROJCS["WGS 84 / Pseudo-Mercator",GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563,AUTHORITY["EPSG","7030"]],AUTHORITY["EPSG","6326"]],PRIMEM["Greenwich",0,AUTHORITY["EPSG","8901"]],UNIT["degree",0.0174532925199433,AUTHORITY["EPSG","9122"]],AUTHORITY["EPSG","4326"]],PROJECTION["Mercator_1SP"],PARAMETER["central_meridian",0],PARAMETER["scale_factor",1],PARAMETER["false_easting",0],PARAMETER["false_northing",0],UNIT["metre",1,AUTHORITY["EPSG","9001"]],AXIS["X",EAST],AXIS["Y",NORTH],EXTENSION["PROJ4","+proj=merc +a=6378137 +b=6378137 +lat_ts=0.0 +lon_0=0.0 +x_0=0.0 +y_0=0 +k=1.0 +units=m +nadgrids=@null +wktext +no_defs"],AUTHORITY["EPSG","3857"]]"#.to_string(),
            "WGS 84 / Pseudo-Mercator".to_string(),
            "meter".to_string(),
            false,
        ));

        // UTM Zone 33N (Europe)
        systems.insert(32633, CoordinateReferenceSystem::new(
            32633,
            "EPSG".to_string(),
            32633,
            "+proj=utm +zone=33 +datum=WGS84 +units=m +no_defs".to_string(),
            r#"PROJCS["WGS 84 / UTM zone 33N",GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563,AUTHORITY["EPSG","7030"]],AUTHORITY["EPSG","6326"]],PRIMEM["Greenwich",0,AUTHORITY["EPSG","8901"]],UNIT["degree",0.0174532925199433,AUTHORITY["EPSG","9122"]],AUTHORITY["EPSG","4326"]],PROJECTION["Transverse_Mercator"],PARAMETER["latitude_of_origin",0],PARAMETER["central_meridian",15],PARAMETER["scale_factor",0.9996],PARAMETER["false_easting",500000],PARAMETER["false_northing",0],UNIT["metre",1,AUTHORITY["EPSG","9001"]],AXIS["Easting",EAST],AXIS["Northing",NORTH],AUTHORITY["EPSG","32633"]]"#.to_string(),
            "WGS 84 / UTM zone 33N".to_string(),
            "meter".to_string(),
            false,
        ));

        Self { systems }
    }

    /// Get a coordinate reference system by SRID.
    pub fn get(&self, srid: i32) -> Option<&CoordinateReferenceSystem> {
        self.systems.get(&srid)
    }

    /// Add a new CRS definition to the registry.
    pub fn add(&mut self, crs: CoordinateReferenceSystem) {
        self.systems.insert(crs.srid, crs);
    }

    /// List all available SRIDs in the registry.
    pub fn available_srids(&self) -> Vec<i32> {
        let mut srids: Vec<i32> = self.systems.keys().cloned().collect();
        srids.sort();
        srids
    }
}

impl Default for EPSGRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Coordinate transformation engine for converting between different CRS.
pub struct CoordinateTransformer {
    registry: EPSGRegistry,
    transformation_cache: HashMap<(i32, i32), TransformationParameters>,
}

impl CoordinateTransformer {
    /// Create a new coordinate transformer.
    pub fn new() -> Self {
        Self {
            registry: EPSGRegistry::new(),
            transformation_cache: HashMap::new(),
        }
    }

    /// Transform a point from one CRS to another.
    pub fn transform_point(&self, point: &Point, target_srid: i32) -> Result<Point, SpatialError> {
        let source_srid = point.srid.unwrap_or(4326); // Default to WGS84

        if source_srid == target_srid {
            return Ok(point.clone());
        }

        let _source_crs = self.registry.get(source_srid).ok_or_else(|| {
            SpatialError::CRSError(format!("Unknown source SRID: {}", source_srid))
        })?;

        let _target_crs = self.registry.get(target_srid).ok_or_else(|| {
            SpatialError::CRSError(format!("Unknown target SRID: {}", target_srid))
        })?;

        // Handle common transformations directly
        match (source_srid, target_srid) {
            (4326, 3857) => self.wgs84_to_web_mercator(point),
            (3857, 4326) => self.web_mercator_to_wgs84(point),
            (4326, utm_zone) if (32601..=32660).contains(&utm_zone) => {
                self.wgs84_to_utm_north(point, utm_zone)
            }
            (utm_zone, 4326) if (32601..=32660).contains(&utm_zone) => {
                self.utm_north_to_wgs84(point, utm_zone)
            }
            _ => Err(SpatialError::CRSError(format!(
                "Transformation from SRID {} to {} not implemented",
                source_srid, target_srid
            ))),
        }
    }

    /// Transform WGS84 coordinates to Web Mercator.
    fn wgs84_to_web_mercator(&self, point: &Point) -> Result<Point, SpatialError> {
        let lon_rad = point.x.to_radians();
        let lat_rad = point.y.to_radians();

        // Web Mercator transformation
        let x = 6378137.0 * lon_rad;
        let y = 6378137.0 * ((PI / 4.0) + (lat_rad / 2.0)).tan().ln();

        Ok(Point {
            x,
            y,
            z: point.z,
            m: point.m,
            srid: Some(3857),
        })
    }

    /// Transform Web Mercator coordinates to WGS84.
    fn web_mercator_to_wgs84(&self, point: &Point) -> Result<Point, SpatialError> {
        let x = point.x / 6378137.0;
        let y = point.y / 6378137.0;

        let lon = x.to_degrees();
        let lat = (2.0 * (y.exp().atan()) - PI / 2.0).to_degrees();

        Ok(Point {
            x: lon,
            y: lat,
            z: point.z,
            m: point.m,
            srid: Some(4326),
        })
    }

    /// Transform WGS84 coordinates to UTM North.
    fn wgs84_to_utm_north(&self, point: &Point, utm_srid: i32) -> Result<Point, SpatialError> {
        let zone = utm_srid - 32600; // Extract UTM zone number

        if !(1..=60).contains(&zone) {
            return Err(SpatialError::CRSError(format!(
                "Invalid UTM zone: {}",
                zone
            )));
        }

        let lon_rad = point.x.to_radians();
        let lat_rad = point.y.to_radians();

        // UTM parameters
        let k0 = 0.9996; // Scale factor
        let e = 0.08181919084262; // Eccentricity of WGS84 ellipsoid
        let e_prime = 0.08209443794970; // Second eccentricity
        let a = 6378137.0; // Semi-major axis of WGS84

        let central_meridian = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
        let lon_origin = lon_rad - central_meridian;

        // UTM transformation formulas (simplified)
        let n = a / (1.0 - e * e * lat_rad.sin() * lat_rad.sin()).sqrt();
        let t = lat_rad.tan();
        let c = e_prime * e_prime * lat_rad.cos() * lat_rad.cos();
        let a_coeff = lat_rad.cos() * lon_origin;

        let m = a
            * ((1.0 - e * e / 4.0 - 3.0 * e * e * e * e / 64.0) * lat_rad
                - (3.0 * e * e / 8.0 + 3.0 * e * e * e * e / 32.0) * (2.0 * lat_rad).sin()
                + (15.0 * e * e * e * e / 256.0) * (4.0 * lat_rad).sin());

        let x =
            k0 * n * (a_coeff + (1.0 - t * t + c) * a_coeff * a_coeff * a_coeff / 6.0) + 500000.0;
        let y = k0
            * (m + n
                * t
                * (a_coeff * a_coeff / 2.0
                    + (5.0 - t * t + 9.0 * c + 4.0 * c * c)
                        * a_coeff
                        * a_coeff
                        * a_coeff
                        * a_coeff
                        / 24.0));

        Ok(Point {
            x,
            y,
            z: point.z,
            m: point.m,
            srid: Some(utm_srid),
        })
    }

    /// Transform UTM North coordinates to WGS84.
    fn utm_north_to_wgs84(&self, point: &Point, utm_srid: i32) -> Result<Point, SpatialError> {
        let zone = utm_srid - 32600;

        if !(1..=60).contains(&zone) {
            return Err(SpatialError::CRSError(format!(
                "Invalid UTM zone: {}",
                zone
            )));
        }

        // UTM parameters
        let k0 = 0.9996;
        let e = 0.08181919084262;
        let e1 = (1.0 - (1.0_f64 - e * e).sqrt()) / (1.0 + (1.0_f64 - e * e).sqrt());
        let a = 6378137.0;

        let x = point.x - 500000.0;
        let y = point.y;

        let central_meridian = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();

        let m = y / k0;
        let mu = m / (a * (1.0 - e * e / 4.0 - 3.0 * e * e * e * e / 64.0));

        let lat_rad = mu
            + (3.0 * e1 / 2.0 - 27.0 * e1 * e1 * e1 / 32.0) * (2.0 * mu).sin()
            + (21.0 * e1 * e1 / 16.0 - 55.0 * e1 * e1 * e1 * e1 / 32.0) * (4.0 * mu).sin()
            + (151.0 * e1 * e1 * e1 / 96.0) * (6.0 * mu).sin();

        let lat = lat_rad.to_degrees();
        let lon = (central_meridian
            + x / (k0 * a / (1.0 - e * e * lat_rad.sin() * lat_rad.sin()).sqrt()) / lat_rad.cos())
        .to_degrees();

        Ok(Point {
            x: lon,
            y: lat,
            z: point.z,
            m: point.m,
            srid: Some(4326),
        })
    }

    /// Get the registry for direct access.
    pub fn registry(&self) -> &EPSGRegistry {
        &self.registry
    }

    /// Add a custom CRS to the registry.
    pub fn add_crs(&mut self, crs: CoordinateReferenceSystem) {
        self.registry.add(crs);
    }
}

impl Default for CoordinateTransformer {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for common coordinate operations.
pub mod utils {
    use super::*;

    /// Convert degrees to radians.
    pub fn deg_to_rad(degrees: f64) -> f64 {
        degrees * PI / 180.0
    }

    /// Convert radians to degrees.
    pub fn rad_to_deg(radians: f64) -> f64 {
        radians * 180.0 / PI
    }

    /// Calculate the great circle distance between two WGS84 points using the Haversine formula.
    pub fn haversine_distance(p1: &Point, p2: &Point) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;

        let lat1_rad = deg_to_rad(p1.y);
        let lat2_rad = deg_to_rad(p2.y);
        let delta_lat = deg_to_rad(p2.y - p1.y);
        let delta_lon = deg_to_rad(p2.x - p1.x);

        let a = (delta_lat / 2.0).sin().powi(2)
            + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        EARTH_RADIUS_KM * c * 1000.0 // Return distance in meters
    }

    /// Calculate the bearing (azimuth) from one WGS84 point to another.
    pub fn bearing(p1: &Point, p2: &Point) -> f64 {
        let lat1_rad = deg_to_rad(p1.y);
        let lat2_rad = deg_to_rad(p2.y);
        let delta_lon = deg_to_rad(p2.x - p1.x);

        let y = delta_lon.sin() * lat2_rad.cos();
        let x = lat1_rad.cos() * lat2_rad.sin() - lat1_rad.sin() * lat2_rad.cos() * delta_lon.cos();

        let bearing_rad = y.atan2(x);
        let bearing_deg = rad_to_deg(bearing_rad);

        (bearing_deg + 360.0) % 360.0 // Normalize to 0-360 degrees
    }

    /// Project a WGS84 point to a destination point given distance and bearing.
    pub fn project_point(point: &Point, distance_meters: f64, bearing_degrees: f64) -> Point {
        const EARTH_RADIUS_M: f64 = 6378137.0;

        let lat1_rad = deg_to_rad(point.y);
        let lon1_rad = deg_to_rad(point.x);
        let bearing_rad = deg_to_rad(bearing_degrees);
        let angular_distance = distance_meters / EARTH_RADIUS_M;

        let lat2_rad = (lat1_rad.sin() * angular_distance.cos()
            + lat1_rad.cos() * angular_distance.sin() * bearing_rad.cos())
        .asin();

        let lon2_rad = lon1_rad
            + (bearing_rad.sin() * angular_distance.sin() * lat1_rad.cos())
                .atan2(angular_distance.cos() - lat1_rad.sin() * lat2_rad.sin());

        Point::new(rad_to_deg(lon2_rad), rad_to_deg(lat2_rad), point.srid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use crate::spatial::WGS84_SRID;

    #[test]
    fn test_epsg_registry() {
        let registry = EPSGRegistry::new();

        // Test WGS84
        let wgs84 = registry.get(4326).unwrap();
        assert_eq!(wgs84.name, "WGS 84");
        assert_eq!(wgs84.units, "degree");
        assert!(wgs84.is_geographic);

        // Test Web Mercator
        let web_mercator = registry.get(3857).unwrap();
        assert_eq!(web_mercator.name, "WGS 84 / Pseudo-Mercator");
        assert_eq!(web_mercator.units, "meter");
        assert!(!web_mercator.is_geographic);
    }

    #[test]
    fn test_coordinate_transformation() {
        let transformer = CoordinateTransformer::new();

        // Test WGS84 to Web Mercator transformation
        let wgs84_point = Point::new(0.0, 0.0, Some(4326)); // Equator/Prime Meridian
        let web_mercator_point = transformer.transform_point(&wgs84_point, 3857).unwrap();

        assert!((web_mercator_point.x - 0.0).abs() < 1e-6);
        assert!((web_mercator_point.y - 0.0).abs() < 1e-6);
        assert_eq!(web_mercator_point.srid, Some(3857));

        // Test round-trip transformation
        let back_to_wgs84 = transformer
            .transform_point(&web_mercator_point, 4326)
            .unwrap();
        assert!((back_to_wgs84.x - wgs84_point.x).abs() < 1e-10);
        assert!((back_to_wgs84.y - wgs84_point.y).abs() < 1e-10);
    }

    #[test]
    fn test_haversine_distance() {
        // Distance between New York and Los Angeles
        let ny = Point::new(-74.0060, 40.7128, Some(4326));
        let la = Point::new(-118.2437, 34.0522, Some(4326));

        let distance = utils::haversine_distance(&ny, &la);

        // Should be approximately 3,944 km
        assert!((distance / 1000.0 - 3944.0).abs() < 50.0);
    }

    #[test]
    fn test_bearing_calculation() {
        let p1 = Point::new(0.0, 0.0, Some(4326));
        let p2 = Point::new(1.0, 1.0, Some(4326));

        let bearing = utils::bearing(&p1, &p2);

        // Should be northeast direction (around 45 degrees)
        assert!((bearing - 45.0).abs() < 5.0);
    }

    #[test]
    fn test_point_projection() {
        let origin = Point::new(0.0, 0.0, Some(4326));
        let projected = utils::project_point(&origin, 111320.0, 90.0); // 1 degree east

        // Should be approximately 1 degree east
        assert!((projected.x - 1.0).abs() < 0.01);
        assert!(projected.y.abs() < 0.01);
    }
}
