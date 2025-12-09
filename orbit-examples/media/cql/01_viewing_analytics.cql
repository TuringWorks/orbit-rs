-- ============================================================================
-- OrbitRS Media Examples - Viewing Analytics & Time-Series (CQL)
-- ============================================================================
-- Wide-column time-series storage optimized for:
-- - Viewing history and playback events
-- - Content engagement metrics
-- - QoS monitoring and CDN analytics
-- - ML training data for recommendations
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS media_analytics
WITH replication = {'class': 'NetworkTopologyStrategy', 'dc1': 3}
AND durable_writes = true;

USE media_analytics;

-- ============================================================================
-- USER VIEWING HISTORY
-- ============================================================================
-- Partition by user + month for efficient history queries
-- Supports billions of events per day

CREATE TABLE viewing_history (
    user_id text,
    view_month text,  -- Format: '2024-12'
    view_time timestamp,
    view_id uuid,
    -- Content info
    content_id text,
    content_type text,  -- movie, series, live, short
    title text,
    season_episode text,  -- S01E05 for series
    -- Viewing details
    start_position_sec int,
    end_position_sec int,
    duration_sec int,
    total_duration_sec int,
    completion_pct double,
    -- Quality
    quality_profile text,  -- SD, HD, 4K, 4K_HDR
    avg_bitrate_kbps int,
    -- Device & context
    device_id text,
    device_type text,  -- smart_tv, mobile, tablet, web
    platform text,  -- roku, ios, android, web
    -- Session
    session_id text,
    PRIMARY KEY ((user_id, view_month), view_time, view_id)
) WITH CLUSTERING ORDER BY (view_time DESC, view_id ASC)
  AND default_time_to_live = 31536000  -- 1 year retention
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 1, 'compaction_window_unit': 'DAYS'}
  AND compression = {'sstable_compression': 'LZ4Compressor'};

-- Insert viewing record
INSERT INTO viewing_history (
    user_id, view_month, view_time, view_id,
    content_id, content_type, title, season_episode,
    start_position_sec, end_position_sec, duration_sec, total_duration_sec, completion_pct,
    quality_profile, avg_bitrate_kbps,
    device_id, device_type, platform, session_id
) VALUES (
    'U-12345', '2024-12', '2024-12-09 20:00:00', uuid(),
    'MOVIE-001', 'movie', 'The Matrix', null,
    0, 3600, 3600, 7200, 50.0,
    '4K_HDR', 25000,
    'DEV-ROKU-001', 'smart_tv', 'roku', 'sess_abc123'
);

-- Query: User's recent viewing history
SELECT view_time, title, completion_pct, device_type
FROM viewing_history
WHERE user_id = 'U-12345'
  AND view_month = '2024-12'
LIMIT 50;

-- Query: What did user watch on specific device?
SELECT view_time, title, duration_sec
FROM viewing_history
WHERE user_id = 'U-12345'
  AND view_month = '2024-12'
  AND device_type = 'smart_tv'
ALLOW FILTERING;

-- ============================================================================
-- PLAYBACK EVENTS (High-frequency telemetry)
-- ============================================================================
-- Captures every play, pause, seek, quality change, buffer event

CREATE TABLE playback_events (
    session_id text,
    event_date date,
    event_time timestamp,
    event_id uuid,
    -- Event details
    event_type text,  -- PLAY, PAUSE, SEEK, QUALITY_CHANGE, BUFFER_START, BUFFER_END, ERROR, COMPLETE
    position_sec int,
    -- Quality metrics
    quality_profile text,
    bitrate_kbps int,
    buffer_length_sec double,
    -- Network
    cdn_node text,
    client_ip text,
    -- Error details
    error_code text,
    error_message text,
    -- Context
    user_id text,
    content_id text,
    device_type text,
    PRIMARY KEY ((session_id, event_date), event_time, event_id)
) WITH CLUSTERING ORDER BY (event_time ASC, event_id ASC)
  AND default_time_to_live = 604800  -- 7 days retention
  AND compaction = {'class': 'TimeWindowCompactionStrategy', 'compaction_window_size': 1, 'compaction_window_unit': 'HOURS'};

