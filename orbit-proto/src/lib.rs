// Protocol buffer generated code
tonic::include_proto!("orbit.shared");

pub mod converters;
pub mod services;

pub use converters::{
    AddressableReferenceConverter, InvocationReasonConverter, KeyConverter, NodeIdConverter,
    NodeStatusConverter, TimestampConverter,
};
pub use services::{OrbitConnectionService, OrbitHealthService};

// Generated protobuf types are available at the root level
