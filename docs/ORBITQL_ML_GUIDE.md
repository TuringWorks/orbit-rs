---
layout: default
title: OrbitQL Machine Learning Guide
category: documentation
---

## OrbitQL Machine Learning Guide

### Complete guide to using ML functions and boosting algorithms in OrbitQL

## 🎯 **Overview**

OrbitQL now supports comprehensive machine learning capabilities directly in SQL, including state-of-the-art boosting algorithms like XGBoost, LightGBM, CatBoost, and AdaBoost. This brings ML computation directly to your data without complex ETL pipelines.

## 🏗️ **Quick Start**

### Basic ML Query

```sql
-- Train a customer segmentation model using XGBoost
SELECT ML_XGBOOST(
    ARRAY[age, income, purchase_frequency, days_since_last_order],
    customer_segment
) as model_accuracy
FROM customer_data
WHERE created_at > NOW() - INTERVAL '1 year';
```

### Model Management

```sql
-- Train and save a model
SELECT ML_TRAIN_MODEL(
    'fraud_detector',           -- Model name
    'XGBOOST',                 -- Algorithm
    ARRAY[amount, merchant_category, hour_of_day, user_age],  -- Features
    is_fraud                   -- Target
) as training_result
FROM transactions
WHERE created_at > NOW() - INTERVAL '90 days';

-- Use the trained model for predictions
SELECT 
    transaction_id,
    amount,
    ML_PREDICT('fraud_detector', ARRAY[amount, merchant_category, hour_of_day, user_age]) as fraud_probability
FROM new_transactions
WHERE fraud_probability > 0.8;
```

## 📚 **Boosting Algorithms**

### 1. **XGBoost** - Extreme Gradient Boosting

**Best for**: Large datasets, high accuracy requirements, feature importance analysis

```sql
-- Basic XGBoost with default parameters
SELECT ML_XGBOOST(
    ARRAY[feature1, feature2, feature3],
    target_column
) FROM training_data;

-- Advanced XGBoost with custom parameters
SELECT ML_TRAIN_MODEL(
    'advanced_xgb_model',
    'XGBOOST', 
    ARRAY[age, income, credit_score, debt_ratio],
    loan_default,
    OBJECT(
        'n_estimators', 100,
        'learning_rate', 0.1,
        'max_depth', 6,
        'subsample', 0.8,
        'colsample_bytree', 0.8,
        'reg_alpha', 0.1,
        'reg_lambda', 1.0
    )
) FROM loan_applications;
```

**Parameters:**

- `n_estimators`: Number of boosting rounds (default: 100)
- `learning_rate`: Step size shrinkage (default: 0.3)
- `max_depth`: Maximum tree depth (default: 6)
- `subsample`: Subsample ratio of training instances (default: 1.0)
- `colsample_bytree`: Subsample ratio of features (default: 1.0)

### 2. **LightGBM** - Light Gradient Boosting Machine

**Best for**: Fast training, memory efficiency, categorical features

```sql
-- LightGBM for categorical data
SELECT ML_LIGHTGBM(
    ARRAY[category_id, numerical_feature, another_category],
    sales_volume
) FROM product_sales
WHERE product_category IN ('electronics', 'clothing', 'books');

-- LightGBM with categorical feature specification
SELECT ML_TRAIN_MODEL(
    'sales_forecaster',
    'LIGHTGBM',
    ARRAY[product_category, season, price_tier, promotion_active],
    monthly_sales,
    OBJECT(
        'num_leaves', 31,
        'learning_rate', 0.05,
        'feature_fraction', 0.9,
        'bagging_fraction', 0.8,
        'categorical_feature', ARRAY[0, 2, 3]  -- Indices of categorical columns
    )
) FROM historical_sales;
```

### 3. **CatBoost** - Categorical Boosting

**Best for**: Datasets with many categorical features, minimal preprocessing

```sql
-- CatBoost handles categorical features automatically
SELECT ML_CATBOOST(
    ARRAY[user_country, device_type, browser, os_version, session_duration],
    conversion_rate
) FROM user_sessions;

-- Advanced CatBoost configuration
SELECT ML_TRAIN_MODEL(
    'conversion_optimizer',
    'CATBOOST',
    ARRAY[country, device, browser, hour_of_day, page_views],
    converted,
    OBJECT(
        'iterations', 1000,
        'learning_rate', 0.03,
        'depth', 6,
        'l2_leaf_reg', 3,
        'border_count', 128,
        'auto_class_weights', 'Balanced'
    )
) FROM user_behavior_data;
```

