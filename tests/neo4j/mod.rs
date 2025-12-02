// Neo4j Bolt Protocol Tests
// These tests are disabled by default and enabled only when NEO4J_FEATURES flag is set
//
// Note: Only existing test modules are included. Additional modules can be added as they are implemented:
// - bolt_protocol_tests.rs
// - graph_algorithms_tests.rs
// - integration_tests.rs

#[cfg(feature = "neo4j-features")]
mod cypher_parser_tests;

#[cfg(feature = "neo4j-features")]
mod graph_actors_tests;