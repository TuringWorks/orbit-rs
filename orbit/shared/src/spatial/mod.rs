//! # Spatial Data Types and Functionality
//!
//! This module provides comprehensive geospatial support for Orbit-RS, including:
//! - OGC-compliant spatial geometry types
//! - Coordinate reference system support
//! - High-performance spatial indexing
//! - GPU-accelerated spatial operations
//! - Multi-protocol spatial query support
//!
//! ## Core Spatial Types
//!
//! - [`SpatialGeometry`]: Universal geometry type supporting all OGC simple features
//! - [`Point`]: 2D, 3D, and measured point geometries  
//! - [`LineString`]: Linear geometries with multiple points
//! - [`Polygon`]: Polygon geometries with holes support
//! - [`MultiPoint`], [`MultiLineString`], [`MultiPolygon`]: Multi-geometries
//! - [`GeometryCollection`]: Heterogeneous geometry collections
//!
//! ## Coordinate Reference Systems
//!
//! - Support for EPSG coordinate systems (WGS84, Web Mercator, UTM)
//! - Coordinate transformations between different projections
//! - Well-Known Text (WKT) and PROJ.4 string support
//!
//! ## High-Performance Indexing
//!
//! - [`SpatialIndex`]: Adaptive spatial indexing with multiple algorithms
//! - R-tree indexing for complex geometries and range queries
//! - QuadTree indexing for high-density point data
//! - Geohash grid indexing for global applications
//! - GPU-accelerated spatial grids for massive datasets
//!
//! ## Examples
//!
//! ```rust
//! use orbit_shared::spatial::{Point, SpatialGeometry, CoordinateReferenceSystem};
//!
//! // Create a point in WGS84 (GPS coordinates)
//! let san_francisco = Point::new(-122.4194, 37.7749, Some(4326));
//!
//! // Create a point with elevation
//! let mount_everest = Point::with_elevation(86.9250, 27.9881, 8848.86, Some(4326));
//!
//! // Convert to universal geometry type
//! let geometry = SpatialGeometry::Point(san_francisco);
//! ```

pub mod crs;
pub mod functions;
pub mod geometry;
pub mod gpu;
pub mod index;
pub mod operations;
pub mod streaming;

pub use crs::*;
pub use functions::*;
pub use geometry::*;
pub use gpu::*;
pub use index::*;
pub use operations::*;
pub use streaming::*;

// Common spatial constants
pub const WGS84_SRID: i32 = 4326; // World Geodetic System 1984
pub const WEB_MERCATOR_SRID: i32 = 3857; // Web Mercator (Google Maps)
pub const UTM_ZONE_33N_SRID: i32 = 32633; // UTM Zone 33N (Europe)

// Spatial precision constants
pub const DEFAULT_PRECISION: f64 = 1e-9; // ~1mm precision for coordinates
pub const EARTH_RADIUS_METERS: f64 = 6_378_137.0; // WGS84 equatorial radius
