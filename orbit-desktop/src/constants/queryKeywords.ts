// OrbitQL keywords for syntax highlighting
export const orbitqlKeywords = [
  // Standard SQL
  'select', 'from', 'where', 'join', 'inner', 'left', 'right', 'full', 'cross',
  'group', 'by', 'having', 'order', 'limit', 'offset', 'insert', 'into', 'values',
  'update', 'set', 'delete', 'create', 'drop', 'table', 'index', 'view',
  
  // OrbitQL Extensions
  'with', 'recursive', 'traverse', 'outbound', 'inbound', 'steps', 'on',
  'relate', 'node', 'edge', 'path', 'connected', 'live', 'diff',
  
  // ML Functions
  'ml_train_model', 'ml_predict', 'ml_evaluate_model', 'ml_drop_model',
  'ml_list_models', 'ml_model_info', 'ml_update_model',
  
  // Boosting Algorithms
  'ml_xgboost', 'ml_lightgbm', 'ml_catboost', 'ml_adaboost', 'ml_gradient_boosting',
  
  // Statistical Functions
  'ml_linear_regression', 'ml_logistic_regression', 'ml_correlation',
  'ml_covariance', 'ml_zscore',
  
  // Feature Engineering
  'ml_normalize', 'ml_encode_categorical', 'ml_polynomial_features',
  'ml_pca', 'ml_feature_selection',
  
  // Vector Operations
  'ml_embed_text', 'ml_embed_image', 'ml_similarity_search',
  'ml_vector_cluster', 'ml_dimensionality_reduction',
  
  // Time Series ML
  'ml_forecast', 'ml_seasonality_decompose', 'ml_anomaly_detection',
  
  // NLP Functions
  'ml_sentiment_analysis', 'ml_extract_entities', 'ml_summarize_text',
  
  // Keywords
  'model', 'train', 'predict', 'using', 'algorithm', 'features', 'target',
  'evaluate', 'score', 'fit', 'transform'
];

// Redis commands for autocompletion
export const redisCommands = [
  'get', 'set', 'del', 'exists', 'expire', 'ttl', 'keys', 'scan',
  'hget', 'hset', 'hdel', 'hgetall', 'hkeys', 'hvals', 'hmget', 'hmset',
  'llen', 'lpush', 'rpush', 'lpop', 'rpop', 'lrange', 'lindex', 'lset',
  'sadd', 'srem', 'smembers', 'scard', 'sismember', 'sunion', 'sinter',
  'zadd', 'zrem', 'zrange', 'zrank', 'zscore', 'zcard', 'zcount',
  'ping', 'echo', 'info', 'dbsize', 'flushdb', 'flushall', 'select',
  'auth', 'quit', 'shutdown', 'lastsave', 'save', 'bgsave'
];