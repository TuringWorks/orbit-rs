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
Make sure to install dependencies for the protocols you intend to test. Each subdirectory has its own `requirements.txt`.
To install all dependencies:
```bash
pip install -r postgresql/requirements.txt
pip install -r aql/requirements.txt
pip install -r cql/requirements.txt
pip install -r neo4j/requirements.txt
pip install -r mongodb/requirements.txt
pip install -r mysql/requirements.txt
pip install -r redis/requirements.txt
pip install -r orbitql/requirements.txt
```
