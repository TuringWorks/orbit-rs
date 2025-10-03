use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// OrbitActor represents a managed actor deployment within an Orbit cluster
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "orbit.turingworks.com",
    version = "v1",
    kind = "OrbitActor",
    plural = "orbitactors",
    namespaced
)]
#[kube(status = "OrbitActorStatus")]
#[kube(shortname = "oa")]
#[allow(dead_code)]
pub struct OrbitActorSpec {
    /// Actor type identifier
    pub actor_type: String,

    /// Target cluster to deploy to
    pub cluster_ref: ClusterReference,

    /// Actor configuration
    #[serde(default)]
    pub config: ActorConfig,

    /// Scaling configuration
    #[serde(default)]
    pub scaling: ScalingConfig,

    /// Actor-specific environment variables
    #[serde(default)]
    pub env: BTreeMap<String, String>,

    /// Custom actor properties
    #[serde(default)]
    pub properties: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[allow(dead_code)]
pub struct ClusterReference {
    /// Name of the OrbitCluster
    pub name: String,

    /// Namespace of the OrbitCluster (optional, defaults to same namespace)
    pub namespace: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[allow(dead_code)]
pub struct ActorConfig {
    /// Actor activation timeout in seconds
    #[serde(default = "default_activation_timeout")]
    pub activation_timeout: u32,

    /// Actor deactivation timeout in seconds
    #[serde(default = "default_deactivation_timeout")]
    pub deactivation_timeout: u32,

    /// Actor idle timeout in seconds (0 = never deactivate)
    #[serde(default)]
    pub idle_timeout: u32,

    /// Maximum concurrent actors per node
    #[serde(default = "default_max_actors_per_node")]
    pub max_actors_per_node: u32,

    /// Actor persistence enabled
    #[serde(default)]
    pub persistence_enabled: bool,

    /// Actor state serialization format
    #[serde(default = "default_serialization_format")]
    pub serialization_format: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[allow(dead_code)]
pub struct ScalingConfig {
    /// Minimum number of actor instances
    #[serde(default = "default_min_instances")]
    pub min_instances: u32,

    /// Maximum number of actor instances
    #[serde(default = "default_max_instances")]
    pub max_instances: u32,

    /// Target CPU utilization percentage for scaling
    #[serde(default = "default_target_cpu")]
    pub target_cpu_utilization: u32,

    /// Target memory utilization percentage for scaling
    #[serde(default = "default_target_memory")]
    pub target_memory_utilization: u32,

    /// Scaling policy
    #[serde(default = "default_scaling_policy")]
    pub scaling_policy: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[allow(dead_code)]
pub struct OrbitActorStatus {
    /// Current phase of the actor deployment
    pub phase: Option<ActorPhase>,

    /// Number of active actor instances
    pub active_instances: Option<u32>,

    /// Number of ready actor instances
    pub ready_instances: Option<u32>,

    /// Conditions affecting the actor
    #[serde(default)]
    pub conditions: Vec<ActorCondition>,

    /// Node distribution of actor instances
    #[serde(default)]
    pub node_distribution: BTreeMap<String, u32>,

    /// Last observed generation
    pub observed_generation: Option<i64>,

    /// Actor metrics summary
    pub metrics: Option<ActorMetrics>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[allow(dead_code)]
pub enum ActorPhase {
    Pending,
    Deploying,
    Running,
    Scaling,
    Updating,
    Failed,
    Terminating,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[allow(dead_code)]
pub struct ActorCondition {
    /// Type of condition
    pub condition_type: String,

    /// Status of the condition
    pub status: String,

    /// Last transition time
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,

    /// Reason for the condition
    pub reason: Option<String>,

    /// Human-readable message
    pub message: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[allow(dead_code)]
pub struct ActorMetrics {
    /// Total number of invocations
    pub total_invocations: u64,

    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,

    /// Current CPU utilization percentage
    pub cpu_utilization: f32,

    /// Current memory utilization percentage
    pub memory_utilization: f32,

    /// Error rate percentage
    pub error_rate: f32,

    /// Last updated timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

// Default value functions for ActorConfig
#[allow(dead_code)]
fn default_activation_timeout() -> u32 {
    30
}

#[allow(dead_code)]
fn default_deactivation_timeout() -> u32 {
    30
}

#[allow(dead_code)]
fn default_max_actors_per_node() -> u32 {
    1000
}

#[allow(dead_code)]
fn default_serialization_format() -> String {
    "json".to_string()
}

// Default value functions for ScalingConfig
#[allow(dead_code)]
fn default_min_instances() -> u32 {
    1
}

#[allow(dead_code)]
fn default_max_instances() -> u32 {
    100
}

#[allow(dead_code)]
fn default_target_cpu() -> u32 {
    70
}

#[allow(dead_code)]
fn default_target_memory() -> u32 {
    80
}

#[allow(dead_code)]
fn default_scaling_policy() -> String {
    "horizontal".to_string()
}
