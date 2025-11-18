#!/bin/bash
# Setup script for Azurite integration tests

echo "Setting up Azurite container..."

# Create the container using OpenDAL's internal mechanism
# We'll use a simple Python script since curl auth is complex

python3 - <<'EOF'
import requests
from datetime import datetime

endpoint = "http://127.0.0.1:10000/devstoreaccount1"
container = "orbitstore"

url = f"{endpoint}/{container}?restype=container"

# For Azurite, we can use a simple PUT without proper auth
# as it's more lenient than production Azure
headers = {
    "x-ms-date": datetime.utcnow().strftime('%a, %d %b %Y %H:%M:%S GMT'),
    "x-ms-version": "2021-08-06",
}

try:
    response = requests.put(url, headers=headers)
    if response.status_code in [201, 409]:  # 201 = created, 409 = already exists
        print(f"✓ Container '{container}' is ready")
        exit(0)
    else:
        print(f"✗ Failed to create container: {response.status_code}")
        print(response.text)
        exit(1)
except Exception as e:
    print(f"✗ Error: {e}")
    exit(1)
EOF