### 4. **AdaBoost** - Adaptive Boosting

**Best for**: Binary classification, when you want interpretable results

```sql
-- Simple AdaBoost for binary classification
SELECT ML_ADABOOST(
    ARRAY[credit_score, income, age, employment_years],
    loan_approved
) FROM loan_decisions;

-- AdaBoost with custom base estimator
SELECT ML_TRAIN_MODEL(
    'credit_approval_model',
    'ADABOOST',
    ARRAY[credit_score, income, debt_ratio, employment_status],
    approved,
    OBJECT(
        'n_estimators', 50,
        'learning_rate', 1.0,
        'algorithm', 'SAMME.R',
        'random_state', 42
    )
) FROM credit_applications;
```

### 5. **Gradient Boosting Machine** - Classic GBM

**Best for**: Regression tasks, when you need smooth predictions

```sql
-- Gradient Boosting for regression
SELECT ML_GRADIENT_BOOSTING(
    ARRAY[bedrooms, bathrooms, sqft, lot_size, age],
    house_price
) FROM real_estate_data;

-- GBM with loss function specification
SELECT ML_TRAIN_MODEL(
    'house_price_model',
    'GRADIENT_BOOSTING',
    ARRAY[bedrooms, bathrooms, sqft, neighborhood_score],
    price,
    OBJECT(
        'n_estimators', 100,
        'learning_rate', 0.1,
        'max_depth', 4,
        'loss', 'huber',           -- Robust to outliers
        'alpha', 0.9              -- Quantile for huber loss
    )
) FROM housing_market;
```

## 🔄 **Model Management Workflow**

### Complete ML Pipeline Example

```sql
-- 1. Train multiple models and compare performance
WITH model_comparison AS (
    SELECT 
        'XGBoost' as algorithm,
        ML_TRAIN_MODEL('customer_xgb', 'XGBOOST', features, target) as result
    FROM customer_features
    UNION ALL
    SELECT 
        'LightGBM' as algorithm,
        ML_TRAIN_MODEL('customer_lgb', 'LIGHTGBM', features, target) as result
    FROM customer_features
    UNION ALL
    SELECT 
        'CatBoost' as algorithm,
        ML_TRAIN_MODEL('customer_cat', 'CATBOOST', features, target) as result
    FROM customer_features
)
-- 2. Evaluate all models
SELECT 
    algorithm,
    ML_EVALUATE_MODEL(
        CONCAT('customer_', LOWER(SUBSTRING(algorithm, 1, 3))), 
        test_features, 
        test_target
    ) as performance_metrics
FROM model_comparison, test_data;

-- 3. Use the best performing model
SELECT 
    customer_id,
    customer_name,
    ML_PREDICT('customer_xgb', 
        ARRAY[age, income, purchase_frequency, days_since_last_order]
    ) as predicted_segment,
    ML_PREDICT('customer_xgb', 
        ARRAY[age, income, purchase_frequency, days_since_last_order]
    )[1] as segment_probability
FROM current_customers;
```

### Model Versioning and Updates

```sql
-- Create versioned models
SELECT ML_TRAIN_MODEL(
    'fraud_detector_v2',
    'XGBOOST',
    ARRAY[amount, merchant_type, time_of_day, location_risk_score],
    is_fraud,
    OBJECT('version', '2.0', 'description', 'Added location risk score')
) FROM fraud_training_data_v2;

-- Online learning - update existing model with new data
SELECT ML_UPDATE_MODEL(
    'fraud_detector_v2',
    ARRAY[amount, merchant_type, time_of_day, location_risk_score],
    is_fraud
) FROM new_fraud_cases
WHERE created_at > NOW() - INTERVAL '7 days';

-- Model metadata
SELECT ML_MODEL_INFO('fraud_detector_v2');
-- Returns:
-- {
--   "name": "fraud_detector_v2",
--   "algorithm": "XGBoost", 
--   "accuracy": 0.94,
--   "features": ["amount", "merchant_type", "time_of_day", "location_risk_score"],
--   "created_at": "2024-01-15T10:30:00Z",
--   "last_updated": "2024-01-20T15:45:00Z",
--   "training_samples": 50000,
--   "version": "2.0"
-- }
```

