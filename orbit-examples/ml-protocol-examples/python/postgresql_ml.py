#!/usr/bin/env python3
"""
PostgreSQL ML Example - Orbit Database

This example demonstrates using ML functions through Orbit's PostgreSQL protocol.
Requires: pip install psycopg2-binary
"""

import psycopg2
from psycopg2.extras import RealDictCursor

# Connection parameters
CONN_PARAMS = {
    "host": "localhost",
    "port": 5432,
    "database": "orbit",
    "user": "orbit",
    "password": "orbit"
}


def setup_sample_data(conn):
    """Create sample tables for ML examples."""
    with conn.cursor() as cur:
        # Customer churn prediction dataset
        cur.execute("""
            CREATE TABLE IF NOT EXISTS customers (
                customer_id SERIAL PRIMARY KEY,
                tenure INT,
                monthly_charges DECIMAL(10,2),
                total_charges DECIMAL(10,2),
                contract_type VARCHAR(50),
                churned BOOLEAN
            )
        """)

        # Insert sample data
        cur.execute("""
            INSERT INTO customers (tenure, monthly_charges, total_charges, contract_type, churned)
            VALUES
                (12, 29.85, 358.20, 'month-to-month', false),
                (72, 109.70, 7896.00, 'two-year', false),
                (2, 53.85, 107.70, 'month-to-month', true),
                (45, 42.30, 1903.50, 'one-year', false),
                (3, 70.70, 212.10, 'month-to-month', true),
                (24, 89.10, 2138.40, 'one-year', false),
                (1, 20.05, 20.05, 'month-to-month', true),
                (60, 99.65, 5979.00, 'two-year', false)
            ON CONFLICT DO NOTHING
        """)

        # Transactions for fraud detection
        cur.execute("""
            CREATE TABLE IF NOT EXISTS transactions (
                transaction_id SERIAL PRIMARY KEY,
                amount DECIMAL(10,2),
                merchant_category VARCHAR(50),
                hour_of_day INT,
                day_of_week INT,
                is_fraud BOOLEAN
            )
        """)

        cur.execute("""
            INSERT INTO transactions (amount, merchant_category, hour_of_day, day_of_week, is_fraud)
            VALUES
                (25.50, 'groceries', 10, 2, false),
                (1500.00, 'electronics', 3, 0, true),
                (45.00, 'restaurant', 19, 5, false),
                (2000.00, 'jewelry', 2, 1, true),
                (12.99, 'streaming', 20, 4, false),
                (89.00, 'clothing', 14, 6, false)
            ON CONFLICT DO NOTHING
        """)

        conn.commit()
        print("Sample data created successfully.")


def example_train_model(conn):
    """Example: Train a churn prediction model."""
    print("\n=== Training Churn Prediction Model ===")

    with conn.cursor() as cur:
        # Train a random forest model for churn prediction
        cur.execute("""
            SELECT ML_TRAIN_MODEL(
                'churn_model',
                'random_forest',
                ARRAY[tenure, monthly_charges, total_charges],
                churned,
                '{"n_estimators": 100, "max_depth": 10}'::jsonb
            ) FROM customers
        """)
        result = cur.fetchone()
        print(f"Model training result: {result}")


def example_predict(conn):
    """Example: Run predictions on customer data."""
    print("\n=== Running Churn Predictions ===")

    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        cur.execute("""
            SELECT
                customer_id,
                tenure,
                monthly_charges,
                ML_PREDICT('churn_model',
                    ARRAY[tenure, monthly_charges, total_charges]) AS churn_probability
            FROM customers
            ORDER BY churn_probability DESC
        """)

        print("\nChurn Risk Assessment:")
        print("-" * 60)
        for row in cur.fetchall():
            risk_level = "HIGH" if row['churn_probability'] > 0.7 else \
                        "MEDIUM" if row['churn_probability'] > 0.3 else "LOW"
            print(f"Customer {row['customer_id']}: "
                  f"Tenure={row['tenure']}mo, "
                  f"Charges=${row['monthly_charges']}, "
                  f"Churn Risk={row['churn_probability']:.2%} ({risk_level})")


def example_clustering(conn):
    """Example: Customer segmentation with K-means clustering."""
    print("\n=== Customer Segmentation (K-Means) ===")

    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        cur.execute("""
            SELECT
                customer_id,
                tenure,
                monthly_charges,
                ML_KMEANS(ARRAY[tenure, monthly_charges, total_charges], 3) AS segment
            FROM customers
            ORDER BY segment, tenure
        """)

        print("\nCustomer Segments:")
        print("-" * 60)
        current_segment = None
        for row in cur.fetchall():
            if row['segment'] != current_segment:
                current_segment = row['segment']
                print(f"\nSegment {current_segment}:")
            print(f"  Customer {row['customer_id']}: "
                  f"Tenure={row['tenure']}mo, Charges=${row['monthly_charges']}")


