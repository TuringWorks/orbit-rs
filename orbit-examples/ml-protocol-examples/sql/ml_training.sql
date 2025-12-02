-- ============================================================================
-- ML Training Examples for Orbit PostgreSQL Protocol
-- ============================================================================
-- This file demonstrates model training using Orbit's ML SQL functions.
-- Run against Orbit PostgreSQL interface (port 5432)
-- ============================================================================

-- ----------------------------------------------------------------------------
-- Setup: Create sample tables
-- ----------------------------------------------------------------------------

-- Customer churn prediction dataset
CREATE TABLE IF NOT EXISTS customers (
    customer_id SERIAL PRIMARY KEY,
    tenure INT,
    monthly_charges DECIMAL(10,2),
    total_charges DECIMAL(10,2),
    contract_type VARCHAR(50),
    payment_method VARCHAR(50),
    churned BOOLEAN
);

-- Insert sample data
INSERT INTO customers (tenure, monthly_charges, total_charges, contract_type, payment_method, churned)
VALUES
    (12, 29.85, 358.20, 'month-to-month', 'credit_card', false),
    (72, 109.70, 7896.00, 'two-year', 'bank_transfer', false),
    (2, 53.85, 107.70, 'month-to-month', 'electronic_check', true),
    (45, 42.30, 1903.50, 'one-year', 'credit_card', false),
    (3, 70.70, 212.10, 'month-to-month', 'electronic_check', true),
    (24, 89.10, 2138.40, 'one-year', 'bank_transfer', false),
    (1, 20.05, 20.05, 'month-to-month', 'electronic_check', true),
    (60, 99.65, 5979.00, 'two-year', 'credit_card', false),
    (36, 65.50, 2358.00, 'one-year', 'bank_transfer', false),
    (6, 45.20, 271.20, 'month-to-month', 'electronic_check', true)
ON CONFLICT DO NOTHING;

-- Transactions for fraud detection
CREATE TABLE IF NOT EXISTS transactions (
    transaction_id SERIAL PRIMARY KEY,
    amount DECIMAL(10,2),
    merchant_category VARCHAR(50),
    hour_of_day INT,
    day_of_week INT,
    location_risk_score DECIMAL(3,2),
    is_fraud BOOLEAN
);

INSERT INTO transactions (amount, merchant_category, hour_of_day, day_of_week, location_risk_score, is_fraud)
VALUES
    (25.50, 'groceries', 10, 2, 0.1, false),
    (1500.00, 'electronics', 3, 0, 0.8, true),
    (45.00, 'restaurant', 19, 5, 0.2, false),
    (2000.00, 'jewelry', 2, 1, 0.9, true),
    (12.99, 'streaming', 20, 4, 0.1, false),
    (89.00, 'clothing', 14, 6, 0.3, false),
    (3500.00, 'electronics', 4, 0, 0.85, true),
    (55.00, 'gas_station', 7, 3, 0.15, false)
ON CONFLICT DO NOTHING;

-- ----------------------------------------------------------------------------
-- Example 1: Train a Random Forest model for churn prediction
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'churn_model_rf',                    -- Model name
    'random_forest',                      -- Algorithm
    ARRAY[tenure, monthly_charges, total_charges],  -- Features
    churned                               -- Target variable
) FROM customers;

-- ----------------------------------------------------------------------------
-- Example 2: Train with hyperparameters
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'churn_model_optimized',
    'random_forest',
    ARRAY[tenure, monthly_charges, total_charges],
    churned,
    '{"n_estimators": 200, "max_depth": 15, "min_samples_split": 5}'::jsonb
) FROM customers;

-- ----------------------------------------------------------------------------
-- Example 3: Train XGBoost model for fraud detection
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'fraud_model_xgb',
    'xgboost',
    ARRAY[amount, hour_of_day, day_of_week, location_risk_score],
    is_fraud,
    '{"max_depth": 10, "learning_rate": 0.1, "n_estimators": 100}'::jsonb
) FROM transactions;

-- ----------------------------------------------------------------------------
-- Example 4: Train Logistic Regression (interpretable model)
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'churn_model_lr',
    'logistic_regression',
    ARRAY[tenure, monthly_charges],
    churned,
    '{"regularization": "l2", "C": 1.0}'::jsonb
) FROM customers;

-- ----------------------------------------------------------------------------
-- Example 5: Train SVM classifier
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'fraud_model_svm',
    'svm',
    ARRAY[amount, hour_of_day, location_risk_score],
    is_fraud,
    '{"kernel": "rbf", "C": 1.0, "gamma": "scale"}'::jsonb
) FROM transactions;

-- ----------------------------------------------------------------------------
-- Example 6: Train Neural Network
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'fraud_model_nn',
    'neural_network',
    ARRAY[amount, hour_of_day, day_of_week, location_risk_score],
    is_fraud,
    '{"hidden_layers": [64, 32, 16], "activation": "relu", "epochs": 100}'::jsonb
) FROM transactions;

-- ----------------------------------------------------------------------------
-- Example 7: Train regression model (predict continuous value)
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'charges_predictor',
    'linear_regression',
    ARRAY[tenure, monthly_charges],
    total_charges
) FROM customers;

-- ----------------------------------------------------------------------------
-- Example 8: Train on filtered data
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'premium_churn_model',
    'random_forest',
    ARRAY[tenure, monthly_charges, total_charges],
    churned
) FROM customers
WHERE monthly_charges > 50;

-- ----------------------------------------------------------------------------
-- Example 9: Model evaluation after training
-- ----------------------------------------------------------------------------

-- Evaluate model performance
SELECT ML_EVALUATE_MODEL(
    'churn_model_rf',
    ARRAY[tenure, monthly_charges, total_charges],
    churned
) FROM customers;

-- ----------------------------------------------------------------------------
-- Example 10: Cross-validation training
-- ----------------------------------------------------------------------------

SELECT ML_TRAIN_MODEL(
    'churn_model_cv',
    'random_forest',
    ARRAY[tenure, monthly_charges, total_charges],
    churned,
    '{"n_estimators": 100, "cross_validation": 5}'::jsonb
) FROM customers;

-- ----------------------------------------------------------------------------
-- Cleanup (optional)
-- ----------------------------------------------------------------------------
-- DROP TABLE IF EXISTS customers;
-- DROP TABLE IF EXISTS transactions;
-- SELECT ML_DELETE_MODEL('churn_model_rf');
-- SELECT ML_DELETE_MODEL('fraud_model_xgb');
