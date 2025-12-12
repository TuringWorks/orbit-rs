// FTS module temporarily disabled - needs API update to work with SharedFtsEngine
// #[cfg(feature = "fts")]
// pub mod fts;
pub mod graph;
pub mod protocol;
pub mod server;
pub mod spatial;
pub mod storage;

pub use graph::{GraphLookupConfig, GraphLookupResult, MongoGraphEngine, NeighborDirection};
pub use server::MongoDbServer;
pub use spatial::{
    GeoJsonPoint, GeoJsonPolygon, GeoNearConfig, GeoNearResult, GeoWithinShape, MongoSpatialEngine,
    MongoSpatialError, MongoSpatialResult, NearConfig,
};
pub use storage::DocumentStore;
