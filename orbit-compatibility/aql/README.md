# AQL (ArangoDB) Compatibility Checks

This directory contains compatibility checks for the ArangoDB Query Language (AQL) protocol.

## Contents

- `compatibility_check.py`: A Python script that verifies AQL syntax support, query execution, and features like graphs and traversals using `python-arango`.
- `requirements.txt`: Python dependencies.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export ARANGO_HOST=http://localhost:8529
    export ARANGO_USER=root
    export ARANGO_PASSWORD=password
    export ARANGO_DB=_system
    
    python3 compatibility_check.py
    ```