## 🎯 **Real-World Use Cases**

### 1. **E-commerce Recommendation Engine**

```sql
-- Train collaborative filtering model
WITH user_item_features AS (
    SELECT 
        user_id,
        item_id,
        ARRAY[
            user_age, user_location_score, user_purchase_history_length,
            item_category_id, item_price_tier, item_rating, item_popularity
        ] as features,
        purchased as target
    FROM user_item_interactions
)
SELECT ML_TRAIN_MODEL(
    'recommendation_engine',
    'LIGHTGBM',
    features,
    target,
    OBJECT(
        'objective', 'binary',
        'metric', 'auc',
        'num_leaves', 100,
        'learning_rate', 0.1
    )
) FROM user_item_features;

-- Generate recommendations
SELECT 
    u.user_id,
    u.name,
    i.item_id,
    i.title,
    ML_PREDICT('recommendation_engine', 
        ARRAY[u.age, u.location_score, u.purchase_history_length,
              i.category_id, i.price_tier, i.rating, i.popularity]
    ) as recommendation_score
FROM users u
CROSS JOIN items i
WHERE ML_PREDICT('recommendation_engine', 
    ARRAY[u.age, u.location_score, u.purchase_history_length,
          i.category_id, i.price_tier, i.rating, i.popularity]
) > 0.7
ORDER BY u.user_id, recommendation_score DESC;
```

### 2. **Financial Risk Assessment**

```sql
-- Multi-model ensemble for credit scoring
WITH risk_models AS (
    -- Conservative model (high precision)
    SELECT ML_TRAIN_MODEL(
        'conservative_risk',
        'ADABOOST',
        ARRAY[credit_score, income, debt_ratio, employment_years],
        default_risk,
        OBJECT('n_estimators', 100, 'learning_rate', 0.5)
    ) as conservative_result,
    
    -- Aggressive model (high recall) 
    ML_TRAIN_MODEL(
        'aggressive_risk',
        'XGBOOST',
        ARRAY[credit_score, income, debt_ratio, employment_years, 
              social_media_score, alternative_credit_data],
        default_risk,
        OBJECT('scale_pos_weight', 3)  -- Handle class imbalance
    ) as aggressive_result,
    
    -- Balanced model
    ML_TRAIN_MODEL(
        'balanced_risk',
        'CATBOOST',
        ARRAY[credit_score, income, debt_ratio, employment_years,
              payment_history, account_age],
        default_risk
    ) as balanced_result
    FROM credit_training_data
),

-- Ensemble predictions
ensemble_predictions AS (
    SELECT 
        application_id,
        applicant_name,
        -- Individual model predictions
        ML_PREDICT('conservative_risk', features) as conservative_score,
        ML_PREDICT('aggressive_risk', features) as aggressive_score,
        ML_PREDICT('balanced_risk', features) as balanced_score,
        
        -- Weighted ensemble
        (ML_PREDICT('conservative_risk', features) * 0.4 +
         ML_PREDICT('aggressive_risk', features) * 0.3 +
         ML_PREDICT('balanced_risk', features) * 0.3) as ensemble_score
         
    FROM loan_applications
    WHERE status = 'pending'
)

-- Final risk assessment
SELECT 
    application_id,
    applicant_name,
    ensemble_score,
    CASE 
        WHEN ensemble_score < 0.3 THEN 'LOW_RISK'
        WHEN ensemble_score < 0.7 THEN 'MEDIUM_RISK' 
        ELSE 'HIGH_RISK'
    END as risk_category,
    
    CASE
        WHEN ensemble_score < 0.3 THEN 'APPROVE'
        WHEN ensemble_score < 0.5 THEN 'MANUAL_REVIEW'
        ELSE 'REJECT'
    END as recommendation
    
FROM ensemble_predictions
ORDER BY ensemble_score DESC;
```

### 3. **Predictive Maintenance**

