# MongoDB Compatibility Checks

This directory contains compatibility checks for the MongoDB wire protocol.

## Contents

- `compatibility_check.py`: A Python script that verifies MongoDB operations (CRUD, Aggregation, Indexing) using `pymongo`.
- `requirements.txt`: Python dependencies.

## Running the Checks

1.  Install dependencies:
    ```bash
    pip install -r requirements.txt
    ```

2.  Run the script:
    ```bash
    # Set environment variables if needed
    export MONGO_URI=mongodb://localhost:27017/
    export MONGO_DB=pci_test_db
    
    python3 compatibility_check.py
    ```