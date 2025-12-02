#!/usr/bin/env python3
"""
REST API ML Example - Orbit Database

This example demonstrates using ML capabilities through Orbit's HTTP REST API.
Requires: pip install requests
"""

import requests
import json

# Base URL for Orbit REST API
BASE_URL = "http://localhost:8080"


def check_connection():
    """Check if the server is reachable."""
    try:
        response = requests.get(f"{BASE_URL}/health", timeout=5)
        return response.status_code == 200
    except requests.exceptions.ConnectionError:
        return False


def example_create_model():
    """Example: Create an ML model via REST API."""
    print("\n=== Creating ML Model ===")

    model_config = {
        "name": "fraud_detector",
        "algorithm": "random_forest",
        "features": ["amount", "merchant_category", "hour_of_day"],
        "target": "is_fraud",
        "options": {
            "n_estimators": 100,
            "max_depth": 10
        }
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/models",
            json=model_config,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code in [200, 201]:
            print(f"Model created successfully: {response.json()}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_train_model():
    """Example: Train a model via REST API."""
    print("\n=== Training ML Model ===")

    training_config = {
        "data_source": "transactions",
        "options": {
            "epochs": 100,
            "validation_split": 0.2,
            "early_stopping": True
        }
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/models/fraud_detector/train",
            json=training_config,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            result = response.json()
            print(f"Training completed:")
            print(f"  Accuracy: {result.get('accuracy', 'N/A')}")
            print(f"  Loss: {result.get('loss', 'N/A')}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_single_prediction():
    """Example: Run a single prediction."""
    print("\n=== Single Prediction ===")

    prediction_request = {
        "features": [100.50, "electronics", 14]
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/models/fraud_detector/predict",
            json=prediction_request,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            result = response.json()
            print(f"Prediction result:")
            print(f"  Prediction: {result.get('prediction', 'N/A')}")
            print(f"  Confidence: {result.get('confidence', 'N/A')}")
            print(f"  Model version: {result.get('model_version', 'N/A')}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_batch_prediction():
    """Example: Run batch predictions."""
    print("\n=== Batch Predictions ===")

    batch_request = {
        "instances": [
            {"features": [100.50, "electronics", 14]},
            {"features": [25.00, "groceries", 10]},
            {"features": [1500.00, "jewelry", 3]},
            {"features": [45.00, "restaurant", 19]}
        ]
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/models/fraud_detector/predict/batch",
            json=batch_request,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            results = response.json()
            print("Batch prediction results:")
            for i, pred in enumerate(results.get('predictions', [])):
                print(f"  Instance {i+1}: {pred}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_list_models():
    """Example: List all available models."""
    print("\n=== Listing Models ===")

    try:
        response = requests.get(
            f"{BASE_URL}/ml/models",
            params={"pattern": "*"}
        )

        if response.status_code == 200:
            models = response.json()
            print(f"Available models ({len(models.get('models', []))}):")
            for model in models.get('models', []):
                print(f"  - {model.get('name')}: {model.get('algorithm')} "
                      f"(v{model.get('version', '?')})")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_model_info():
    """Example: Get detailed model information."""
    print("\n=== Model Information ===")

    try:
        response = requests.get(f"{BASE_URL}/ml/models/fraud_detector")

        if response.status_code == 200:
            info = response.json()
            print(f"Model: {info.get('name')}")
            print(f"  Algorithm: {info.get('algorithm')}")
            print(f"  Features: {info.get('features')}")
            print(f"  Target: {info.get('target')}")
            print(f"  Version: {info.get('version')}")
            print(f"  Created: {info.get('created_at')}")
            print(f"  Metrics: {info.get('metrics', {})}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_healthcare_prediction():
    """Example: Healthcare industry model prediction."""
    print("\n=== Healthcare Prediction ===")

    request_data = {
        "model": "disease_risk",
        "patient": {
            "age": 45,
            "symptoms": ["fatigue", "weight_loss"],
            "lab_results": {
                "glucose": 126,
                "bmi": 28.5,
                "blood_pressure": 140
            }
        }
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/industry/healthcare/predict",
            json=request_data,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            result = response.json()
            print(f"Healthcare prediction result:")
            print(f"  Risk score: {result.get('risk_score', 'N/A')}")
            print(f"  Risk factors: {result.get('risk_factors', [])}")
            print(f"  Recommendations: {result.get('recommendations', [])}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_finance_prediction():
    """Example: Finance industry model prediction."""
    print("\n=== Finance Prediction ===")

    request_data = {
        "model": "credit_risk",
        "customer": {
            "income": 75000,
            "debt_ratio": 0.35,
            "credit_history_years": 8,
            "employment_status": "employed",
            "loan_amount": 25000
        }
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/industry/finance/predict",
            json=request_data,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            result = response.json()
            print(f"Credit risk assessment:")
            print(f"  Risk category: {result.get('risk_category', 'N/A')}")
            print(f"  Default probability: {result.get('default_probability', 'N/A')}")
            print(f"  Recommended rate: {result.get('recommended_rate', 'N/A')}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_embedding_generation():
    """Example: Generate text embeddings."""
    print("\n=== Text Embedding Generation ===")

    request_data = {
        "text": "Machine learning is transforming database technology",
        "model": "sentence-transformers"
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/embed",
            json=request_data,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            result = response.json()
            embedding = result.get('embedding', [])
            print(f"Embedding generated:")
            print(f"  Dimensions: {len(embedding)}")
            print(f"  First 5 values: {embedding[:5]}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def example_semantic_search():
    """Example: Semantic search using embeddings."""
    print("\n=== Semantic Search ===")

    request_data = {
        "query": "how to implement neural networks",
        "index": "documents",
        "limit": 5
    }

    try:
        response = requests.post(
            f"{BASE_URL}/ml/search/semantic",
            json=request_data,
            headers={"Content-Type": "application/json"}
        )

        if response.status_code == 200:
            results = response.json()
            print(f"Semantic search results:")
            for i, result in enumerate(results.get('results', []), 1):
                print(f"  {i}. {result.get('content', 'N/A')}")
                print(f"     Score: {result.get('score', 'N/A')}")
        else:
            print(f"Response: {response.status_code} - {response.text}")

    except requests.exceptions.RequestException as e:
        print(f"Request error: {e}")


def main():
    """Run all REST API ML examples."""
    print("=" * 60)
    print("Orbit ML Examples - HTTP REST API")
    print("=" * 60)

    if not check_connection():
        print(f"\nCannot connect to Orbit REST API at {BASE_URL}")
        print("Make sure Orbit server is running with HTTP enabled on port 8080")
        return

    print(f"Connected to Orbit REST API at {BASE_URL}")

    # Run examples
    example_create_model()
    example_train_model()
    example_list_models()
    example_model_info()
    example_single_prediction()
    example_batch_prediction()
    example_healthcare_prediction()
    example_finance_prediction()
    example_embedding_generation()
    example_semantic_search()

    print("\n" + "=" * 60)
    print("All examples completed!")
    print("Note: Some endpoints may return errors if ML features")
    print("are not fully implemented in the current server version.")
    print("=" * 60)


if __name__ == "__main__":
    main()
