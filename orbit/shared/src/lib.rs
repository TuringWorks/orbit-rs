pub mod actor_communication;
pub mod actor_memory;
pub mod addressable;
pub mod benchmarks;
pub mod builder_pattern;
pub mod cdc;
pub mod cluster_manager;
pub mod config;
pub mod config_utils;
pub mod consensus;
pub mod election_metrics;
pub mod election_state;
// Unified error module - use this for all error handling
pub mod error;
// Legacy modules - use error module instead for new code
pub mod error_handling;
pub mod error_utils;
pub mod exception;
pub mod event_sourcing;
pub mod execution_utils;
pub mod extensions;
pub mod graph;
pub mod graphrag;
pub mod integrated_recovery;
pub mod k8s_election;
pub mod mesh;
pub mod net;
pub mod orbitql;
pub mod persistence;
pub mod pooling;
pub mod raft_transport;
pub mod recovery;
pub mod replication;
pub mod router;
pub mod saga;
pub mod saga_recovery;
pub mod security;
pub mod security_patterns;
pub mod serialization;
pub mod spatial;
pub mod stream_processing;
pub mod streaming_integrations;
pub mod timeseries;
pub mod transaction_log;
pub mod transactions;
pub mod transport;
pub mod triggers;
pub mod types;
pub mod validation;

pub use addressable::{
    ActorWithInt32Key, ActorWithInt64Key, ActorWithNoKey, ActorWithStringKey, Addressable,
    AddressableInvocation, AddressableInvocationArgument, AddressableInvocationArguments,
    AddressableLease, AddressableReference, AddressableType, Key, NamespacedAddressableReference,
};
// Re-export from unified error module
pub use error::{
    ErrorContext, ErrorConverter, ErrorLog, OrbitError, OrbitResult, Result, SecurityValidator,
};
// Backward compatibility re-exports
#[allow(deprecated)]
pub use exception::{OrbitError as LegacyOrbitError, OrbitResult as LegacyOrbitResult};
// Re-export specific graph types to avoid conflicts
pub use graph::{Direction, GraphNode, GraphRelationship, GraphStorage, InMemoryGraphStorage};
pub use graph::{NodeId as GraphNodeId, RelationshipId as GraphRelationshipId};
pub use mesh::{Namespace, NodeCapabilities, NodeId, NodeInfo, NodeKey, NodeLease, NodeStatus};
pub use net::{InvocationReason, Message, MessageContent, MessageTarget};
pub use router::Route;

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
    ReplicationConfig, ReplicationSlot, ReplicationSlotManager, ReplicationStats, ReplicationStream,
};

// Re-export advanced transaction features (excluding conflicting core module)
pub use transactions::{
    locks, metrics, performance, DistributedTransaction, TransactionCoordinator, TransactionId,
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

// Re-export security functionality
pub use security::authentication::AuthToken;
pub use security::{
    AccessLevel, AnomalyDetector, AuditEvent, AuditLogger, AuditPolicy, AuthenticationManager,
    AuthenticationProvider, ComplianceMonitor, CrossTenantAction, CrossTenantPolicy,
    DataMaskingEngine, DatePrecision, EncryptedFieldValue, EncryptionManager, EncryptionType,
    FieldEncryptionConfig, FieldEncryptionEngine, FieldEncryptionPolicy, FieldMaskingConfig,
    IsolationLevel, KeyManagementSystem, KeyRotationPolicy, LdapAuthProvider, MaskingContext,
    MaskingPolicy, MaskingStrategy, OAuth2AuthProvider, PolicyEngine, QueryValidator, RbacEngine,
    ResourceQuota, ResourceUsage, RoleBasedAccessControl, SamlAuthProvider, SecurityAction,
    SecurityContext, SecurityFramework, SecurityResource, SecuritySubject, SensitivityLevel,
    SqlInjectionDetector, TenantAccessResult, TenantConfig, TenantContext, TenantId,
    TenantManager, TenantStatus, ThreatDetectionEngine, TlsConfig, TlsVersion, TokenStore,
};

// Re-export trigger functionality
pub use triggers::{
    TriggerContext, TriggerCoordinator, TriggerDefinition, TriggerEvent, TriggerExecutor,
    TriggerFunction, TriggerLevel, TriggerResult, TriggerStats, TriggerTiming,
};

// Re-export advanced connection pooling
pub use pooling::{
    AdvancedConnectionPool, AdvancedPoolConfig, CircuitBreaker, CircuitBreakerConfig,
    CircuitBreakerState, ConnectionHealth, ConnectionHealthMonitor, ConnectionLoadBalancer,
    ConnectionPoolMetrics, HealthCheck, HealthStatus, LoadBalancingStrategy, NodeHealth, PoolTier,
};
