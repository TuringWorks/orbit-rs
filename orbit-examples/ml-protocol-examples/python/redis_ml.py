#!/usr/bin/env python3
"""
Redis ML Example - Orbit Database

This example demonstrates using ML commands through Orbit's Redis (RESP) protocol.
Requires: pip install redis
"""

import redis
import json


# Connection parameters
REDIS_HOST = "localhost"
REDIS_PORT = 6379


def create_sample_data(r):
    """Create sample data for ML examples."""
    print("\n=== Setting Up Sample Data ===")

    # Customer data for churn prediction
    customers = [
        {"id": "1", "tenure": 12, "monthly_charges": 29.85, "churned": 0},
        {"id": "2", "tenure": 72, "monthly_charges": 109.70, "churned": 0},
        {"id": "3", "tenure": 2, "monthly_charges": 53.85, "churned": 1},
        {"id": "4", "tenure": 45, "monthly_charges": 42.30, "churned": 0},
        {"id": "5", "tenure": 3, "monthly_charges": 70.70, "churned": 1},
    ]

    for customer in customers:
        r.hset(f"customer:{customer['id']}", mapping=customer)

    # Transaction data for fraud detection
    transactions = [
        {"id": "1", "amount": 25.50, "merchant": "groceries", "hour": 10, "fraud": 0},
        {"id": "2", "amount": 1500.00, "merchant": "electronics", "hour": 3, "fraud": 1},
        {"id": "3", "amount": 45.00, "merchant": "restaurant", "hour": 19, "fraud": 0},
        {"id": "4", "amount": 2000.00, "merchant": "jewelry", "hour": 2, "fraud": 1},
    ]

    for txn in transactions:
        r.hset(f"transaction:{txn['id']}", mapping=txn)

    print(f"Created {len(customers)} customer records")
    print(f"Created {len(transactions)} transaction records")


def example_model_management(r):
    """Example: ML model management commands."""
    print("\n=== ML Model Management ===")

    # Create a model
    print("\nCreating fraud detection model...")
    try:
        result = r.execute_command(
            'ML.CREATE', 'fraud_detector', 'xgboost',
            'FEATURES', 'amount,hour',
            'LABEL', 'fraud'
        )
        print(f"ML.CREATE result: {result}")
    except redis.ResponseError as e:
        print(f"Note: {e}")

    # List models
    print("\nListing all models...")
    try:
        models = r.execute_command('ML.LIST')
        print(f"Available models: {models}")
    except redis.ResponseError as e:
        print(f"ML.LIST: {e}")


def example_model_training(r):
    """Example: Train ML models with Redis commands."""
    print("\n=== ML Model Training ===")

    # Train the fraud model
    print("\nTraining fraud detection model...")
    try:
        # In real usage, this would train on the transaction data
        result = r.execute_command(
            'ML.TRAIN', 'fraud_detector', 'transactions:*',
            'EPOCHS', '100'
        )
        print(f"Training result: {result}")
    except redis.ResponseError as e:
        print(f"ML.TRAIN: {e}")


def example_predictions(r):
    """Example: Run predictions using ML models."""
    print("\n=== ML Predictions ===")

    # Single prediction
    print("\nPredicting fraud for new transaction...")
    try:
        # Predict fraud for a transaction: amount=500, hour=2 (suspicious)
        result = r.execute_command(
            'ML.PREDICT', 'fraud_detector', '[500.0, 2]'
        )
        print(f"Fraud prediction for $500 at 2am: {result}")
    except redis.ResponseError as e:
        print(f"ML.PREDICT: {e}")

    # Prediction with score/confidence
    print("\nPredicting with confidence score...")
    try:
        result = r.execute_command(
            'ML.PREDICT.SCORE', 'fraud_detector', '[1500.0, 3]'
        )
        print(f"Prediction with score: {result}")
    except redis.ResponseError as e:
        print(f"ML.PREDICT.SCORE: {e}")


def example_batch_predictions(r):
    """Example: Batch predictions for efficiency."""
    print("\n=== Batch Predictions ===")

    try:
        result = r.execute_command(
            'ML.PREDICT.BATCH', 'fraud_detector', 'transaction:*',
            'LIMIT', '10'
        )
        print(f"Batch prediction results: {result}")
    except redis.ResponseError as e:
        print(f"ML.PREDICT.BATCH: {e}")


