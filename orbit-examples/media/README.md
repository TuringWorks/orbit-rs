# Media & Entertainment

Comprehensive examples demonstrating OrbitRS's multi-protocol capabilities for streaming platforms, content delivery, recommendation engines, and entertainment analytics.

## Why OrbitRS for Media?

OrbitRS excels in media environments where:
- **Sub-50ms recommendations** require real-time ML scoring with cached personalization
- **High-volume telemetry** from millions of concurrent streams needs time-series optimization
- **Multi-protocol access** allows player SDKs (Redis), catalog systems (PostgreSQL), and analytics (CQL) to share data
- **Vector search** enables content similarity and collaborative filtering
- **Live events** demand real-time viewer counts and chat at scale

## Scenarios

### 1. Content Catalog & Users - PostgreSQL
**File**: [`sql/01_schema_media.sql`](sql/01_schema_media.sql)

Relational schema for content management:
- **Users**: User accounts and profiles
- **Content**: Movies, series, episodes with metadata
- **Interactions**: Watch history with engagement metrics

**Use Case**: CMS integration, user management, content metadata

### 2. Real-time Streaming Operations - Redis
**File**: [`redis/01_streaming_ops.redis`](redis/01_streaming_ops.redis)

High-performance real-time operations using Redis data structures:
- **Sessions**: Active playback state with position tracking, heartbeats
- **Continue Watching**: Resume positions per content per user
- **Recommendations**: Personalized ML recommendations with scoring
- **Trending**: Global, regional, and category trending content
- **Content Cache**: Metadata, series episodes, thumbnails
- **Live Events**: Viewer counts, chat streams, DVR state
- **Notifications**: Pub/sub for new episodes, live starts, personalized alerts
- **Watchlist**: User-managed watchlist with timestamps
- **Preferences**: Language, quality, autoplay, content ratings
- **Subscriptions**: Plan entitlements, screen limits, feature flags
- **Ad Serving**: Targeted ad slots, impression tracking
- **QoS Metrics**: Buffering events, CDN health, bitrate distribution
- **Engagement**: Content views, completions, likes, shares
- **Search**: Autocomplete, user search history
- **Full-Text Search**: Content search (OrbitRS extension)
- **Vector Search**: Content similarity (OrbitRS extension)

**Use Case**: Player SDK, recommendation API, live streaming, personalization

```redis
# Track playback session
HSET session:user:U-12345 content_id "MOVIE-001" playback_position_sec 3542 quality "4K_HDR"

# Get personalized recommendations
ZREVRANGE recommendations:U-12345 0 9 WITHSCORES

# Track trending content
ZINCRBY trending:global:hourly 1 "MOVIE-001"

# Live event chat
XADD live:chat:EVENT-001 * user_id "U-12345" message "Amazing fight!"
```

### 3. Viewing Analytics & Time-Series - CQL (Cassandra)
**File**: [`cql/01_viewing_analytics.cql`](cql/01_viewing_analytics.cql)

Wide-column time-series storage for media analytics:
- **Viewing History**: Complete watch history per user with completion metrics (1 year)
- **Playback Events**: High-frequency telemetry (play, pause, seek, buffer, error) (7 days)
- **Content Metrics**: Daily aggregated engagement (views, completions, likes)
- **QoS Metrics**: Regional streaming quality (buffering, startup time, bitrate)
- **Live Event Analytics**: Minute-level concurrent viewers and engagement
- **User Behavior Profiles**: Aggregated preferences and patterns
- **Search Analytics**: Query performance and click-through
- **Ad Impressions**: Detailed ad delivery and engagement tracking
- **A/B Test Events**: Experiment results for UI/UX optimization
- **Content Embeddings**: Vector storage for similarity search (OrbitRS extension)
- **User Embeddings**: Collaborative filtering vectors (OrbitRS extension)

**Use Case**: Analytics dashboards, ML training, QoS monitoring, A/B testing

```cql
-- Query user's recent viewing history
SELECT view_time, title, completion_pct
FROM viewing_history
WHERE user_id = 'U-12345'
  AND view_month = '2024-12'
LIMIT 50;

-- Query QoS metrics by region
SELECT metric_hour, total_sessions, avg_buffer_ratio
FROM qos_metrics
WHERE region = 'US-WEST'
LIMIT 24;

-- Live event viewer count
SELECT metric_minute, concurrent_viewers
FROM live_event_metrics
WHERE event_id = 'EVENT-001';
```

### 4. ML Integration - Python
**Files**: [`python/01_run_ml_examples.py`](python/01_run_ml_examples.py), [`sql/02_ml_examples.sql`](sql/02_ml_examples.sql)

