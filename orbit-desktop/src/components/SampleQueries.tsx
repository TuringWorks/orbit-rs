import React, { useState } from 'react';
import styled from 'styled-components';
import { QueryType } from '@/types';

interface SampleQuery {
  id: string;
  name: string;
  description: string;
  category: string;
  queryType: QueryType;
  query: string;
  tags: string[];
}

interface SampleQueriesProps {
  onSelectQuery: (query: string, queryType: QueryType) => void;
  className?: string;
}

const Container = styled.div`
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #1e1e1e;
  border: 1px solid #3c3c3c;
  border-radius: 8px;
  overflow: hidden;
`;

const Header = styled.div`
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid #3c3c3c;
  background: #2d2d2d;
`;

const Title = styled.h3`
  margin: 0;
  color: #ffffff;
  font-size: 16px;
  font-weight: 600;
`;

const Filters = styled.div`
  display: flex;
  gap: 8px;
  align-items: center;
`;

const FilterButton = styled.button<{ active: boolean }>`
  padding: 4px 12px;
  background: ${props => props.active ? '#0078d4' : 'transparent'};
  border: 1px solid ${props => props.active ? '#0078d4' : '#5a5a5a'};
  border-radius: 4px;
  color: ${props => props.active ? 'white' : '#cccccc'};
  font-size: 12px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: ${props => props.active ? '#106ebe' : '#3c3c3c'};
    color: white;
  }
`;

const Content = styled.div`
  flex: 1;
  overflow-y: auto;
  padding: 16px;
`;

const CategorySection = styled.div`
  margin-bottom: 24px;

  &:last-child {
    margin-bottom: 0;
  }
`;

const CategoryHeader = styled.h4`
  margin: 0 0 12px 0;
  color: #0078d4;
  font-size: 14px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
`;

const QueryCard = styled.div`
  background: #2d2d2d;
  border: 1px solid #3c3c3c;
  border-radius: 6px;
  padding: 16px;
  margin-bottom: 12px;
  cursor: pointer;
  transition: all 0.2s;

  &:hover {
    background: #333333;
    border-color: #0078d4;
    transform: translateY(-1px);
  }

  &:last-child {
    margin-bottom: 0;
  }
`;

const QueryHeader = styled.div`
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 8px;
`;

const QueryName = styled.h5`
  margin: 0;
  color: #ffffff;
  font-size: 14px;
  font-weight: 600;
`;

const QueryTypeLabel = styled.span<{ type: string }>`
  padding: 2px 8px;
  border-radius: 12px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  background: ${props => {
    switch (props.type) {
      case 'OrbitQL': return '#107c10';
      case 'SQL': return '#0078d4';
      case 'Redis': return '#d83b01';
      default: return '#5a5a5a';
    }
  }};
  color: white;
`;

const QueryDescription = styled.p`
  margin: 0 0 12px 0;
  color: #cccccc;
  font-size: 13px;
  line-height: 1.4;
`;

const QueryPreview = styled.code`
  display: block;
  background: #1e1e1e;
  border: 1px solid #3c3c3c;
  border-radius: 4px;
  padding: 8px;
  color: #e6e6e6;
  font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
  font-size: 11px;
  line-height: 1.4;
  overflow-x: auto;
  white-space: pre;
`;

const Tags = styled.div`
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 8px;
`;

const Tag = styled.span`
  padding: 2px 6px;
  background: #3c3c3c;
  border-radius: 10px;
  color: #cccccc;
  font-size: 10px;
`;

