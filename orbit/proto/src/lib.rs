// Protocol buffer generated code
tonic::include_proto!("orbit.shared");

pub mod converters;
pub mod services;

/// File descriptor set for gRPC reflection
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("proto_descriptor");

pub use converters::{
    AddressableReferenceConverter, InvocationReasonConverter, KeyConverter, NodeIdConverter,
    NodeStatusConverter, TimestampConverter,
};
pub use services::{OrbitConnectionService, OrbitHealthService};

// Generated protobuf types are available at the root level
