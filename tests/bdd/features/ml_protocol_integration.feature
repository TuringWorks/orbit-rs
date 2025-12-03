Feature: ML Protocol Integration
  As a developer using Orbit's multi-protocol database
  I want to access ML capabilities through any supported protocol
  So that I can use ML features with my preferred database interface

  # ==========================================================================
  # PostgreSQL Protocol - ML SQL Functions
  # ==========================================================================

  Background:
    Given I have a connection to Orbit

  @postgresql @ml
  Scenario: Train a random forest model via PostgreSQL
    Given I have a connection to Orbit PostgreSQL interface
    And I have a table "customers" with columns:
      | column          | type           |
      | customer_id     | SERIAL         |
      | tenure          | INT            |
      | monthly_charges | DECIMAL(10,2)  |
      | churned         | BOOLEAN        |
    And I have inserted customer data
    When I execute:
      """
      SELECT ML_TRAIN_MODEL(
        'churn_model',
        'random_forest',
        ARRAY[tenure, monthly_charges],
        churned
      ) FROM customers
      """
    Then the operation should succeed
    And the model "churn_model" should be registered

  @postgresql @ml
  Scenario: Run predictions using trained model
    Given I have a trained model "churn_model"
    And I have a table "customers" with customer data
    When I execute:
      """
      SELECT
        customer_id,
        ML_PREDICT('churn_model', ARRAY[tenure, monthly_charges]) AS churn_risk
      FROM customers
      """
    Then the operation should succeed
    And I should get a numeric prediction for each row
    And prediction values should be between 0 and 1

  @postgresql @ml
  Scenario: K-means clustering
    Given I have a table "customers" with columns:
      | column          | type           |
      | customer_id     | SERIAL         |
      | tenure          | INT            |
      | monthly_charges | DECIMAL(10,2)  |
    And I have inserted at least 10 customer records
    When I execute:
      """
      SELECT
        customer_id,
        ML_KMEANS(ARRAY[tenure, monthly_charges], 3) AS segment
      FROM customers
      """
    Then the operation should succeed
    And I should get cluster assignments 0, 1, or 2

  @postgresql @ml @vectors
  Scenario: Generate text embeddings
    Given I have a connection to Orbit PostgreSQL interface
    When I execute:
      """
      SELECT ML_EMBED_TEXT('machine learning tutorial', 'sentence-transformers') AS embedding
      """
    Then the operation should succeed
    And the result should be a vector of dimension 384

  @postgresql @ml @vectors
  Scenario: Semantic search with ML embeddings
    Given I have a table "documents" with columns:
      | column    | type        |
      | id        | SERIAL      |
      | content   | TEXT        |
      | embedding | vector(384) |
    And I have inserted documents with embeddings
    When I execute:
      """
      SELECT content,
             embedding <=> ML_EMBED_TEXT('neural networks', 'sentence-transformers') AS distance
      FROM documents
      ORDER BY distance
      LIMIT 5
      """
    Then the operation should succeed
    And I should get 5 results ordered by relevance

  @postgresql @ml
  Scenario: Statistical correlation analysis
    Given I have a table "sales" with numeric columns
    When I execute:
      """
      SELECT
        ML_CORRELATION(temperature, ice_cream_sales) AS correlation
      FROM sales
      """
    Then the operation should succeed
    And the result should be a number between -1 and 1

  # ==========================================================================
  # Redis (RESP) Protocol - ML Commands
  # ==========================================================================

  @redis @ml
  Scenario: Create ML model via Redis
    Given I have a connection to Orbit Redis interface
    When I execute Redis command "ML.CREATE fraud_model xgboost FEATURES amount,hour LABEL fraud"
    Then the command should succeed
    And the model "fraud_model" should be listed in ML.LIST

  @redis @ml
  Scenario: Train model via Redis
    Given I have a connection to Orbit Redis interface
    And I have created model "fraud_model"
    And I have stored training data in "transactions:*" keys
    When I execute Redis command "ML.TRAIN fraud_model transactions:* EPOCHS 100"
    Then the command should succeed
    And the model "fraud_model" should have status "trained"

  @redis @ml
  Scenario: Run prediction via Redis
    Given I have a trained model "fraud_model" in Redis
    When I execute Redis command "ML.PREDICT fraud_model [500.0, 3]"
    Then the command should return a numeric prediction
    And the prediction should be between 0 and 1

  @redis @ml
  Scenario: Batch predictions via Redis
    Given I have a trained model "fraud_model" in Redis
    And I have transaction data stored in "transaction:*" keys
    When I execute Redis command "ML.PREDICT.BATCH fraud_model transaction:* LIMIT 10"
    Then the command should return predictions for up to 10 transactions

  @redis @ml @vectors
  Scenario: Generate embedding via Redis
    Given I have a connection to Orbit Redis interface
    When I execute Redis command "ML.EMBED 'machine learning' sentence-transformers"
    Then the command should return a vector
    And the vector should have 384 dimensions

  @redis @ml @vectors
  Scenario: Semantic search via Redis
    Given I have a connection to Orbit Redis interface
    And I have a vector index "documents" with embedded documents
    When I execute Redis command "ML.SEARCH.SEMANTIC documents 'neural networks' LIMIT 5"
    Then the command should return 5 results
    And results should be sorted by relevance

  # ==========================================================================
  # HTTP REST API - ML Endpoints
  # ==========================================================================

  @rest @ml
  Scenario: Create model via REST API
    Given I have access to Orbit REST API
    When I POST to "/ml/models" with:
      """
      {
        "name": "fraud_detector",
        "algorithm": "random_forest",
        "features": ["amount", "hour", "merchant"],
        "target": "is_fraud"
      }
      """
    Then the response status should be 201
    And the response should contain "model_id"

  @rest @ml
  Scenario: Train model via REST API
    Given I have created model "fraud_detector" via REST
    When I POST to "/ml/models/fraud_detector/train" with:
      """
      {
        "data_source": "transactions",
        "options": {"epochs": 100}
      }
      """
    Then the response status should be 200
    And the response should contain training metrics

  @rest @ml
  Scenario: Single prediction via REST API
    Given I have a trained model "fraud_detector"
    When I POST to "/ml/models/fraud_detector/predict" with:
      """
      {
        "features": [100.50, 14, "electronics"]
      }
      """
    Then the response status should be 200
    And the response should contain "prediction"
    And the response should contain "confidence"

  @rest @ml
  Scenario: Batch prediction via REST API
    Given I have a trained model "fraud_detector"
    When I POST to "/ml/models/fraud_detector/predict/batch" with:
      """
      {
        "instances": [
          {"features": [100.50, 14, "electronics"]},
          {"features": [25.00, 10, "groceries"]}
        ]
      }
      """
    Then the response status should be 200
    And the response should contain 2 predictions

  @rest @ml
  Scenario: List models via REST API
    Given I have created several ML models
    When I GET "/ml/models"
    Then the response status should be 200
    And the response should contain a list of models

  # ==========================================================================
  # Industry Models
  # ==========================================================================

  @industry @healthcare
  Scenario: Healthcare risk prediction
    Given I have access to healthcare industry models
    When I request a prediction for patient data:
      | field          | value |
      | age            | 45    |
      | bmi            | 28.5  |
      | blood_pressure | 140   |
      | glucose        | 126   |
    Then I should receive a risk assessment
    And the response should include risk factors

  @industry @finance
  Scenario: Fraud detection prediction
    Given I have access to finance industry models
    When I request fraud analysis for transaction:
      | field    | value       |
      | amount   | 1500.00     |
      | merchant | electronics |
      | hour     | 3           |
      | location | foreign     |
    Then I should receive a fraud score
    And the score should indicate high risk

  @industry @retail
  Scenario: Demand forecasting
    Given I have access to retail industry models
    And I have historical sales data for a product
    When I request demand forecast for the next 7 days
    Then I should receive daily forecasts
    And forecasts should include confidence intervals

  # ==========================================================================
  # Model Management
  # ==========================================================================

  @management
  Scenario: Model versioning
    Given I have a trained model "churn_model" version 1
    When I train a new version of "churn_model"
    Then I should have "churn_model" version 2
    And both versions should be available for prediction

  @management
  Scenario: Model evaluation
    Given I have a trained model "churn_model"
    And I have test data with known labels
    When I evaluate the model on test data
    Then I should receive evaluation metrics
    And metrics should include accuracy, precision, recall, and F1

  @management
  Scenario: Delete model
    Given I have a model "temporary_model"
    When I delete the model "temporary_model"
    Then the model should no longer be listed
    And predictions using "temporary_model" should fail

  # ==========================================================================
  # Cross-Protocol Consistency
  # ==========================================================================

  @cross-protocol
  Scenario: Same model accessible from multiple protocols
    Given I train a model "unified_model" via PostgreSQL
    Then the model should be accessible via Redis ML.PREDICT
    And the model should be accessible via REST API
    And predictions should be consistent across protocols

  @cross-protocol
  Scenario: Vector embeddings consistent across protocols
    Given I generate an embedding for "machine learning" via PostgreSQL
    And I generate an embedding for "machine learning" via Redis
    Then both embeddings should be identical

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  @errors
  Scenario: Prediction with invalid model name
    When I try to predict using model "nonexistent_model"
    Then the operation should fail
    And the error should mention "model not found"

  @errors
  Scenario: Training with insufficient data
    Given I have a table with only 2 rows
    When I try to train a model on this data
    Then the operation should fail
    And the error should mention "insufficient training data"

  @errors
  Scenario: Feature dimension mismatch
    Given I have a trained model expecting 3 features
    When I try to predict with 5 features
    Then the operation should fail
    And the error should mention "feature dimension mismatch"

  @errors
  Scenario: Invalid algorithm name
    When I try to create a model with algorithm "invalid_algorithm"
    Then the operation should fail
    And the error should list available algorithms

  # ==========================================================================
  # Performance
  # ==========================================================================

  @performance
  Scenario: Batch prediction performance
    Given I have a trained model "fast_model"
    And I have 1000 records to predict
    When I run batch prediction
    Then all predictions should complete within 5 seconds

  @performance
  Scenario: Embedding generation performance
    Given I have 100 text documents
    When I generate embeddings for all documents
    Then all embeddings should be generated within 10 seconds