const sampleQueries: SampleQuery[] = [
  // PostgreSQL Examples
  {
    id: 'pg-basic-select',
    name: 'Basic SELECT Query',
    description: 'Simple data retrieval from a table',
    category: 'PostgreSQL - Basic',
    queryType: QueryType.SQL,
    query: `-- Basic SELECT query
SELECT id, name, email, created_at 
FROM users 
WHERE active = true 
ORDER BY created_at DESC 
LIMIT 10;`,
    tags: ['select', 'basic', 'postgresql']
  },
  {
    id: 'pg-joins',
    name: 'JOIN Operations',
    description: 'Complex query with multiple table joins',
    category: 'PostgreSQL - Basic',
    queryType: QueryType.SQL,
    query: `-- JOIN query with multiple tables
SELECT 
    u.name,
    p.title as product_name,
    o.order_date,
    o.total_amount
FROM users u
JOIN orders o ON u.id = o.user_id
JOIN products p ON o.product_id = p.id
WHERE o.order_date >= CURRENT_DATE - INTERVAL '30 days'
ORDER BY o.order_date DESC;`,
    tags: ['join', 'complex', 'postgresql']
  },
  {
    id: 'pg-aggregations',
    name: 'Aggregation Functions',
    description: 'Using GROUP BY with aggregate functions',
    category: 'PostgreSQL - Basic',
    queryType: QueryType.SQL,
    query: `-- Aggregation and grouping
SELECT 
    EXTRACT(YEAR FROM order_date) as year,
    EXTRACT(MONTH FROM order_date) as month,
    COUNT(*) as total_orders,
    SUM(total_amount) as revenue,
    AVG(total_amount) as avg_order_value
FROM orders
WHERE order_date >= '2024-01-01'
GROUP BY year, month
ORDER BY year DESC, month DESC;`,
    tags: ['aggregation', 'group-by', 'postgresql']
  },

  // OrbitQL ML Examples  
  {
    id: 'orbitql-xgboost',
    name: 'XGBoost Classification',
    description: 'Train and use XGBoost model for binary classification',
    category: 'OrbitQL - ML Boosting',
    queryType: QueryType.OrbitQL,
    query: `-- XGBoost for loan approval prediction
SELECT 
    ML_XGBOOST(
        ARRAY[age, income, credit_score, debt_ratio],
        loan_approved,
        '{"max_depth": 6, "learning_rate": 0.1, "n_estimators": 100}'
    ) as model_performance
FROM loan_applications
WHERE training_set = true;`,
    tags: ['xgboost', 'classification', 'ml', 'orbitql']
  },
  {
    id: 'orbitql-lightgbm',
    name: 'LightGBM Regression',
    description: 'House price prediction using LightGBM',
    category: 'OrbitQL - ML Boosting',
    queryType: QueryType.OrbitQL,
    query: `-- LightGBM for house price prediction
SELECT 
    ML_LIGHTGBM(
        ARRAY[bedrooms, bathrooms, sqft, lot_size, year_built],
        price,
        '{"objective": "regression", "metric": "rmse", "boosting_type": "gbdt"}'
    ) as price_model
FROM real_estate_data
WHERE split_type = 'train';`,
    tags: ['lightgbm', 'regression', 'ml', 'orbitql']
  },
  {
    id: 'orbitql-catboost',
    name: 'CatBoost with Categorical Features',
    description: 'Customer churn prediction with categorical data',
    category: 'OrbitQL - ML Boosting',
    queryType: QueryType.OrbitQL,
    query: `-- CatBoost for customer churn prediction
SELECT 
    ML_CATBOOST(
        ARRAY[tenure, monthly_charges, contract_type, payment_method, tech_support],
        churned,
        '{"iterations": 1000, "depth": 6, "cat_features": [2, 3, 4]}'
    ) as churn_model
FROM customer_data
WHERE dataset_split = 'training';`,
    tags: ['catboost', 'categorical', 'churn', 'orbitql']
  },
  {
    id: 'orbitql-adaboost',
    name: 'AdaBoost Ensemble',
    description: 'Fraud detection using AdaBoost algorithm',
    category: 'OrbitQL - ML Boosting',
    queryType: QueryType.OrbitQL,
    query: `-- AdaBoost for fraud detection
SELECT 
    ML_ADABOOST(
        ARRAY[transaction_amount, merchant_category, hour_of_day, day_of_week],
        is_fraud,
        '{"n_estimators": 50, "learning_rate": 1.0, "algorithm": "SAMME.R"}'
    ) as fraud_model
FROM transaction_history
WHERE labeled = true;`,
    tags: ['adaboost', 'fraud-detection', 'ml', 'orbitql']
  },
  {
    id: 'orbitql-train-model',
    name: 'Model Training & Management',
    description: 'Train and save a named ML model',
    category: 'OrbitQL - ML Management',
    queryType: QueryType.OrbitQL,
    query: `-- Train and save a named model
SELECT 
    ML_TRAIN_MODEL(
        'customer_lifetime_value_v1',
        'XGBOOST',
        ARRAY[age, total_purchases, avg_order_value, days_since_last_order],
        lifetime_value,
        '{"max_depth": 8, "learning_rate": 0.05, "n_estimators": 200}'
    ) as training_result
FROM customer_analytics
WHERE data_quality_score > 0.8;`,
    tags: ['model-training', 'ml-ops', 'orbitql']
  },
  {
    id: 'orbitql-predict',
    name: 'Model Prediction',
    description: 'Make predictions using a trained model',
    category: 'OrbitQL - ML Management',
    queryType: QueryType.OrbitQL,
    query: `-- Make predictions with trained model
SELECT 
    customer_id,
    ML_PREDICT(
        'customer_lifetime_value_v1',
        ARRAY[age, total_purchases, avg_order_value, days_since_last_order]
    ) as predicted_clv,
    ML_PREDICT_PROBA(
        'customer_lifetime_value_v1',
        ARRAY[age, total_purchases, avg_order_value, days_since_last_order]
    ) as prediction_confidence
FROM customers
WHERE prediction_needed = true;`,
    tags: ['prediction', 'ml-inference', 'orbitql']
  },
  {
    id: 'orbitql-evaluate',
    name: 'Model Evaluation',
    description: 'Evaluate model performance on test data',
    category: 'OrbitQL - ML Management',
    queryType: QueryType.OrbitQL,
    query: `-- Evaluate model performance
SELECT 
    ML_EVALUATE_MODEL(
        'customer_churn_v2',
        ARRAY[tenure, monthly_charges, contract_type],
        actual_churn,
        '{"metrics": ["accuracy", "precision", "recall", "f1", "auc"]}'
    ) as model_metrics
FROM customer_test_data
WHERE evaluation_set = true;`,
    tags: ['evaluation', 'metrics', 'ml-ops', 'orbitql']
  },
  {
    id: 'orbitql-feature-importance',
    name: 'Feature Importance Analysis',
    description: 'Analyze which features matter most in your model',
    category: 'OrbitQL - ML Analysis',
    queryType: QueryType.OrbitQL,
    query: `-- Get feature importance from trained model
SELECT 
    ML_FEATURE_IMPORTANCE('loan_approval_model_v3') as feature_analysis,
    ML_MODEL_INFO('loan_approval_model_v3') as model_metadata;`,
    tags: ['feature-importance', 'analysis', 'explainability', 'orbitql']
  },

  // Redis Examples
  {
    id: 'redis-basic-ops',
    name: 'Basic Key-Value Operations',
    description: 'Fundamental Redis operations - SET, GET, DEL',
    category: 'Redis - Basic Operations',
    queryType: QueryType.Redis,
    query: `SET user:1000:name "John Doe"
SET user:1000:email "john@example.com"
SET user:1000:last_login "2024-01-15T10:30:00Z"
GET user:1000:name
EXISTS user:1000:email
DEL user:1000:temp`,
    tags: ['set', 'get', 'basic', 'redis']
  },
  {
    id: 'redis-lists',
    name: 'List Operations',
    description: 'Working with Redis lists - queues and stacks',
    category: 'Redis - Data Structures',
    queryType: QueryType.Redis,
    query: `LPUSH recent_orders "order:5001" "order:5002"
RPUSH pending_tasks "process_payment" "send_email"
LRANGE recent_orders 0 10
LPOP pending_tasks
LLEN recent_orders
LTRIM recent_orders 0 99`,
    tags: ['lists', 'queue', 'stack', 'redis']
  },
  {
    id: 'redis-sets',
    name: 'Set Operations',
    description: 'Unique collections and set operations',
    category: 'Redis - Data Structures',
    queryType: QueryType.Redis,
    query: `SADD active_users "user:123" "user:456" "user:789"
SADD premium_users "user:123" "user:999"
SISMEMBER active_users "user:123"
SINTER active_users premium_users
SUNION active_users premium_users
SCARD active_users`,
    tags: ['sets', 'intersection', 'union', 'redis']
  },
  {
    id: 'redis-hashes',
    name: 'Hash Operations',
    description: 'Object-like data structures in Redis',
    category: 'Redis - Data Structures',
    queryType: QueryType.Redis,
    query: `HSET product:1001 name "Gaming Laptop" price 1299.99 category "Electronics"
HGET product:1001 name
HMGET product:1001 name price
HGETALL product:1001
HINCRBY product:1001 views 1
HDEL product:1001 temp_field`,
    tags: ['hash', 'object', 'increment', 'redis']
  },
  {
    id: 'redis-sorted-sets',
    name: 'Sorted Set Operations',
    description: 'Ranked data structures and leaderboards',
    category: 'Redis - Data Structures',
    queryType: QueryType.Redis,
    query: `ZADD leaderboard 1500 "player:alice" 1200 "player:bob" 1800 "player:carol"
ZRANGE leaderboard 0 2 WITHSCORES
ZREVRANGE leaderboard 0 2 WITHSCORES
ZRANK leaderboard "player:bob"
ZINCRBY leaderboard 50 "player:bob"
ZCOUNT leaderboard 1000 2000`,
    tags: ['sorted-sets', 'leaderboard', 'ranking', 'redis']
  },
  {
    id: 'redis-expiration',
    name: 'Key Expiration and TTL',
    description: 'Managing key lifetimes and cache expiration',
    category: 'Redis - Advanced',
    queryType: QueryType.Redis,
    query: `SET session:abc123 "user_data" EX 3600
SETEX cache:api_response 300 "cached_json_data"
TTL session:abc123
EXPIRE user:temp 1800
PERSIST important_key
PTTL cache:api_response`,
    tags: ['expiration', 'ttl', 'cache', 'redis']
  },
  {
    id: 'redis-pub-sub',
    name: 'Pub/Sub Messaging',
    description: 'Real-time messaging and event publishing',
    category: 'Redis - Advanced',
    queryType: QueryType.Redis,
    query: `SUBSCRIBE notifications
SUBSCRIBE user:*:updates
PUBLISH notifications "Server maintenance in 10 minutes"
PUBLISH user:123:updates "New message received"
PSUBSCRIBE order:*
UNSUBSCRIBE notifications`,
    tags: ['pubsub', 'messaging', 'events', 'redis']
  },
  {
    id: 'redis-transactions',
    name: 'Transactions and Atomicity',
    description: 'Atomic operations using MULTI/EXEC',
    category: 'Redis - Advanced',
    queryType: QueryType.Redis,
    query: `MULTI
INCR counter:page_views
SADD unique_visitors "192.168.1.100"
ZADD hourly_stats 1 "2024-01-15:14"
EXEC

WATCH important_counter
MULTI
GET important_counter
INCR important_counter
EXEC`,
    tags: ['transactions', 'atomic', 'multi-exec', 'redis']
  }
];

