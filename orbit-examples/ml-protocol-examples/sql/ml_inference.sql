-- ============================================================================
-- ML Inference Examples for Orbit PostgreSQL Protocol
-- ============================================================================
-- This file demonstrates running predictions using trained ML models.
-- Assumes models from ml_training.sql have been created.
-- ============================================================================

-- ----------------------------------------------------------------------------
-- Example 1: Basic prediction
-- ----------------------------------------------------------------------------

-- Predict churn probability for all customers
SELECT
    customer_id,
    tenure,
    monthly_charges,
    ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS churn_probability
FROM customers
ORDER BY churn_probability DESC;

-- ----------------------------------------------------------------------------
-- Example 2: Prediction with filtering
-- ----------------------------------------------------------------------------

-- Find high-risk customers (churn probability > 70%)
SELECT
    customer_id,
    tenure,
    monthly_charges,
    contract_type,
    ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS churn_probability
FROM customers
WHERE ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) > 0.7
ORDER BY churn_probability DESC;

-- ----------------------------------------------------------------------------
-- Example 3: Prediction with case statement for categorization
-- ----------------------------------------------------------------------------

SELECT
    customer_id,
    tenure,
    ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS churn_prob,
    CASE
        WHEN ML_PREDICT('churn_model_rf',
            ARRAY[tenure, monthly_charges, total_charges]) > 0.7 THEN 'HIGH_RISK'
        WHEN ML_PREDICT('churn_model_rf',
            ARRAY[tenure, monthly_charges, total_charges]) > 0.3 THEN 'MEDIUM_RISK'
        ELSE 'LOW_RISK'
    END AS risk_category
FROM customers;

-- ----------------------------------------------------------------------------
-- Example 4: Fraud detection predictions
-- ----------------------------------------------------------------------------

-- Score all transactions for fraud
SELECT
    transaction_id,
    amount,
    merchant_category,
    hour_of_day,
    ML_PREDICT('fraud_model_xgb',
        ARRAY[amount, hour_of_day, day_of_week, location_risk_score]) AS fraud_score
FROM transactions
ORDER BY fraud_score DESC;

-- ----------------------------------------------------------------------------
-- Example 5: Prediction with explanation (feature importance)
-- ----------------------------------------------------------------------------

SELECT
    customer_id,
    ML_PREDICT_EXPLAIN('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS prediction_details
FROM customers
LIMIT 5;

-- ----------------------------------------------------------------------------
-- Example 6: Batch prediction for new data
-- ----------------------------------------------------------------------------

-- Predict for hypothetical new customers
WITH new_customers AS (
    SELECT 1 AS id, 3 AS tenure, 75.00 AS monthly, 225.00 AS total
    UNION ALL
    SELECT 2, 24, 55.00, 1320.00
    UNION ALL
    SELECT 3, 1, 90.00, 90.00
    UNION ALL
    SELECT 4, 48, 45.00, 2160.00
)
SELECT
    id,
    tenure,
    monthly,
    ML_PREDICT('churn_model_rf', ARRAY[tenure, monthly, total]) AS churn_prob
FROM new_customers;

-- ----------------------------------------------------------------------------
-- Example 7: Compare predictions from multiple models
-- ----------------------------------------------------------------------------

SELECT
    customer_id,
    ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS rf_prediction,
    ML_PREDICT('churn_model_lr',
        ARRAY[tenure, monthly_charges]) AS lr_prediction,
    ABS(
        ML_PREDICT('churn_model_rf', ARRAY[tenure, monthly_charges, total_charges]) -
        ML_PREDICT('churn_model_lr', ARRAY[tenure, monthly_charges])
    ) AS model_disagreement
FROM customers
ORDER BY model_disagreement DESC;

-- ----------------------------------------------------------------------------
-- Example 8: Regression prediction
-- ----------------------------------------------------------------------------

-- Predict total charges based on tenure and monthly charges
SELECT
    customer_id,
    tenure,
    monthly_charges,
    total_charges AS actual_total,
    ML_PREDICT('charges_predictor',
        ARRAY[tenure, monthly_charges]) AS predicted_total,
    total_charges - ML_PREDICT('charges_predictor',
        ARRAY[tenure, monthly_charges]) AS prediction_error
FROM customers;

-- ----------------------------------------------------------------------------
-- Example 9: Aggregations with predictions
-- ----------------------------------------------------------------------------

-- Average churn risk by contract type
SELECT
    contract_type,
    COUNT(*) AS customer_count,
    AVG(ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges])) AS avg_churn_risk,
    MAX(ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges])) AS max_churn_risk
FROM customers
GROUP BY contract_type
ORDER BY avg_churn_risk DESC;

-- ----------------------------------------------------------------------------
-- Example 10: Real-time scoring for incoming transaction
-- ----------------------------------------------------------------------------

-- Check if a new transaction is fraudulent
SELECT
    CASE
        WHEN ML_PREDICT('fraud_model_xgb', ARRAY[1500.00, 3, 0, 0.8]) > 0.5
        THEN 'BLOCK_TRANSACTION'
        ELSE 'ALLOW_TRANSACTION'
    END AS decision,
    ML_PREDICT('fraud_model_xgb', ARRAY[1500.00, 3, 0, 0.8]) AS fraud_score;

-- ----------------------------------------------------------------------------
-- Example 11: Prediction with window functions
-- ----------------------------------------------------------------------------

-- Track prediction changes over customer tenure groups
SELECT
    customer_id,
    tenure,
    ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS churn_prob,
    AVG(ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]))
        OVER (PARTITION BY
            CASE
                WHEN tenure < 12 THEN 'new'
                WHEN tenure < 36 THEN 'established'
                ELSE 'loyal'
            END) AS avg_risk_in_segment
FROM customers;

-- ----------------------------------------------------------------------------
-- Example 12: Prediction confidence intervals
-- ----------------------------------------------------------------------------

SELECT
    customer_id,
    ML_PREDICT('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges]) AS point_estimate,
    ML_PREDICT_CONFIDENCE('churn_model_rf',
        ARRAY[tenure, monthly_charges, total_charges], 0.95) AS confidence_interval
FROM customers;
