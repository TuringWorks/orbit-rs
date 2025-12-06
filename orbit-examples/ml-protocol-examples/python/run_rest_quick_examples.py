#!/usr/bin/env python3
"""
Run Orbit-RS REST ML quick examples using Python's standard library.

No external dependencies required (uses urllib.request).

Endpoints used:
- POST /api/ml/models/train
- POST /api/ml/predict
"""

import json
import urllib.request
import urllib.error

BASE_URL = "http://localhost:8080"


def post_json(path: str, payload: dict):
    url = f"{BASE_URL}{path}"
    data = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(url, data=data, headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = resp.read().decode("utf-8")
            print(f"POST {path} -> {resp.status}")
            try:
                print(json.dumps(json.loads(body), indent=2))
            except Exception:
                print(body)
    except urllib.error.HTTPError as e:
        print(f"HTTP {e.code} for {path}: {e.read().decode('utf-8')}")
    except urllib.error.URLError as e:
        print(f"Error calling {path}: {e}")


def main():
    print("=" * 60)
    print("Orbit ML Examples - REST API")
    print("=" * 60)

    # Train a small random forest model
    post_json(
        "/api/ml/models/train",
        {
            "name": "demo_rf",
            "algorithm": "random_forest",
            "features": [[0.2, 0.8], [0.9, 0.1], [0.5, 0.5]],
            "labels": [1, 0, 1],
        },
    )

    # Predict using the trained model
    post_json(
        "/api/ml/predict",
        {
            "name": "demo_rf",
            "features": [[0.2, 0.8], [0.7, 0.3]],
        },
    )

    print("=" * 60)
    print("REST examples completed.")


if __name__ == "__main__":
    main()

