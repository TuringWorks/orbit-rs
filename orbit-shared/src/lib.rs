pub mod actor_communication;
pub mod addressable;
pub mod cdc;
pub mod cluster_manager;
pub mod consensus;
pub mod data_sync;
pub mod election_metrics;
pub mod election_state;
pub mod event_sourcing;
pub mod exception;
pub mod failover;
pub mod graph;
pub mod graphrag;
pub mod integrated_recovery;
pub mod k8s_election;
pub mod mesh;
pub mod multi_master;
pub mod net;
pub mod orbitql;
pub mod persistence;
pub mod pooling;
pub mod raft_transport;
pub mod recovery;
pub mod replication;
pub mod rolling_upgrade;
pub mod router;
pub mod saga;
pub mod saga_recovery;
pub mod spatial;
pub mod stream_processing;
pub mod streaming_integrations;
pub mod timeseries;
pub mod transaction_log;
pub mod transactions;
pub mod transport;

pub use addressable::*;
pub use exception::*;
// Re-export specific graph types to avoid conflicts
pub use graph::{Direction, GraphNode, GraphRelationship, GraphStorage, InMemoryGraphStorage};
pub use graph::{NodeId as GraphNodeId, RelationshipId as GraphRelationshipId};
pub use mesh::*;
pub use net::*;
pub use router::*;

// Re-export CDC functionality
pub use cdc::{
    CdcConsumer, CdcCoordinator, CdcEvent, CdcFilter, CdcStats, CdcStream, CdcSubscription,
    DmlOperation,
};

// Re-export streaming integrations
pub use streaming_integrations::{
    KafkaCdcConsumer, KafkaConfig, KafkaStats, RabbitMqCdcConsumer, RabbitMqConfig, RabbitMqStats,
    WebhookCdcConsumer, WebhookConfig, WebhookStats,
};

// Re-export stream processing
pub use stream_processing::{
    AggregationFunction, StreamEvent, StreamProcessor, StreamStats, WindowResult, WindowState,
    WindowType,
};

// Re-export event sourcing
pub use event_sourcing::{
    DomainEvent, EventSourcedAggregate, EventStore, EventStoreConfig, EventStoreStats, Snapshot,
};

// Re-export replication
pub use replication::{
    ReplicationConfig, ReplicationSlot, ReplicationSlotManager, ReplicationStats,
    ReplicationStream,
};

// Re-export failover
pub use failover::{
    FailoverManager, FailoverPolicy, FailoverResult, FailoverStrategy, FailureDetector,
    FailureDetectionResult,
};

// Re-export multi-master
pub use multi_master::{
    Conflict, ConflictResolution, ConflictResolutionStrategy, MultiMasterCluster,
    MultiMasterConfig, WriteOperation,
};

// Re-export data sync
pub use data_sync::{
    DataSyncConfig, DataSyncCoordinator, SyncMode, SyncOperation, SyncStats, SyncStatus,
};

// Re-export rolling upgrade
pub use rolling_upgrade::{
    NodeUpgradePlan, RollingUpgradeConfig, RollingUpgradeManager, UpgradeOperation,
    UpgradeStatus, UpgradeStrategy, Version,
};

// Re-export advanced transaction features (excluding conflicting core module)
pub use transactions::{
    locks, metrics, performance, security, DistributedTransaction, TransactionCoordinator,
    TransactionId,
};

// Re-export time series functionality (excluding conflicting core module)
pub use timeseries::{
    aggregation, compression, partitioning, postgresql, query, redis, retention, storage,
    AggregationType, CompressionType, DataPoint, QueryResult, RetentionPolicy, SeriesId,
    StorageBackend, TimeRange, TimeSeriesConfig, TimeSeriesMetadata, TimeSeriesValue, Timestamp,
};

// Re-export OrbitQL functionality
pub use orbitql::{
    GeometryLiteral, OrbitQLEngine, QueryContext, QueryParams, QueryStats, QueryValue,
    SpatialFilter, SpatialFunctionCategory, SpatialFunctionRegistry, SpatialIndexConfig,
    SpatialIndexType, SpatialOperator, StreamTrigger, StreamingClause, WindowSpec,
    SPATIAL_FUNCTIONS,
};

// Re-export GraphRAG functionality
pub use graphrag::{
    ContextItem, ContextSourceType, EmbeddingModel, EmbeddingModelType, EnhancedGraphNode,
    EnhancedGraphRelationship, EntityType, ExtractedEntity, ExtractedRelationship, LLMProvider,
    RAGResponse, ReasoningPath, SearchStrategy,
};

// Re-export spatial functionality
pub use spatial::{
    AdaptiveSpatialIndexer, BoundingBox, CoordinateReferenceSystem, CoordinateTransformer,
    EPSGRegistry, GPUSpatialEngine, GeohashGrid, GeometryStatistics, IndexStatistics, LineString,
    LinearRing, MultiLineString, MultiPoint, MultiPolygon, Point, Polygon, QuadTree, RTree,
    SpatialAlert, SpatialError, SpatialFunctions, SpatialGeometry, SpatialIndex, SpatialOperations,
    SpatialRelation, SpatialStreamProcessor, DEFAULT_PRECISION, EARTH_RADIUS_METERS,
    UTM_ZONE_33N_SRID, WEB_MERCATOR_SRID, WGS84_SRID,
};

// Re-export advanced connection pooling
pub use pooling::{
    AdvancedConnectionPool, AdvancedPoolConfig, CircuitBreaker, CircuitBreakerConfig,
    CircuitBreakerState, ConnectionHealth, ConnectionHealthMonitor, ConnectionLoadBalancer,
    ConnectionPoolMetrics, HealthCheck, HealthStatus, LoadBalancingStrategy, NodeHealth, PoolTier,
};
