# Media Workflow: Content Recommendation & Streaming

## Overview
Personalized content recommendations and streaming analytics.

## Workflow Steps

### 1. User Session (Redis)
```redis
SETEX session:user-789 3600 '{"user_id": "user-789", "device": "smart_tv"}'
```

### 2. ML Content Recommendations (Redis + Neo4j)
```redis
GET ml:recommendations:user-789
# Returns: Top 10 personalized content (91% accuracy)
```

```cypher
// Content similarity graph
MATCH (u:User {id: 'user-789'})-[:WATCHED]->(c:Content)
MATCH (c)-[:SIMILAR_TO]->(rec:Content)
WHERE NOT (u)-[:WATCHED]->(rec)
RETURN rec ORDER BY rec.score DESC LIMIT 10;
```

### 3. Stream Content (MongoDB + Cassandra)
```javascript
// Get content metadata
db.content.findOne({content_id: "movie-456"});
```

```cql
// Log viewing event
INSERT INTO viewing_history (user_id, content_id, timestamp, duration)
VALUES ('user-789', 'movie-456', now(), 7200);
```

### 4. Real-Time Analytics (Redis + Cassandra)
```redis
INCR views:movie-456
ZINCRBY trending:movies 1 "movie-456"
```

### 5. ML Churn Prediction (Redis)
```redis
GET ml:churn:user-789
# Returns: Churn probability (XGBoost, 86% accuracy)
```

**Performance**: <50ms recommendations, real-time streaming analytics
