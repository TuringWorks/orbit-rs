# Workflow: User Streaming Journey

This workflow follows a user from logging in to watching a movie and generating data for analytics.

## Overview

1.  **Authentication & Session** (Redis)
    - User logs in; session token stored in Redis with TTL.
    - User Profile loaded from RedisJSON (cached from primary DB).

2.  **Discovery** (Cypher & MongoDB)
    - "Recommended for You" list generated via Cypher queries.
    - Movie details fetched from MongoDB.

3.  **Playback Start** (Redis & SQL)
    - Check active stream limit in Redis.
    - Verify entitlement in SQL (e.g., is subscription active?).

4.  **During Playback** (Redis)
    - Heartbeats update `bookmark` key in Redis every 10 seconds.

5.  **Completion** (Cypher)
    - On finish, create `WATCHED` relationship in Cypher.
    - Trigger background job to update `trending` sorted set in Redis.

## Step-by-Step Execution

### Step 1: Login
```bash
# Redis
HSET session:user:1001 ...
```

### Step 2: Get Recommendations
```cypher
// Find movies similar to what user watched
MATCH (u:User {id: '1001'})-[:WATCHED]->(:Movie)-[:IN_GENRE]->(g:Genre)<-[:IN_GENRE]-(rec:Movie)
RETURN rec LIMIT 5
```

### Step 3: Start Stream
```bash
# Redis - Check concurrency
INCR streams:user:1001
# If Result > 3, DENY.
```

### Step 4: Heartbeat
```bash
# Redis - Save position
SET bookmark:user:1001:content:mv_88392 300
```