```sql
-- Time series forecasting for equipment failure
WITH sensor_features AS (
    SELECT 
        equipment_id,
        timestamp,
        -- Rolling statistics
        AVG(temperature) OVER (
            PARTITION BY equipment_id 
            ORDER BY timestamp 
            ROWS BETWEEN 23 PRECEDING AND CURRENT ROW
        ) as temp_24h_avg,
        
        STDDEV(vibration) OVER (
            PARTITION BY equipment_id 
            ORDER BY timestamp 
            ROWS BETWEEN 11 PRECEDING AND CURRENT ROW  
        ) as vibration_12h_std,
        
        -- Trend features
        temperature - LAG(temperature, 1) OVER (
            PARTITION BY equipment_id ORDER BY timestamp
        ) as temp_change,
        
        failure_within_7_days
        
    FROM equipment_sensors es
    JOIN equipment_failures ef ON es.equipment_id = ef.equipment_id
        AND ef.failure_date BETWEEN es.timestamp AND es.timestamp + INTERVAL '7 days'
)

-- Train failure prediction model
SELECT ML_TRAIN_MODEL(
    'equipment_failure_predictor',
    'LIGHTGBM',
    ARRAY[temp_24h_avg, vibration_12h_std, temp_change, 
          pressure, rpm, load_factor],
    failure_within_7_days,
    OBJECT(
        'objective', 'binary',
        'is_unbalance', true,         -- Handle class imbalance
        'metric', 'auc',
        'num_iterations', 1000,
        'early_stopping_rounds', 50
    )
) FROM sensor_features;

-- Real-time failure prediction
SELECT 
    equipment_id,
    equipment_name,
    location,
    current_timestamp,
    ML_PREDICT('equipment_failure_predictor', current_features) as failure_probability,
    
    CASE 
        WHEN ML_PREDICT('equipment_failure_predictor', current_features) > 0.8 
        THEN 'CRITICAL - Schedule immediate maintenance'
        WHEN ML_PREDICT('equipment_failure_predictor', current_features) > 0.5
        THEN 'WARNING - Schedule maintenance within 48 hours'
        WHEN ML_PREDICT('equipment_failure_predictor', current_features) > 0.3
        THEN 'CAUTION - Monitor closely'
        ELSE 'NORMAL - Continue operation'
    END as maintenance_recommendation
    
FROM current_equipment_status
WHERE ML_PREDICT('equipment_failure_predictor', current_features) > 0.3
ORDER BY failure_probability DESC;
```

## 🎨 **Feature Engineering Functions**

### Data Preprocessing

```sql
-- Normalize features for better model performance
SELECT 
    customer_id,
    ML_NORMALIZE(
        ARRAY[age, income, purchase_amount], 
        'Z_SCORE'  -- Options: MIN_MAX, Z_SCORE, ROBUST, UNIT_VECTOR
    ) as normalized_features
FROM customer_data;

-- Handle categorical variables
SELECT 
    ML_ENCODE_CATEGORICAL(
        customer_segment, 
        'ONE_HOT'  -- Options: ONE_HOT, LABEL, TARGET, BINARY
    ) as encoded_segment
FROM customers;

-- Principal Component Analysis for dimensionality reduction
SELECT 
    customer_id,
    ML_PCA(
        ARRAY[feature1, feature2, feature3, feature4, feature5],
        3  -- Reduce to 3 components
    ) as reduced_features
FROM high_dimensional_data;
```

### Advanced Feature Engineering

```sql
-- Polynomial features for capturing interactions
SELECT 
    ML_POLYNOMIAL_FEATURES(
        ARRAY[age, income],
        2  -- Degree 2: age, income, age^2, income^2, age*income
    ) as polynomial_features
FROM customer_data;

-- Automatic feature selection
SELECT 
    ML_FEATURE_SELECTION(
        ARRAY[f1, f2, f3, f4, f5, f6, f7, f8, f9, f10],
        target_variable,
        'MUTUAL_INFO'  -- Selection method
    ) as selected_features
FROM training_data;
```

## 📊 **Model Evaluation and Monitoring**

### Performance Metrics

