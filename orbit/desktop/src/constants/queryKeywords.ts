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

// MySQL keywords (similar to SQL but with MySQL-specific extensions)
export const mysqlKeywords = [
  ...orbitqlKeywords,
  'engine', 'charset', 'collate', 'auto_increment', 'unsigned',
  'zerofill', 'binary', 'varbinary', 'tinyint', 'smallint', 'mediumint',
  'bigint', 'decimal', 'float', 'double', 'bit', 'year', 'enum', 'set',
  'show', 'describe', 'explain', 'use', 'lock', 'unlock', 'grant', 'revoke'
];

// CQL (Cassandra Query Language) keywords
export const cqlKeywords = [
  'select', 'from', 'where', 'insert', 'into', 'values', 'update', 'set',
  'delete', 'create', 'drop', 'alter', 'table', 'keyspace', 'index',
  'primary', 'key', 'partition', 'clustering', 'order', 'by', 'asc', 'desc',
  'allow', 'filtering', 'using', 'ttl', 'timestamp', 'batch', 'apply',
  'truncate', 'grant', 'revoke', 'use', 'describe', 'copy', 'consistency',
  'level', 'one', 'quorum', 'all', 'any', 'local_quorum', 'each_quorum',
  'serial', 'local_serial', 'local_one'
];

// Cypher (Neo4j) keywords
export const cypherKeywords = [
  'match', 'where', 'return', 'create', 'merge', 'delete', 'detach', 'remove',
  'set', 'with', 'unwind', 'union', 'call', 'yield', 'order', 'by', 'skip',
  'limit', 'distinct', 'optional', 'as', 'and', 'or', 'not', 'xor',
  'case', 'when', 'then', 'else', 'end', 'is', 'null', 'exists',
  'all', 'any', 'none', 'single', 'start', 'end', 'node', 'relationship',
  'rel', 'path', 'shortestpath', 'allshortestpaths', 'count', 'collect',
  'sum', 'avg', 'min', 'max', 'head', 'last', 'tail', 'size', 'keys',
  'labels', 'type', 'id', 'properties', 'toInteger', 'toFloat', 'toString',
  'toBoolean', 'coalesce', 'timestamp', 'datetime', 'date', 'time'
];

// AQL (ArangoDB Query Language) keywords
export const aqlKeywords = [
  'for', 'in', 'return', 'let', 'filter', 'sort', 'limit', 'collect',
  'with', 'into', 'keep', 'count', 'aggregate', 'group', 'distinct',
  'insert', 'update', 'replace', 'upsert', 'remove', 'let', 'with',
  'into', 'options', 'new', 'old', 'outbound', 'inbound', 'any',
  'all', 'shortest_path', 'k_shortest_paths', 'k_paths', 'p_paths',
  'traversal', 'graph', 'shortest_path', 'k_shortest_paths',
  'document', 'collection', 'edge', 'vertex', 'prune', 'search',
  'analyzer', 'boost', 'min_match', 'prefix', 'fuzzy', 'wildcard',
  'phrase', 'near', 'within', 'fulltext', 'geo_distance', 'geo_contains'
];