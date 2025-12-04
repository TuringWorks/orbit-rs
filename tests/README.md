# Orbit-RS Test Suite

This directory contains all test scripts and utilities for the Orbit-RS project.

## Directory Structure

```text
tests/
├── README.md                       # This file
├── Cargo.toml                      # Integration test crate configuration
├── requirements.txt                # Python test dependencies
├── run_integration_tests.py        # Main test runner script
│
├── integration/                    # Python integration tests
│   ├── test_graph_commands.py      # Graph database (Cypher) tests
│   ├── test_timeseries_commands.py # Time series tests
│   ├── test_vector_commands.py     # Vector store tests
│   ├── leader_election_tests.rs    # Leader election integration tests
│   └── leader_election_recovery_tests.rs
│
├── orbit-protocols/                # Protocol-specific tests
│   ├── resp_*.rs                   # Redis RESP protocol tests
│   ├── postgres_*.rs               # PostgreSQL wire protocol tests
│   ├── iceberg_*.rs                # Iceberg storage tests
│   └── protocol_integration_tests.rs
│
├── orbit-engine/                   # Storage engine tests
│   ├── gpu_acceleration_integration.rs
│   ├── simd_acceleration_functional.rs
│   └── icelake_evaluation.rs
│
├── orbit-server/                   # Server integration tests
│   └── integration_test.rs
│
├── neo4j/                          # Neo4j/Cypher compatibility tests
│   ├── cypher_parser_tests.rs
│   └── graph_actors_tests.rs
│
├── graphml/                        # GraphML/embedding tests
│   └── node_embeddings_tests.rs
│
├── bdd/                            # BDD feature tests
│   ├── features/                   # Cucumber feature files
│   └── mod.rs
│
├── features/                       # BDD scenario definitions
│   ├── actor_lifecycle.feature
│   ├── messaging.feature
│   └── neo4j_cypher.feature
│
├── mocks/                          # Mock implementations
│   └── mock_simple.rs
│
├── scripts/                        # Utility scripts
│   ├── fix_handlers.py
│   └── fix_handlers2.py
│
├── verification/                   # Verification scripts
│   ├── verify_all.sh
│   └── quick_check.sh
│
└── *.sh                            # SQL test scripts
    ├── test_comprehensive_sql_keywords.sh
    ├── test_postgres_queries.sh
    └── test_sql_keyword_examples.sh
```

## Integration Tests

The integration tests verify that the Orbit-RS server correctly implements various protocol specifications:

### Graph Commands Tests (`test_graph_commands.py`)

- Tests RedisGraph-compatible GRAPH.* commands
- Verifies Cypher query execution
- Tests node and relationship operations
- Validates query optimization and profiling

### Time Series Tests (`test_timeseries_commands.py`)

- Tests Redis TimeSeries-compatible TS.* commands
- Verifies time series data ingestion and retrieval
- Tests aggregation and compaction rules
- Validates multi-series operations

### Vector Store Tests (`test_vector_commands.py`)

- Tests custom VECTOR.* commands
- Tests RedisSearch-compatible FT.* commands
- Verifies vector similarity search
- Tests k-nearest neighbor operations

## Running Tests

### Prerequisites

1. **Install Python dependencies:**

   ```bash
   pip install -r tests/requirements.txt
   ```

2. **Start Orbit-RS server:**

   ```bash
   # Make sure the Orbit server is running
   # RESP protocol on port 6379 (default)
   # Vector store on port 6381 (default)
   ```

### Running All Tests

```bash

# Run all integration tests
python3 tests/run_integration_tests.py

# Run specific test suite
python3 tests/run_integration_tests.py --test graph
python3 tests/run_integration_tests.py --test timeseries
python3 tests/run_integration_tests.py --test vector

# Custom server configuration
python3 tests/run_integration_tests.py --host localhost --resp-port 6379 --vector-port 6381
```

### Running Individual Tests

You can also run the test files directly:

```bash

# Graph tests
python3 tests/integration/test_graph_commands.py

# Time series tests
python3 tests/integration/test_timeseries_commands.py

# Vector tests
python3 tests/integration/test_vector_commands.py
```

## Utility Scripts

The `scripts/` directory contains utility scripts for development:

- **`fix_handlers.py`** - Helps fix REST API handler code generation
- **`fix_handlers2.py`** - Additional REST handler code fixes

These scripts are used during development to clean up generated code.

## Test Output

The test runner provides comprehensive output including:

- ✅ Individual test results
- 📊 Test summary statistics
- 🏁 Overall pass/fail status
- 🚨 Detailed error messages when tests fail

Example output:

```text
🧪 Orbit-RS Integration Test Suite
==================================================
🚀 Running Graph Commands Integration Tests
✅ CREATE (n:Person {name: 'Alice', age: 30}) RETURN n
✅ Node creation query result: [['n'], [[['id', 1], ['labels', ['Person']], ...]]]
📊 Graph Tests Results: 15/15 tests passed

🏁 FINAL TEST RESULTS
============================================================
Graph             ✅ PASSED
Time Series       ✅ PASSED  
Vector            ✅ PASSED
Overall Result:   ✅ ALL TESTS PASSED
============================================================
```

## Development

When adding new tests:

1. **Integration tests** go in `tests/integration/`
2. **Utility scripts** go in `tests/scripts/`
3. **Update dependencies** in `tests/requirements.txt` if needed
4. **Update the main runner** in `run_integration_tests.py`

## Related Documentation

- [Protocol Implementation Status](../docs/content/protocols/RESP_IMPLEMENTATION_STATUS.md)
- [Graph Commands Documentation](../docs/content/graph_commands.md)
- [Time Series Commands Documentation](../docs/content/timeseries_commands.md)
- [Vector Commands Documentation](../docs/content/vector_commands.md)
- [PRD (Product Requirements Document)](../specifications/PRD.md)
