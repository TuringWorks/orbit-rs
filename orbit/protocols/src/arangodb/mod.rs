//! ArangoDB Protocol Adapter for Orbit-RS
//!
//! This module provides compatibility with ArangoDB's HTTP API and AQL query language,
//! with enhanced spatial capabilities through Orbit's spatial engine.

pub mod aql_spatial;
pub mod http;

pub use aql_spatial::{
    AQLSpatialExecutor, AQLSpatialParser, AQLSpatialResult, Document, SpatialCollection,
    SpatialIndex, SpatialIndexType,
};

pub use http::ArangoHttpProtocol;