-- Insert playback events
INSERT INTO playback_events (
    session_id, event_date, event_time, event_id,
    event_type, position_sec, quality_profile, bitrate_kbps, buffer_length_sec,
    user_id, content_id, device_type
) VALUES (
    'sess_abc123', '2024-12-09', '2024-12-09 20:00:00', uuid(),
    'PLAY', 0, '4K_HDR', 25000, 30.0,
    'U-12345', 'MOVIE-001', 'smart_tv'
);

INSERT INTO playback_events (
    session_id, event_date, event_time, event_id,
    event_type, position_sec, quality_profile, bitrate_kbps,
    user_id, content_id, device_type
) VALUES (
    'sess_abc123', '2024-12-09', '2024-12-09 20:30:00', uuid(),
    'BUFFER_START', 1800, 'HD', 8000,
    'U-12345', 'MOVIE-001', 'smart_tv'
);

-- Query: Session playback timeline
SELECT event_time, event_type, position_sec, quality_profile
FROM playback_events
WHERE session_id = 'sess_abc123'
  AND event_date = '2024-12-09';

-- ============================================================================
-- CONTENT ENGAGEMENT METRICS
-- ============================================================================
-- Daily aggregated engagement per content

CREATE TABLE content_daily_metrics (
    content_id text,
    metric_date date,
    -- Views
    total_views counter,
    unique_viewers counter,
    -- Completion
    completions counter,
    completion_75pct counter,
    completion_50pct counter,
    completion_25pct counter,
    PRIMARY KEY (content_id, metric_date)
) WITH CLUSTERING ORDER BY (metric_date DESC);

-- Increment counters
UPDATE content_daily_metrics
SET total_views = total_views + 1,
    unique_viewers = unique_viewers + 1
WHERE content_id = 'MOVIE-001'
  AND metric_date = '2024-12-09';

-- Query: Content performance
SELECT metric_date, total_views, completions
FROM content_daily_metrics
WHERE content_id = 'MOVIE-001'
LIMIT 30;

-- ============================================================================
-- QoS METRICS (CDN & Streaming Quality)
-- ============================================================================

CREATE TABLE qos_metrics (
    region text,
    metric_hour timestamp,  -- Rounded to hour
    -- Buffering
    total_sessions int,
    sessions_with_buffering int,
    avg_buffer_ratio double,
    avg_startup_time_ms int,
    -- Quality
    avg_bitrate_kbps int,
    quality_changes int,
    -- Errors
    error_count int,
    fatal_error_count int,
    -- Network
    avg_latency_ms int,
    cdn_cache_hit_rate double,
    PRIMARY KEY (region, metric_hour)
) WITH CLUSTERING ORDER BY (metric_hour DESC)
  AND default_time_to_live = 2592000;  -- 30 days

-- Insert QoS metrics
INSERT INTO qos_metrics (
    region, metric_hour,
    total_sessions, sessions_with_buffering, avg_buffer_ratio, avg_startup_time_ms,
    avg_bitrate_kbps, quality_changes,
    error_count, fatal_error_count,
    avg_latency_ms, cdn_cache_hit_rate
) VALUES (
    'US-WEST', '2024-12-09 20:00:00',
    125000, 5000, 0.02, 1500,
    15000, 12500,
    250, 10,
    15, 0.95
);

-- Query: Regional QoS
SELECT metric_hour, total_sessions, avg_buffer_ratio, avg_startup_time_ms
FROM qos_metrics
WHERE region = 'US-WEST'
LIMIT 24;

-- ============================================================================
-- LIVE EVENT ANALYTICS
-- ============================================================================

