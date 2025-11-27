//! ArangoDB Protocol Server
//!
//! Handles ArangoDB HTTP protocol connections.

#[cfg(feature = "protocol-arangodb")]
pub mod http_server;
#[cfg(feature = "protocol-arangodb")]
pub use http_server::ArangoServer;

pub mod aql_spatial;

pub use aql_spatial::{
    AQLSpatialExecutor, AQLSpatialParser, AQLSpatialResult, Document, SpatialCollection,
    SpatialIndex, SpatialIndexType,
};
