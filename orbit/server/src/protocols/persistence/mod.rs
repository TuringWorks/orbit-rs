//! Redis data persistence module for orbit-protocols
//!
//! This module provides persistence providers for Redis RESP protocol data.

pub mod redis_data;
pub mod rocksdb_redis_provider;
// Disabled tikv_redis_provider due to security vulnerabilities in tikv-client dependencies
// See: https://github.com/advisories (protobuf < 3.7.2 - CVE for uncontrolled recursion)
// TODO: Re-enable when tikv-client updates to use protobuf 3.x or prometheus 0.14+
// pub mod tikv_redis_provider;