CREATE TABLE live_event_metrics (
    event_id text,
    metric_minute timestamp,  -- Minute-level granularity
    -- Viewers
    concurrent_viewers int,
    peak_viewers int,
    unique_viewers int,
    -- Engagement
    chat_messages int,
    reactions int,
    shares int,
    -- Quality
    avg_bitrate_kbps int,
    buffer_events int,
    -- DVR
    dvr_users int,
    avg_dvr_delay_sec int,
    PRIMARY KEY (event_id, metric_minute)
) WITH CLUSTERING ORDER BY (metric_minute DESC)
  AND default_time_to_live = 604800;  -- 7 days

-- Insert live metrics
INSERT INTO live_event_metrics (
    event_id, metric_minute,
    concurrent_viewers, peak_viewers, unique_viewers,
    chat_messages, reactions, shares,
    avg_bitrate_kbps, buffer_events
) VALUES (
    'EVENT-001', '2024-12-09 21:30:00',
    450000, 475000, 520000,
    12500, 8500, 250,
    12000, 1500
);

-- Query: Live event viewer timeline
SELECT metric_minute, concurrent_viewers, chat_messages
FROM live_event_metrics
WHERE event_id = 'EVENT-001';

-- ============================================================================
-- USER PREFERENCES & BEHAVIOR
-- ============================================================================

CREATE TABLE user_behavior_profile (
    user_id text,
    -- Viewing patterns
    preferred_genres list<text>,
    avg_session_duration_min int,
    avg_completion_rate double,
    -- Time patterns
    preferred_watch_hours list<int>,  -- 0-23
    preferred_days list<text>,  -- Mon, Tue, etc.
    -- Device usage
    primary_device text,
    devices_used list<text>,
    -- Content types
    movies_watched int,
    series_episodes_watched int,
    live_events_watched int,
    -- Engagement
    total_watch_hours int,
    last_active_date date,
    member_since date,
    PRIMARY KEY (user_id)
);

-- Insert behavior profile
INSERT INTO user_behavior_profile (
    user_id,
    preferred_genres, avg_session_duration_min, avg_completion_rate,
    preferred_watch_hours, preferred_days,
    primary_device, devices_used,
    movies_watched, series_episodes_watched, live_events_watched,
    total_watch_hours, last_active_date, member_since
) VALUES (
    'U-12345',
    ['action', 'sci-fi', 'thriller'], 90, 0.75,
    [19, 20, 21, 22], ['Fri', 'Sat', 'Sun'],
    'smart_tv', ['smart_tv', 'mobile', 'tablet'],
    125, 450, 15,
    850, '2024-12-09', '2022-01-15'
);

-- Query: User profile
SELECT preferred_genres, avg_completion_rate, total_watch_hours
FROM user_behavior_profile
WHERE user_id = 'U-12345';

-- ============================================================================
-- SEARCH & DISCOVERY ANALYTICS
-- ============================================================================

CREATE TABLE search_analytics (
    search_date date,
    search_time timestamp,
    search_id uuid,
    -- Query
    query_text text,
    query_normalized text,
    -- Results
    result_count int,
    results_clicked list<text>,
    result_played text,
    -- User
    user_id text,
    device_type text,
    -- Performance
    response_time_ms int,
    PRIMARY KEY (search_date, search_time, search_id)
) WITH CLUSTERING ORDER BY (search_time DESC, search_id ASC)
  AND default_time_to_live = 2592000;  -- 30 days

-- Insert search event
INSERT INTO search_analytics (
    search_date, search_time, search_id,
    query_text, query_normalized, result_count, results_clicked, result_played,
    user_id, device_type, response_time_ms
) VALUES (
    '2024-12-09', '2024-12-09 19:45:00', uuid(),
    'The Matrix', 'the matrix', 15, ['MOVIE-001', 'MOVIE-002'], 'MOVIE-001',
    'U-12345', 'smart_tv', 45
);

-- ============================================================================
-- AD IMPRESSIONS & PERFORMANCE
-- ============================================================================