def example_statistical_functions(conn):
    """Example: Statistical ML functions."""
    print("\n=== Statistical Analysis ===")

    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Correlation analysis
        cur.execute("""
            SELECT
                ML_CORRELATION(tenure, monthly_charges) AS tenure_charges_corr,
                ML_CORRELATION(monthly_charges, total_charges) AS monthly_total_corr
            FROM customers
        """)
        result = cur.fetchone()
        print(f"\nCorrelation Analysis:")
        print(f"  Tenure vs Monthly Charges: {result['tenure_charges_corr']:.4f}")
        print(f"  Monthly vs Total Charges: {result['monthly_total_corr']:.4f}")


def example_feature_engineering(conn):
    """Example: Feature engineering functions."""
    print("\n=== Feature Engineering ===")

    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Normalize features
        cur.execute("""
            SELECT
                customer_id,
                ML_NORMALIZE(ARRAY[tenure, monthly_charges, total_charges], 'minmax')
                    AS normalized_features
            FROM customers
            LIMIT 3
        """)

        print("\nNormalized Features (Min-Max):")
        for row in cur.fetchall():
            print(f"  Customer {row['customer_id']}: {row['normalized_features']}")


def example_fraud_detection(conn):
    """Example: Fraud detection with ML."""
    print("\n=== Fraud Detection ===")

    with conn.cursor() as cur:
        # Train fraud model
        cur.execute("""
            SELECT ML_TRAIN_MODEL(
                'fraud_model',
                'xgboost',
                ARRAY[amount, hour_of_day, day_of_week],
                is_fraud
            ) FROM transactions
        """)

    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Detect potentially fraudulent transactions
        cur.execute("""
            SELECT
                transaction_id,
                amount,
                merchant_category,
                ML_PREDICT('fraud_model',
                    ARRAY[amount, hour_of_day, day_of_week]) AS fraud_score
            FROM transactions
            WHERE ML_PREDICT('fraud_model',
                    ARRAY[amount, hour_of_day, day_of_week]) > 0.5
            ORDER BY fraud_score DESC
        """)

        print("\nPotentially Fraudulent Transactions:")
        print("-" * 60)
        for row in cur.fetchall():
            print(f"Transaction {row['transaction_id']}: "
                  f"${row['amount']} at {row['merchant_category']}, "
                  f"Fraud Score: {row['fraud_score']:.2%}")


def example_vector_operations(conn):
    """Example: Vector operations with ML embeddings."""
    print("\n=== Vector Operations (Semantic Search) ===")

    with conn.cursor() as cur:
        # Create documents table with vector column
        cur.execute("""
            CREATE TABLE IF NOT EXISTS documents (
                id SERIAL PRIMARY KEY,
                content TEXT,
                embedding vector(384)
            )
        """)

        # Insert documents with ML-generated embeddings
        cur.execute("""
            INSERT INTO documents (content, embedding)
            SELECT
                content,
                ML_EMBED_TEXT(content, 'sentence-transformers')
            FROM (VALUES
                ('Introduction to machine learning algorithms'),
                ('Deep learning with neural networks'),
                ('Customer churn prediction using random forests'),
                ('Time series forecasting with ARIMA'),
                ('Natural language processing fundamentals')
            ) AS t(content)
            ON CONFLICT DO NOTHING
        """)
        conn.commit()

    with conn.cursor(cursor_factory=RealDictCursor) as cur:
        # Semantic search
        cur.execute("""
            SELECT
                content,
                embedding <=> ML_EMBED_TEXT('machine learning tutorial',
                    'sentence-transformers') AS distance
            FROM documents
            ORDER BY distance
            LIMIT 3
        """)

        print("\nSemantic Search Results for 'machine learning tutorial':")
        print("-" * 60)
        for i, row in enumerate(cur.fetchall(), 1):
            print(f"{i}. {row['content']}")
            print(f"   Distance: {row['distance']:.4f}")


def main():
    """Run all ML examples."""
    print("=" * 60)
    print("Orbit ML Examples - PostgreSQL Protocol")
    print("=" * 60)

    try:
        conn = psycopg2.connect(**CONN_PARAMS)
        print(f"Connected to Orbit at {CONN_PARAMS['host']}:{CONN_PARAMS['port']}")

        # Setup and run examples
        setup_sample_data(conn)
        example_train_model(conn)
        example_predict(conn)
        example_clustering(conn)
        example_statistical_functions(conn)
        example_feature_engineering(conn)
        example_fraud_detection(conn)
        example_vector_operations(conn)

        print("\n" + "=" * 60)
        print("All examples completed successfully!")
        print("=" * 60)

    except psycopg2.OperationalError as e:
        print(f"\nConnection Error: {e}")
        print("Make sure Orbit server is running on port 5432")
    finally:
        if 'conn' in locals():
            conn.close()


if __name__ == "__main__":
    main()
