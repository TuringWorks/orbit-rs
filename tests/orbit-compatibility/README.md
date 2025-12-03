# Orbit Compatibility Layer

This directory contains compatibility checks and tests for various protocols supported by Orbit.
Each subdirectory corresponds to a specific protocol and contains tests to verify compliance and compatibility.

## Supported Protocols

- `postgresql/`: PostgreSQL compatibility checks (PCI, pgvector, TimescaleDB, PG18).
- `aql/`: ArangoDB Query Language (AQL) checks.
- `cql/`: Cassandra Query Language (CQL) checks.
- `neo4j/`: Neo4j (Bolt/Cypher) checks.
- `mongodb/`: MongoDB wire protocol checks.
- `mysql/`: MySQL protocol checks.
- `redis/`: Redis (RESP) protocol checks.
- `orbitql/`: OrbitQL native checks.
- `run_tests.py`: Unified test runner script.

## Running Tests

You can run all tests or specific protocols using the `run_tests.py` script.

### Run All Tests
```bash
python3 run_tests.py
```

### Run Specific Protocols
```bash
python3 run_tests.py --protocols postgresql redis
```

### Prerequisites
The script will automatically install necessary Python dependencies for the selected protocols.
If you wish to skip automatic installation, use the `--skip-install` flag:
```bash
python3 run_tests.py --skip-install
```