CREATE TABLE ad_impressions (
    ad_id text,
    impression_date date,
    impression_time timestamp,
    impression_id uuid,
    -- Placement
    placement_type text,  -- PREROLL, MIDROLL, POSTROLL, BANNER
    content_id text,
    position_sec int,
    -- Targeting
    user_id text,
    user_segment text,
    -- Engagement
    view_duration_sec int,
    completed boolean,
    clicked boolean,
    skipped boolean,
    -- Revenue
    cpm_rate decimal,
    PRIMARY KEY ((ad_id, impression_date), impression_time, impression_id)
) WITH CLUSTERING ORDER BY (impression_time DESC, impression_id ASC)
  AND default_time_to_live = 7776000;  -- 90 days

-- Insert ad impression
INSERT INTO ad_impressions (
    ad_id, impression_date, impression_time, impression_id,
    placement_type, content_id, position_sec,
    user_id, user_segment,
    view_duration_sec, completed, clicked, skipped,
    cpm_rate
) VALUES (
    'AD-001', '2024-12-09', '2024-12-09 20:00:00', uuid(),
    'PREROLL', 'MOVIE-001', 0,
    'U-67890', 'sports_fans',
    30, true, false, false,
    15.50
);

-- ============================================================================
-- CONTENT EMBEDDINGS (OrbitRS Extension)
-- ============================================================================
-- Vector storage for content similarity and recommendations

CREATE TABLE content_embeddings (
    content_id text PRIMARY KEY,
    -- Embedding vectors
    embedding vector<float, 512>,
    -- Metadata for filtering
    content_type text,
    genres list<text>,
    release_year int,
    language text,
    -- Derived metrics
    avg_rating double,
    popularity_score double,
    updated_at timestamp
);

-- Insert content embedding
INSERT INTO content_embeddings (
    content_id,
    content_type, genres, release_year, language,
    avg_rating, popularity_score, updated_at
) VALUES (
    'MOVIE-001',
    'movie', ['action', 'sci-fi'], 1999, 'en',
    8.7, 0.95, toTimestamp(now())
);

-- ANN query for similar content (OrbitRS extension)
-- SELECT content_id, genres, avg_rating
-- FROM content_embeddings
-- ORDER BY embedding ANN OF [0.1, 0.2, ...]
-- LIMIT 20;

-- ============================================================================
-- USER EMBEDDINGS FOR COLLABORATIVE FILTERING
-- ============================================================================

CREATE TABLE user_embeddings (
    user_id text PRIMARY KEY,
    -- Embedding vector
    embedding vector<float, 256>,
    -- Profile summary
    subscription_tier text,
    preferred_genres list<text>,
    total_watch_hours int,
    member_since date,
    -- Model metadata
    model_version text,
    updated_at timestamp
);

-- ============================================================================
-- A/B TEST RESULTS
-- ============================================================================

CREATE TABLE ab_test_events (
    experiment_id text,
    event_date date,
    event_time timestamp,
    event_id uuid,
    -- Variant
    variant_id text,  -- control, treatment_a, treatment_b
    user_id text,
    -- Event
    event_type text,  -- IMPRESSION, CLICK, CONVERSION, ENGAGEMENT
    metric_value double,
    -- Context
    device_type text,
    user_segment text,
    PRIMARY KEY ((experiment_id, event_date), event_time, event_id)
) WITH CLUSTERING ORDER BY (event_time DESC, event_id ASC);

-- Insert A/B test event
INSERT INTO ab_test_events (
    experiment_id, event_date, event_time, event_id,
    variant_id, user_id, event_type, metric_value,
    device_type, user_segment
) VALUES (
    'EXP-REC-001', '2024-12-09', '2024-12-09 20:15:00', uuid(),
    'treatment_a', 'U-12345', 'CLICK', 1.0,
    'smart_tv', 'heavy_users'
);

-- Query: Experiment results
SELECT variant_id, event_type, count(*) as event_count
FROM ab_test_events
WHERE experiment_id = 'EXP-REC-001'
  AND event_date = '2024-12-09'
GROUP BY variant_id, event_type
ALLOW FILTERING;