def example_vector_operations(r):
    """Example: Vector operations with ML embeddings."""
    print("\n=== Vector Operations ===")

    # Generate embedding for text
    print("\nGenerating text embedding...")
    try:
        embedding = r.execute_command(
            'ML.EMBED', 'machine learning tutorial', 'sentence-transformers'
        )
        print(f"Embedding generated (first 5 dims): {embedding[:5] if embedding else 'N/A'}...")
    except redis.ResponseError as e:
        print(f"ML.EMBED: {e}")

    # Semantic search
    print("\nSemantic search...")
    try:
        results = r.execute_command(
            'ML.SEARCH.SEMANTIC', 'documents', 'how to train models',
            'LIMIT', '5'
        )
        print(f"Semantic search results: {results}")
    except redis.ResponseError as e:
        print(f"ML.SEARCH.SEMANTIC: {e}")


def example_industry_models(r):
    """Example: Industry-specific ML models."""
    print("\n=== Industry Models ===")

    # Healthcare prediction
    print("\nHealthcare - Disease Risk Prediction...")
    try:
        patient_data = json.dumps({
            "age": 45,
            "bmi": 28.5,
            "blood_pressure": 140,
            "glucose": 126
        })
        result = r.execute_command(
            'ML.HEALTHCARE.PREDICT', 'diabetes_risk', patient_data
        )
        print(f"Diabetes risk prediction: {result}")
    except redis.ResponseError as e:
        print(f"ML.HEALTHCARE.PREDICT: {e}")

    # Finance prediction
    print("\nFinance - Credit Risk Assessment...")
    try:
        customer_data = json.dumps({
            "income": 75000,
            "debt_ratio": 0.35,
            "credit_history_years": 8
        })
        result = r.execute_command(
            'ML.FINANCE.PREDICT', 'credit_risk', customer_data
        )
        print(f"Credit risk assessment: {result}")
    except redis.ResponseError as e:
        print(f"ML.FINANCE.PREDICT: {e}")

    # Retail prediction
    print("\nRetail - Demand Forecast...")
    try:
        product_data = json.dumps({
            "product_id": "SKU-12345",
            "historical_sales": [100, 120, 95, 140, 160],
            "season": "summer"
        })
        result = r.execute_command(
            'ML.RETAIL.PREDICT', 'demand_forecast', product_data
        )
        print(f"Demand forecast: {result}")
    except redis.ResponseError as e:
        print(f"ML.RETAIL.PREDICT: {e}")


def example_time_series(r):
    """Example: Time series ML operations."""
    print("\n=== Time Series ML ===")

    # Store time series data
    print("\nStoring time series data...")
    sales_data = [100, 120, 95, 140, 160, 155, 180, 175, 190, 210]
    for i, value in enumerate(sales_data):
        r.zadd("sales:daily", {f"day:{i}": value})

    # Forecast
    print("\nForecasting future sales...")
    try:
        result = r.execute_command(
            'ML.FORECAST', 'sales:daily', '7'  # Forecast 7 days
        )
        print(f"Sales forecast (next 7 days): {result}")
    except redis.ResponseError as e:
        print(f"ML.FORECAST: {e}")

    # Anomaly detection
    print("\nDetecting anomalies...")
    try:
        result = r.execute_command(
            'ML.ANOMALY.DETECT', 'sales:daily'
        )
        print(f"Anomaly detection result: {result}")
    except redis.ResponseError as e:
        print(f"ML.ANOMALY.DETECT: {e}")


def example_model_evaluation(r):
    """Example: Model evaluation and metrics."""
    print("\n=== Model Evaluation ===")

    try:
        result = r.execute_command(
            'ML.EVALUATE', 'fraud_detector', 'transactions:test:*'
        )
        print(f"Model evaluation metrics: {result}")
    except redis.ResponseError as e:
        print(f"ML.EVALUATE: {e}")


def main():
    """Run all Redis ML examples."""
    print("=" * 60)
    print("Orbit ML Examples - Redis (RESP) Protocol")
    print("=" * 60)

    try:
        r = redis.Redis(host=REDIS_HOST, port=REDIS_PORT, decode_responses=True)
        r.ping()
        print(f"Connected to Orbit Redis at {REDIS_HOST}:{REDIS_PORT}")

        # Run examples
        create_sample_data(r)
        example_model_management(r)
        example_model_training(r)
        example_predictions(r)
        example_batch_predictions(r)
        example_vector_operations(r)
        example_industry_models(r)
        example_time_series(r)
        example_model_evaluation(r)

        print("\n" + "=" * 60)
        print("All examples completed!")
        print("Note: Some commands may show errors if ML features")
        print("are not fully implemented in the current server version.")
        print("=" * 60)

    except redis.ConnectionError as e:
        print(f"\nConnection Error: {e}")
        print("Make sure Orbit server is running on port 6379")


if __name__ == "__main__":
    main()