```sql
-- Comprehensive model evaluation
SELECT 
    model_name,
    ML_EVALUATE_MODEL(
        model_name,
        test_features,
        test_target,
        ARRAY['accuracy', 'precision', 'recall', 'f1_score', 'roc_auc']
    ) as metrics
FROM model_registry
CROSS JOIN test_dataset;

-- Model performance over time
WITH daily_predictions AS (
    SELECT 
        DATE(prediction_timestamp) as prediction_date,
        actual_value,
        ML_PREDICT('production_model', features) as predicted_value
    FROM prediction_log
    WHERE prediction_timestamp > NOW() - INTERVAL '30 days'
),
daily_metrics AS (
    SELECT 
        prediction_date,
        -- Classification metrics
        AVG(CASE WHEN actual_value = ROUND(predicted_value) THEN 1.0 ELSE 0.0 END) as daily_accuracy,
        
        -- Regression metrics
        SQRT(AVG(POWER(actual_value - predicted_value, 2))) as daily_rmse,
        AVG(ABS(actual_value - predicted_value)) as daily_mae
        
    FROM daily_predictions
    GROUP BY prediction_date
)
SELECT 
    prediction_date,
    daily_accuracy,
    daily_rmse,
    daily_mae,
    -- Trend detection
    daily_accuracy - LAG(daily_accuracy, 7) OVER (ORDER BY prediction_date) as accuracy_change_7d
FROM daily_metrics
ORDER BY prediction_date;
```

## 🔧 **Advanced Configuration**

### Hyperparameter Tuning

```sql
-- Grid search for hyperparameter optimization
WITH hyperparameter_grid AS (
    SELECT * FROM (
        VALUES 
        (100, 0.1, 6), (100, 0.1, 8), (100, 0.1, 10),
        (200, 0.1, 6), (200, 0.1, 8), (200, 0.1, 10),
        (100, 0.05, 6), (100, 0.05, 8), (100, 0.05, 10)
    ) as params(n_estimators, learning_rate, max_depth)
),
model_results AS (
    SELECT 
        n_estimators,
        learning_rate, 
        max_depth,
        ML_TRAIN_MODEL(
            CONCAT('xgb_', n_estimators, '_', CAST(learning_rate*100 AS INT), '_', max_depth),
            'XGBOOST',
            ARRAY[feature1, feature2, feature3],
            target,
            OBJECT(
                'n_estimators', n_estimators,
                'learning_rate', learning_rate,
                'max_depth', max_depth
            )
        ) as training_result
    FROM hyperparameter_grid
    CROSS JOIN training_data
),
model_evaluation AS (
    SELECT 
        *,
        ML_EVALUATE_MODEL(
            model_name,
            validation_features,
            validation_target
        ) as validation_score
    FROM model_results
    CROSS JOIN validation_data
)
-- Select best hyperparameters
SELECT 
    n_estimators,
    learning_rate,
    max_depth,
    validation_score
FROM model_evaluation
ORDER BY validation_score->>'accuracy' DESC
LIMIT 1;
```

### Model Deployment and A/B Testing

```sql
-- Deploy model with A/B testing
CREATE TABLE model_deployment_config (
    model_name VARCHAR(100),
    traffic_percentage DECIMAL(3,2),
    is_active BOOLEAN,
    deployment_date TIMESTAMP
);

-- A/B test configuration
INSERT INTO model_deployment_config VALUES
('fraud_detector_v1', 0.5, true, NOW()),
('fraud_detector_v2', 0.5, true, NOW());

-- A/B testing query
WITH model_assignment AS (
    SELECT 
        transaction_id,
        CASE 
            WHEN HASH(transaction_id) % 100 < 50 THEN 'fraud_detector_v1'
            ELSE 'fraud_detector_v2'
        END as assigned_model
    FROM transactions
    WHERE timestamp > NOW() - INTERVAL '1 hour'
),
predictions AS (
    SELECT 
        ma.transaction_id,
        ma.assigned_model,
        t.actual_fraud,
        ML_PREDICT(ma.assigned_model, 
            ARRAY[t.amount, t.merchant_category, t.hour_of_day]
        ) as fraud_prediction
    FROM model_assignment ma
    JOIN transactions t ON ma.transaction_id = t.id
)
-- Compare model performance
SELECT 
    assigned_model,
    COUNT(*) as predictions_made,
    AVG(CASE WHEN ROUND(fraud_prediction) = actual_fraud THEN 1.0 ELSE 0.0 END) as accuracy,
    AVG(fraud_prediction) as avg_fraud_score
FROM predictions
GROUP BY assigned_model;
```

## 📈 **Integration with Orbit Desktop**

The Orbit Desktop application provides a rich UI for interacting with ML models:

### Model Management Interface

- **Visual Model Training**: Drag-and-drop interface for feature selection
- **Performance Dashboard**: Real-time model metrics and performance charts
- **Hyperparameter Tuning**: GUI for parameter optimization
- **Model Versioning**: Track model evolution and rollbacks

