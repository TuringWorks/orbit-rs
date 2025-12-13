# Lua UDF Examples - Comprehensive Guide

This document provides practical, real-world examples of Lua UDFs in Orbit-RS across multiple use cases and protocols.

## Table of Contents

1. [Redis Scripting Examples](#redis-scripting-examples)
2. [PostgreSQL PL/Lua Examples](#postgresql-pllua-examples)
3. [Data Processing Pipelines](#data-processing-pipelines)
4. [Advanced Patterns](#advanced-patterns)
5. [Performance Optimization](#performance-optimization)
6. [Testing and Debugging](#testing-and-debugging)

---

## Redis Scripting Examples

### Example 1: Atomic Counter with Metadata

Track increments with timestamps and user information.

```lua
-- Script: atomic_counter_with_metadata.lua
local counter_key = KEYS[1]          -- 'counter:page_views'
local metadata_key = KEYS[2]         -- 'counter:page_views:meta'
local user_id = ARGV[1]              -- User who triggered increment
local increment = tonumber(ARGV[2]) or 1

-- Increment counter
local new_value = redis.call('INCRBY', counter_key, increment)

-- Store metadata
local timestamp = redis.call('TIME')[1]
redis.call('HSET', metadata_key,
    'last_user', user_id,
    'last_update', timestamp,
    'total_count', new_value
)

return {
    count = new_value,
    user = user_id,
    timestamp = timestamp
}
```

**Usage:**
```bash
EVAL "$(cat atomic_counter_with_metadata.lua)" 2 counter:page_views counter:page_views:meta user123 1
```

**Result:**
```json
{
  "count": 42,
  "user": "user123",
  "timestamp": 1702465789
}
```

---

### Example 2: Sliding Window Rate Limiter

Implement precise rate limiting with sliding window.

```lua
-- Script: sliding_window_rate_limiter.lua
local key = KEYS[1]                   -- 'rate_limit:api:user123'
local limit = tonumber(ARGV[1])       -- 100 (max requests)
local window = tonumber(ARGV[2])      -- 60 (seconds)
local now = tonumber(ARGV[3])         -- Current timestamp

-- Remove old entries outside the window
redis.call('ZREMRANGEBYSCORE', key, '-inf', now - window)

-- Count current entries
local current = redis.call('ZCARD', key)

if current < limit then
    -- Add current request
    redis.call('ZADD', key, now, now .. ':' .. math.random(1000000))
    -- Set expiration
    redis.call('EXPIRE', key, window)

    return {
        allowed = true,
        remaining = limit - current - 1,
        reset_in = window
    }
else
    -- Find when the oldest entry will expire
    local oldest = redis.call('ZRANGE', key, 0, 0, 'WITHSCORES')
    local reset_in = math.ceil(tonumber(oldest[2]) + window - now)

    return {
        allowed = false,
        remaining = 0,
        reset_in = reset_in
    }
end
```

**Usage:**
```bash
# Allow 100 requests per 60 seconds
EVAL "$(cat sliding_window_rate_limiter.lua)" 1 rate_limit:api:user123 100 60 $(date +%s)
```

**Result:**
```json
{
  "allowed": true,
  "remaining": 87,
  "reset_in": 60
}
```

---

### Example 3: Distributed Lock with Auto-Release

Acquire locks with automatic expiration and owner tracking.

```lua
-- Script: distributed_lock.lua
local lock_key = KEYS[1]              -- 'lock:resource:payment_123'
local client_id = ARGV[1]             -- Unique client identifier
local ttl = tonumber(ARGV[2])         -- 30 seconds

-- Try to acquire lock
local acquired = redis.call('SET', lock_key, client_id, 'NX', 'EX', ttl)

if acquired then
    return {
        status = 'acquired',
        client = client_id,
        ttl = ttl
    }
else
    -- Lock exists, check owner and TTL
    local owner = redis.call('GET', lock_key)
    local remaining = redis.call('TTL', lock_key)

    -- Check if we already own this lock
    if owner == client_id then
        -- Refresh TTL
        redis.call('EXPIRE', lock_key, ttl)
        return {
            status = 'refreshed',
            client = client_id,
            ttl = ttl
        }
    else
        return {
            status = 'denied',
            owner = owner,
            ttl_remaining = remaining
        }
    end
end
```

**Unlock Script:**
```lua
-- Script: unlock.lua
local lock_key = KEYS[1]
local client_id = ARGV[1]

local owner = redis.call('GET', lock_key)

if owner == client_id then
    redis.call('DEL', lock_key)
    return {status = 'released', client = client_id}
else
    return {status = 'not_owner', owner = owner}
end
```

**Usage:**
```bash
# Acquire lock
EVAL "$(cat distributed_lock.lua)" 1 lock:resource:payment_123 client_a1b2c3 30

# Release lock
EVAL "$(cat unlock.lua)" 1 lock:resource:payment_123 client_a1b2c3
```

---

### Example 4: Leaderboard with Score Tracking

Maintain sorted leaderboards with score history.

```lua
-- Script: leaderboard_update.lua
local leaderboard_key = KEYS[1]       -- 'leaderboard:global'
local history_key = KEYS[2]           -- 'leaderboard:global:history'
local player_id = ARGV[1]             -- 'player_12345'
local new_score = tonumber(ARGV[2])   -- New score

-- Update leaderboard
redis.call('ZADD', leaderboard_key, new_score, player_id)

-- Record in history with timestamp
local timestamp = redis.call('TIME')[1]
redis.call('ZADD', history_key .. ':' .. player_id, timestamp, new_score)

-- Keep only last 100 scores in history
redis.call('ZREMRANGEBYRANK', history_key .. ':' .. player_id, 0, -101)

-- Get player's rank
local rank = redis.call('ZREVRANK', leaderboard_key, player_id)

-- Get top 10
local top10 = redis.call('ZREVRANGE', leaderboard_key, 0, 9, 'WITHSCORES')

return {
    player = player_id,
    score = new_score,
    rank = rank + 1,  -- Ranks are 0-indexed, convert to 1-indexed
    timestamp = timestamp,
    top10 = top10
}
```

**Usage:**
```bash
EVAL "$(cat leaderboard_update.lua)" 2 leaderboard:global leaderboard:global:history player_12345 9750
```

---

### Example 5: Multi-Key Transaction

Atomic transfer between accounts with validation.

```lua
-- Script: atomic_transfer.lua
local from_key = KEYS[1]              -- 'account:123:balance'
local to_key = KEYS[2]                -- 'account:456:balance'
local amount = tonumber(ARGV[1])      -- 100.00

-- Validate amount
if amount <= 0 then
    return {success = false, error = 'Invalid amount'}
end

-- Get source balance
local from_balance = tonumber(redis.call('GET', from_key)) or 0

-- Check sufficient funds
if from_balance < amount then
    return {
        success = false,
        error = 'Insufficient funds',
        available = from_balance,
        required = amount
    }
end

-- Perform transfer
local new_from = from_balance - amount
redis.call('SET', from_key, new_from)

local to_balance = tonumber(redis.call('GET', to_key)) or 0
local new_to = to_balance + amount
redis.call('SET', to_key, new_to)

-- Log transaction
local tx_id = redis.call('INCR', 'transaction:counter')
local timestamp = redis.call('TIME')[1]
redis.call('HSET', 'transaction:' .. tx_id,
    'from', KEYS[1],
    'to', KEYS[2],
    'amount', amount,
    'timestamp', timestamp
)

return {
    success = true,
    transaction_id = tx_id,
    from_balance = new_from,
    to_balance = new_to,
    timestamp = timestamp
}
```

**Usage:**
```bash
EVAL "$(cat atomic_transfer.lua)" 2 account:123:balance account:456:balance 100.00
```

---

### Example 6: Session Management

Manage user sessions with automatic expiration and activity tracking.

```lua
-- Script: session_update.lua
local session_key = KEYS[1]           -- 'session:abc123xyz'
local user_id = ARGV[1]               -- User ID
local ttl = tonumber(ARGV[2])         -- 3600 (1 hour)
local activity = ARGV[3]              -- Activity description

local timestamp = redis.call('TIME')[1]

-- Check if session exists
local exists = redis.call('EXISTS', session_key)

if exists == 1 then
    -- Update existing session
    redis.call('HINCRBY', session_key, 'request_count', 1)
    redis.call('HSET', session_key,
        'last_activity', activity,
        'last_seen', timestamp
    )
    redis.call('EXPIRE', session_key, ttl)

    -- Get session data
    local session_data = redis.call('HGETALL', session_key)

    return {
        status = 'updated',
        session_data = session_data
    }
else
    -- Create new session
    redis.call('HSET', session_key,
        'user_id', user_id,
        'created_at', timestamp,
        'last_activity', activity,
        'last_seen', timestamp,
        'request_count', 1
    )
    redis.call('EXPIRE', session_key, ttl)

    return {
        status = 'created',
        session_id = KEYS[1],
        user_id = user_id
    }
end
```

**Usage:**
```bash
EVAL "$(cat session_update.lua)" 1 session:abc123xyz user_12345 3600 "view_dashboard"
```

---

### Example 7: Cache with Statistics

Track cache hits, misses, and usage patterns.

```lua
-- Script: cache_get_with_stats.lua
local cache_key = KEYS[1]             -- 'cache:user_profile:123'
local stats_key = KEYS[2]             -- 'cache:stats'

local value = redis.call('GET', cache_key)

if value then
    -- Cache hit
    redis.call('HINCRBY', stats_key, 'hits', 1)
    redis.call('HINCRBY', stats_key, 'total', 1)

    local ttl = redis.call('TTL', cache_key)

    return {
        status = 'hit',
        value = value,
        ttl = ttl
    }
else
    -- Cache miss
    redis.call('HINCRBY', stats_key, 'misses', 1)
    redis.call('HINCRBY', stats_key, 'total', 1)

    return {
        status = 'miss',
        value = nil
    }
end
```

**Get Cache Statistics:**
```lua
-- Script: cache_stats.lua
local stats_key = KEYS[1]

local stats = redis.call('HGETALL', stats_key)
local hits = tonumber(stats[2]) or 0
local misses = tonumber(stats[4]) or 0
local total = hits + misses

local hit_rate = 0
if total > 0 then
    hit_rate = (hits / total) * 100
end

return {
    hits = hits,
    misses = misses,
    total = total,
    hit_rate = hit_rate
}
```

---

## PostgreSQL PL/Lua Examples

### Example 8: Calculate Tax with Tiered Rates

```sql
CREATE FUNCTION calculate_progressive_tax(income DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE PLPGSQL
AS $$
DECLARE
    tax DOUBLE PRECISION := 0;
    remaining DOUBLE PRECISION := income;
BEGIN
    -- Tier 1: 0-10000 at 10%
    IF remaining > 10000 THEN
        tax := tax + (10000 * 0.10);
        remaining := remaining - 10000;
    ELSE
        tax := tax + (remaining * 0.10);
        remaining := 0;
    END IF;

    -- Tier 2: 10001-30000 at 20%
    IF remaining > 20000 THEN
        tax := tax + (20000 * 0.20);
        remaining := remaining - 20000;
    ELSIF remaining > 0 THEN
        tax := tax + (remaining * 0.20);
        remaining := 0;
    END IF;

    -- Tier 3: 30001+ at 30%
    IF remaining > 0 THEN
        tax := tax + (remaining * 0.30);
    END IF;

    RETURN tax;
END;
$$;

-- Usage
SELECT
    employee_id,
    salary,
    calculate_progressive_tax(salary) AS tax,
    salary - calculate_progressive_tax(salary) AS net_income
FROM employees;
```

---

### Example 9: JSON Data Transformation

```sql
CREATE FUNCTION transform_user_data(user_json JSONB)
RETURNS JSONB
LANGUAGE PLPGSQL
AS $$
DECLARE
    result JSONB;
BEGIN
    -- Transform user data
    SELECT jsonb_build_object(
        'id', user_json->>'id',
        'full_name', (user_json->>'first_name') || ' ' || (user_json->>'last_name'),
        'age', EXTRACT(YEAR FROM AGE(NOW(), (user_json->>'birthdate')::DATE)),
        'email_domain', SPLIT_PART(user_json->>'email', '@', 2),
        'created_at', user_json->>'created_at'
    ) INTO result;

    RETURN result;
END;
$$;

-- Usage
SELECT transform_user_data('{
    "id": "123",
    "first_name": "Alice",
    "last_name": "Johnson",
    "birthdate": "1990-05-15",
    "email": "alice@example.com",
    "created_at": "2023-01-01T00:00:00Z"
}'::JSONB);
```

---

### Example 10: Stored Procedure for Order Processing

```sql
CREATE OR REPLACE FUNCTION process_order(
    p_order_id INTEGER,
    p_customer_id INTEGER,
    p_items JSONB
)
RETURNS TABLE (
    success BOOLEAN,
    order_total DECIMAL(10,2),
    message TEXT
)
LANGUAGE PLPGSQL
AS $$
DECLARE
    v_total DECIMAL(10,2) := 0;
    v_item JSONB;
    v_product_id INTEGER;
    v_quantity INTEGER;
    v_price DECIMAL(10,2);
    v_available INTEGER;
BEGIN
    -- Start transaction logic
    FOR v_item IN SELECT * FROM jsonb_array_elements(p_items)
    LOOP
        v_product_id := (v_item->>'product_id')::INTEGER;
        v_quantity := (v_item->>'quantity')::INTEGER;

        -- Check inventory
        SELECT stock_quantity, price
        INTO v_available, v_price
        FROM products
        WHERE id = v_product_id;

        IF v_available < v_quantity THEN
            success := FALSE;
            order_total := 0;
            message := 'Insufficient stock for product ' || v_product_id;
            RETURN NEXT;
            RETURN;
        END IF;

        -- Update inventory
        UPDATE products
        SET stock_quantity = stock_quantity - v_quantity
        WHERE id = v_product_id;

        -- Calculate total
        v_total := v_total + (v_price * v_quantity);

        -- Insert order item
        INSERT INTO order_items (order_id, product_id, quantity, price)
        VALUES (p_order_id, v_product_id, v_quantity, v_price);
    END LOOP;

    -- Update order
    UPDATE orders
    SET total_amount = v_total, status = 'processed'
    WHERE id = p_order_id;

    success := TRUE;
    order_total := v_total;
    message := 'Order processed successfully';
    RETURN NEXT;
END;
$$;

-- Usage
SELECT * FROM process_order(
    1001,
    5,
    '[
        {"product_id": 10, "quantity": 2},
        {"product_id": 15, "quantity": 1}
    ]'::JSONB
);
```

---

## Data Processing Pipelines

### Example 11: ETL Pipeline with Error Handling

```lua
-- Script: etl_pipeline.lua
-- Extract data from source, transform, and load into target

local source_pattern = KEYS[1]        -- 'source:data:*'
local target_prefix = KEYS[2]         -- 'target:processed:'
local batch_size = tonumber(ARGV[1]) or 100

-- Find source keys
local source_keys = redis.call('KEYS', source_pattern)

local processed = 0
local failed = 0
local errors = {}

for i, source_key in ipairs(source_keys) do
    if processed >= batch_size then
        break
    end

    -- Extract
    local data = redis.call('GET', source_key)

    if data then
        -- Transform (example: uppercase)
        local transformed = string.upper(data)

        -- Load
        local target_key = target_prefix .. string.match(source_key, "([^:]+)$")

        local success, err = pcall(function()
            redis.call('SET', target_key, transformed)
            -- Mark as processed
            redis.call('SADD', 'processed:keys', source_key)
            -- Delete source
            redis.call('DEL', source_key)
        end)

        if success then
            processed = processed + 1
        else
            failed = failed + 1
            table.insert(errors, {key = source_key, error = tostring(err)})
        end
    end
end

return {
    processed = processed,
    failed = failed,
    total_found = #source_keys,
    errors = errors
}
```

---

### Example 12: Data Aggregation

```lua
-- Script: daily_aggregation.lua
-- Aggregate hourly data into daily summaries

local hourly_pattern = KEYS[1]        -- 'metrics:hourly:2024-12-13:*'
local daily_key = KEYS[2]             -- 'metrics:daily:2024-12-13'

local hourly_keys = redis.call('KEYS', hourly_pattern)

local total_requests = 0
local total_errors = 0
local total_latency = 0
local count = 0

for i, key in ipairs(hourly_keys) do
    local data = redis.call('HGETALL', key)

    -- Parse hash data
    for j = 1, #data, 2 do
        local field = data[j]
        local value = tonumber(data[j + 1])

        if field == 'requests' then
            total_requests = total_requests + value
        elseif field == 'errors' then
            total_errors = total_errors + value
        elseif field == 'latency' then
            total_latency = total_latency + value
            count = count + 1
        end
    end
end

-- Store daily aggregate
local avg_latency = 0
if count > 0 then
    avg_latency = total_latency / count
end

redis.call('HSET', daily_key,
    'total_requests', total_requests,
    'total_errors', total_errors,
    'avg_latency', avg_latency,
    'error_rate', (total_errors / total_requests) * 100
)

-- Set expiration (keep for 365 days)
redis.call('EXPIRE', daily_key, 365 * 24 * 3600)

return {
    daily_key = daily_key,
    total_requests = total_requests,
    total_errors = total_errors,
    avg_latency = avg_latency,
    hours_processed = #hourly_keys
}
```

---

## Advanced Patterns

### Example 13: Pub/Sub with Message Filtering

```lua
-- Script: filtered_publish.lua
local channel_prefix = ARGV[1]         -- 'notifications:'
local message = ARGV[2]                -- Message content
local filter_key = ARGV[3]             -- 'user:preferences'

-- Get subscriber preferences
local subscribers = redis.call('SMEMBERS', 'subscribers:all')

local delivered = 0

for i, subscriber in ipairs(subscribers) do
    local pref_key = filter_key .. ':' .. subscriber

    -- Check if subscriber wants this type of notification
    local wants_notification = redis.call('SISMEMBER', pref_key, ARGV[4])

    if wants_notification == 1 then
        local channel = channel_prefix .. subscriber
        redis.call('PUBLISH', channel, message)
        delivered = delivered + 1
    end
end

return {
    total_subscribers = #subscribers,
    delivered = delivered
}
```

---

### Example 14: Circuit Breaker Pattern

```lua
-- Script: circuit_breaker.lua
local service_key = KEYS[1]            -- 'circuit_breaker:payment_service'
local threshold = tonumber(ARGV[1])    -- 5 failures
local timeout = tonumber(ARGV[2])      -- 60 seconds

local state = redis.call('HGET', service_key, 'state') or 'CLOSED'
local failures = tonumber(redis.call('HGET', service_key, 'failures')) or 0

if state == 'OPEN' then
    -- Check if timeout has elapsed
    local opened_at = tonumber(redis.call('HGET', service_key, 'opened_at'))
    local now = redis.call('TIME')[1]

    if now - opened_at >= timeout then
        -- Try half-open
        redis.call('HSET', service_key, 'state', 'HALF_OPEN')
        return {state = 'HALF_OPEN', message = 'Circuit breaker trying recovery'}
    else
        return {state = 'OPEN', message = 'Circuit breaker is OPEN', failures = failures}
    end
elseif state == 'HALF_OPEN' then
    return {state = 'HALF_OPEN', message = 'Circuit breaker is testing'}
else
    -- CLOSED state
    if failures >= threshold then
        local now = redis.call('TIME')[1]
        redis.call('HSET', service_key,
            'state', 'OPEN',
            'opened_at', now
        )
        return {state = 'OPEN', message = 'Circuit breaker opened due to failures'}
    else
        return {state = 'CLOSED', message = 'Circuit breaker is CLOSED', failures = failures}
    end
end
```

**Record Success:**
```lua
-- Script: record_success.lua
local service_key = KEYS[1]

redis.call('HSET', service_key,
    'state', 'CLOSED',
    'failures', 0
)

return {state = 'CLOSED', message = 'Success recorded'}
```

**Record Failure:**
```lua
-- Script: record_failure.lua
local service_key = KEYS[1]

local failures = redis.call('HINCRBY', service_key, 'failures', 1)

return {failures = failures}
```

---

## Performance Optimization

### Example 15: Batch Operations

Instead of multiple individual calls:
```lua
-- ❌ SLOW: Multiple round-trips
for i=1,1000 do
    redis.call('SET', 'key' .. i, 'value' .. i)
end
```

Use batching:
```lua
-- ✅ FAST: Batched operations
local args = {}
for i=1,1000 do
    table.insert(args, 'key' .. i)
    table.insert(args, 'value' .. i)
end
redis.call('MSET', unpack(args))
```

---

### Example 16: Pipeline Pattern

```lua
-- Script: bulk_update_scores.lua
local leaderboard = KEYS[1]
local updates = cjson.decode(ARGV[1])  -- [{"player": "p1", "score": 100}, ...]

-- Batch all updates
local zadd_args = {leaderboard}

for i, update in ipairs(updates) do
    table.insert(zadd_args, update.score)
    table.insert(zadd_args, update.player)
end

-- Single ZADD with all updates
redis.call('ZADD', unpack(zadd_args))

-- Return top 10
return redis.call('ZREVRANGE', leaderboard, 0, 9, 'WITHSCORES')
```

---

## Testing and Debugging

### Example 17: Unit Testing Lua Scripts

```bash
#!/bin/bash
# test_scripts.sh

# Test rate limiter
echo "Testing rate limiter..."
for i in {1..10}; do
    redis-cli EVAL "$(cat sliding_window_rate_limiter.lua)" 1 "test:rate_limit" 5 10 $(date +%s)
done

# Test distributed lock
echo "Testing distributed lock..."
LOCK_RESULT=$(redis-cli EVAL "$(cat distributed_lock.lua)" 1 "test:lock" "client1" 30)
echo "Lock result: $LOCK_RESULT"

# Test unlock
UNLOCK_RESULT=$(redis-cli EVAL "$(cat unlock.lua)" 1 "test:lock" "client1")
echo "Unlock result: $UNLOCK_RESULT"

# Cleanup
redis-cli DEL "test:rate_limit" "test:lock"
```

---

### Example 18: Debugging with Logging

```lua
-- Enable logging in scripts
local function debug_log(message)
    redis.log(redis.LOG_WARNING, "DEBUG: " .. tostring(message))
end

local function process_data(data)
    debug_log("Processing data: " .. data)

    local result = data * 2

    debug_log("Result: " .. result)

    return result
end

-- Main execution
local input = tonumber(ARGV[1])
debug_log("Input received: " .. input)

local output = process_data(input)

return {
    input = input,
    output = output
}
```

---

### Example 19: Benchmarking Scripts

```lua
-- Script: benchmark.lua
local iterations = tonumber(ARGV[1]) or 1000

local start_time = redis.call('TIME')
local start_seconds = tonumber(start_time[1])
local start_microseconds = tonumber(start_time[2])

-- Operation to benchmark
for i=1,iterations do
    redis.call('SET', 'benchmark:key' .. i, 'value' .. i)
end

local end_time = redis.call('TIME')
local end_seconds = tonumber(end_time[1])
local end_microseconds = tonumber(end_time[2])

-- Calculate elapsed time
local elapsed_seconds = end_seconds - start_seconds
local elapsed_microseconds = end_microseconds - start_microseconds
local total_microseconds = (elapsed_seconds * 1000000) + elapsed_microseconds

return {
    iterations = iterations,
    elapsed_ms = total_microseconds / 1000,
    ops_per_second = (iterations / total_microseconds) * 1000000
}
```

**Usage:**
```bash
redis-cli EVAL "$(cat benchmark.lua)" 0 10000
```

---

## Best Practices Summary

### ✅ DO

1. **Use EVALSHA for repeated scripts** - Cache and reuse
2. **Validate inputs** - Check nil values and types
3. **Handle errors gracefully** - Use pcall() for safe execution
4. **Batch operations** - Minimize Redis calls
5. **Set appropriate TTLs** - Clean up temporary data
6. **Log important events** - Use redis.log() for debugging
7. **Keep scripts focused** - One responsibility per script
8. **Document parameters** - Comment KEYS and ARGV usage

### ❌ DON'T

1. **Don't use blocking operations** - Keep scripts fast
2. **Don't iterate large datasets** - Use SCAN instead of KEYS
3. **Don't modify global state** - Keep scripts pure
4. **Don't use random in deterministic contexts** - Can cause replication issues
5. **Don't hardcode values** - Use ARGV for parameters
6. **Don't ignore errors** - Always handle failure cases
7. **Don't create infinite loops** - Always have exit conditions
8. **Don't perform I/O operations** - Keep scripts CPU-bound

---

## Conclusion

These examples demonstrate the power and flexibility of Lua UDFs in Orbit-RS. Whether you're implementing Redis-compatible scripting, PostgreSQL stored procedures, or custom data processing pipelines, Lua provides a high-performance, sandboxed environment for extending database functionality.

For more information, see:
- [Lua UDF Complete Documentation](./LUA_UDF_COMPLETE_DOCUMENTATION.md)
- [Lua 5.4 Reference Manual](https://www.lua.org/manual/5.4/)
- [Redis Lua Scripting Guide](https://redis.io/docs/manual/programmability/eval-intro/)

---

**Last Updated**: December 13, 2025
**Version**: 1.0