export const SampleQueries: React.FC<SampleQueriesProps> = ({ onSelectQuery, className }) => {
  const [activeFilter, setActiveFilter] = useState<string>('all');

  const filters = ['all', 'PostgreSQL', 'OrbitQL', 'Redis'];
  
  const filteredQueries = sampleQueries.filter(query => {
    if (activeFilter === 'all') return true;
    return query.category.includes(activeFilter);
  });

  const groupedQueries = filteredQueries.reduce((groups, query) => {
    if (!groups[query.category]) {
      groups[query.category] = [];
    }
    groups[query.category].push(query);
    return groups;
  }, {} as Record<string, SampleQuery[]>);

  const handleQuerySelect = (query: SampleQuery) => {
    onSelectQuery(query.query, query.queryType);
  };

  return (
    <Container className={className}>
      <Header>
        <Title>📚 Query Examples</Title>
        <Filters>
          {filters.map(filter => (
            <FilterButton
              key={filter}
              active={activeFilter === filter}
              onClick={() => setActiveFilter(filter)}
            >
              {filter}
            </FilterButton>
          ))}
        </Filters>
      </Header>
      
      <Content>
        {Object.entries(groupedQueries).map(([category, queries]) => (
          <CategorySection key={category}>
            <CategoryHeader>{category}</CategoryHeader>
            {queries.map(query => (
              <QueryCard key={query.id} onClick={() => handleQuerySelect(query)}>
                <QueryHeader>
                  <QueryName>{query.name}</QueryName>
                  <QueryTypeLabel type={query.queryType}>{query.queryType}</QueryTypeLabel>
                </QueryHeader>
                <QueryDescription>{query.description}</QueryDescription>
                <QueryPreview>{query.query.split('\n').slice(0, 3).join('\n')}{query.query.split('\n').length > 3 ? '\n...' : ''}</QueryPreview>
                <Tags>
                  {query.tags.map(tag => (
                    <Tag key={tag}>{tag}</Tag>
                  ))}
                </Tags>
              </QueryCard>
            ))}
          </CategorySection>
        ))}
      </Content>
    </Container>
  );
};