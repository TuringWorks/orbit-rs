# Neo4j Compatibility Checks

This directory contains compatibility checks for the Neo4j (Bolt) protocol.

## Contents

- `compatibility_check.py`: A Python script that verifies Cypher query support using `neo4j` driver.
- `requirements.txt`: Python dependencies.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export NEO4J_URI=bolt://localhost:7687
    export NEO4J_USER=neo4j
    export NEO4J_PASSWORD=password
    
    python3 compatibility_check.py
    ```