Machine learning integration for media applications:
- Content recommendations (collaborative + content-based)
- Churn prediction
- Ad targeting optimization

**Use Case**: Personalization, retention, monetization

## Multi-Protocol Integration Pattern

A typical streaming platform uses multiple protocols:

```
                    ┌─────────────────────────────────────────────────────────┐
                    │                      OrbitRS                            │
                    │                                                         │
  ┌─────────────┐   │   ┌────────────┐   ┌────────────┐   ┌────────────┐    │
  │   Player    │◄──┼──►│   Redis    │   │ PostgreSQL │   │    CQL     │    │
  │    SDK      │   │   │   :6379    │   │   :5432    │   │   :9042    │    │
  └─────────────┘   │   └─────┬──────┘   └─────┬──────┘   └─────┬──────┘    │
                    │         │                │                │           │
  ┌─────────────┐   │         │                │                │           │
  │    CMS      │◄──┼─────────┼────────────────┘                │           │
  │   System    │   │         │  (catalog, rights)              │           │
  └─────────────┘   │         │                                 │           │
                    │         │                                 │           │
  ┌─────────────┐   │         │    ┌────────────────────────────┘           │
  │  Analytics  │◄──┼─────────┼────┘  (viewing history, QoS)                │
  │  Dashboard  │   │         │                                             │
  └─────────────┘   │         ▼                                             │
                    │   ┌──────────────────────────────────────────────┐    │
                    │   │           Unified Storage Layer              │    │
                    │   │  (RocksDB + ML Embeddings + Time-Series)    │    │
                    │   └──────────────────────────────────────────────┘    │
                    └─────────────────────────────────────────────────────────┘
```

## Data Flow: Video Playback

```
1. User opens app        → Redis (session create)
                        → PostgreSQL (user lookup)

2. Home screen loads     → Redis (personalized recs, trending)
                        → Redis (continue watching)

3. Content selected      → Redis (content metadata cache)
                        → PostgreSQL (entitlement check)

4. Playback starts       → Redis (session update, position)
                        → CQL (playback event)

5. During playback       → Redis (heartbeat every 10s)
                        → CQL (telemetry events)

6. Playback ends         → CQL (viewing history record)
                        → Redis (continue watching update)
                        → CQL (engagement metrics)
```

## Performance Characteristics

| Operation | Protocol | Expected Latency |
|-----------|----------|-----------------|
| Get recommendations | Redis | < 5ms |
| Playback position update | Redis | < 1ms |
| Content metadata | Redis (cached) | < 2ms |
| Viewing history write | CQL | < 10ms |
| Telemetry event | CQL | < 5ms |
| Live viewer count | Redis | < 1ms |
| Search autocomplete | Redis | < 10ms |

## Scale Considerations

| Metric | Typical Scale |
|--------|--------------|
| Concurrent streams | 1M+ |
| Playback events/sec | 100K+ |
| Recommendation requests/sec | 50K+ |
| Live event viewers | 500K+ |
| Content catalog | 100K+ titles |

## ML Integration

### Content Recommendations
- **Collaborative Filtering**: User-item matrix factorization
- **Content-Based**: Content embeddings similarity (OrbitRS vector search)
- **Hybrid**: Weighted combination with contextual features
- **Real-time**: Redis-cached scores, refreshed hourly

### Churn Prediction
- **Features**: Watch frequency, completion rates, device diversity
- **Model**: XGBoost with 86% accuracy
- **Action**: Targeted retention campaigns

### Ad Targeting
- **CTR Prediction**: User segment + content context
- **Frequency Capping**: Redis counters
- **Revenue Optimization**: Real-time bidding integration

## Getting Started

1. Start OrbitRS with media configuration:
```bash
cargo run --bin orbit-server -- --config config/media.toml
```

2. Load the SQL schema:
```bash
psql -h localhost -p 5432 -f sql/01_schema_media.sql
```

3. Run Redis streaming examples:
```bash
redis-cli -p 6379 < redis/01_streaming_ops.redis
```

4. Load CQL analytics schema:
```bash
cqlsh localhost 9042 -f cql/01_viewing_analytics.cql
```

## Related Documentation

- [PostgreSQL Protocol](../../docs/content/protocols/postgresql.md)
- [Redis Protocol](../../docs/content/protocols/redis.md)
- [CQL Protocol](../../docs/content/protocols/cql.md)
- [Vector Search](../../docs/content/features/vector-search.md)
- [Time-Series Data](../../docs/content/features/time-series.md)
- [ML Integration](../../docs/content/features/ml-integration.md)
