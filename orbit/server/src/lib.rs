// Allow common clippy lints that are too strict for this codebase
#![allow(clippy::too_many_arguments)] // Complex database operations often need many parameters
#![allow(clippy::type_complexity)] // Type aliases not always cleaner for async types
#![allow(clippy::while_let_loop)] // Sometimes explicit loops are clearer
#![allow(clippy::collapsible_else_if)] // Readability preference
#![allow(clippy::only_used_in_recursion)] // False positives with complex control flow
#![allow(clippy::vec_init_then_push)] // Sometimes clearer to push after init
#![allow(clippy::needless_range_loop)] // Indexed loops often intentional for parallel access
#![allow(clippy::len_zero)] // .len() == 0 vs .is_empty() is a style preference
#![allow(clippy::single_match)] // match vs if let is a readability preference
#![allow(clippy::match_like_matches_macro)] // match vs matches! is a style preference
#![allow(clippy::large_enum_variant)] // Performance tradeoff, not always worth boxing
#![allow(clippy::empty_line_after_doc_comments)] // Doc formatting style
#![allow(clippy::empty_line_after_outer_attr)] // Attribute formatting style
#![allow(clippy::if_same_then_else)] // Sometimes intentional for clarity
#![allow(clippy::unit_arg)] // Unit return values in async code
#![allow(clippy::let_unit_value)] // Unit bindings sometimes useful for documentation
#![allow(clippy::items_after_test_module)] // Module organization preference

pub mod ai;
pub mod config;
pub mod directory;
pub mod features;
#[cfg(feature = "fts")]
pub mod fts;
#[cfg(any(feature = "js-boa", feature = "js-quickjs"))]
pub mod js;
pub mod load_balancer;
pub mod lua;
pub mod mesh;
pub mod persistence;
pub mod protocols;
#[cfg(feature = "python-udf")]
pub mod python;
pub mod server;
pub mod services;
pub mod tcp_proxy;
#[cfg(test)]
mod test_pooling_integration;
pub mod unified_storage;
#[cfg(feature = "wasm-udf")]
pub mod wasm;

pub use features::Features;
pub use load_balancer::{LoadBalancer, LoadBalancerStats, LoadBalancingStrategy, NodeLoad};
pub use mesh::{AddressableDirectory, ClusterManager, ClusterStats, DirectoryStats};
pub use server::{
    OrbitServer, OrbitServerBuilder, OrbitServerConfig, ProtocolConfig, ProtocolStats, ServerStats,
};
pub use unified_storage::{
    UnifiedStorageError, UnifiedStorageIntegration, UnifiedStorageIntegrationConfig,
    UnifiedStorageMetrics,
};