### Query Builder

- **ML Query Templates**: Pre-built templates for common ML tasks
- **Syntax Highlighting**: OrbitQL ML function highlighting
- **Auto-completion**: IntelliSense for ML functions and parameters
- **Result Visualization**: Charts and graphs for ML results

### Example Desktop Usage

```javascript
// In Orbit Desktop - Execute ML query
const mlQuery = `
    SELECT ML_XGBOOST(
        ARRAY[age, income, credit_score],
        loan_default
    ) as model_accuracy
    FROM loan_data
`;

const result = await orbitDesktop.executeQuery(mlQuery);
// Result includes model performance metrics and visualizations
```

## 🚀 **Best Practices**

### 1. **Data Preparation**

```sql
-- Always check data quality before training
SELECT 
    COUNT(*) as total_rows,
    COUNT(DISTINCT customer_id) as unique_customers,
    AVG(CASE WHEN age IS NULL THEN 1.0 ELSE 0.0 END) as age_null_rate,
    MIN(age) as min_age,
    MAX(age) as max_age,
    STDDEV(income) as income_std
FROM customer_data;

-- Handle missing values
SELECT 
    customer_id,
    COALESCE(age, (SELECT AVG(age) FROM customer_data)) as age,
    COALESCE(income, 0) as income,
    CASE WHEN phone_number IS NULL THEN 0 ELSE 1 END as has_phone
FROM customer_data;
```

### 2. **Model Selection Strategy**

```sql
-- Use cross-validation for model selection
WITH fold_results AS (
    SELECT 
        model_type,
        fold_number,
        ML_CROSS_VALIDATE(
            model_type,
            features,
            target,
            fold_number,
            5  -- 5-fold CV
        ) as cv_score
    FROM (
        VALUES ('XGBOOST'), ('LIGHTGBM'), ('CATBOOST')
    ) as models(model_type)
    CROSS JOIN generate_series(1, 5) as folds(fold_number)
    CROSS JOIN training_data
)
SELECT 
    model_type,
    AVG(cv_score) as mean_cv_score,
    STDDEV(cv_score) as cv_score_std
FROM fold_results
GROUP BY model_type
ORDER BY mean_cv_score DESC;
```

### 3. **Production Monitoring**

```sql
-- Set up model monitoring
CREATE TABLE model_performance_log (
    model_name VARCHAR(100),
    prediction_date DATE,
    accuracy DECIMAL(4,3),
    precision DECIMAL(4,3),
    recall DECIMAL(4,3),
    data_drift_score DECIMAL(4,3),
    created_at TIMESTAMP DEFAULT NOW()
);

-- Daily model monitoring
INSERT INTO model_performance_log
SELECT 
    'production_model',
    CURRENT_DATE,
    -- Performance metrics from daily predictions
    AVG(CASE WHEN actual = ROUND(predicted) THEN 1.0 ELSE 0.0 END),
    AVG(CASE WHEN ROUND(predicted) = 1 AND actual = 1 THEN 1.0 ELSE 0.0 END) /
    AVG(CASE WHEN ROUND(predicted) = 1 THEN 1.0 ELSE 0.0 END),
    AVG(CASE WHEN actual = 1 AND ROUND(predicted) = 1 THEN 1.0 ELSE 0.0 END) /
    AVG(CASE WHEN actual = 1 THEN 1.0 ELSE 0.0 END),
    -- Data drift detection would be implemented here
    0.0,
    NOW()
FROM daily_predictions
WHERE prediction_date = CURRENT_DATE;
```

## 🎯 **Summary**

OrbitQL's ML capabilities bring enterprise-grade machine learning directly to your SQL environment:

- **🚀 Boosting Algorithms**: XGBoost, LightGBM, CatBoost, AdaBoost, and GBM
- **🎯 Model Management**: Train, evaluate, deploy, and monitor models
- **🔧 Feature Engineering**: Built-in preprocessing and feature selection
- **📊 Production Ready**: A/B testing, monitoring, and drift detection
- **🖥️ Desktop UI**: Rich graphical interface with Orbit Desktop
- **⚡ Performance**: Optimized for large-scale data processing

Start building intelligent applications today with OrbitQL's comprehensive ML toolkit!

---

*For more examples and advanced use cases, visit the [Orbit Desktop ML Examples](/examples/ml/) directory.*
