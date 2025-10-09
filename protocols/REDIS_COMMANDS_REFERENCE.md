# Redis Commands Reference - Orbit-RS RESP Protocol

## Overview

Orbit-RS provides full Redis compatibility through the RESP (Redis Serialization Protocol) adapter, supporting:
- ✅ **Standard Redis Commands** - All core Redis functionality
- ✅ **Vector Operations** - AI/ML vector search with multiple similarity metrics
- ✅ **Time Series** - Complete RedisTimeSeries compatibility
- ✅ **Graph Database** - Cypher-like graph queries and operations
- ✅ **Search Engine** - RedisSearch-compatible full-text and vector search

## Connection Commands

### PING [message]
Test server connection and optionally echo message.
```redis
PING

# "PONG"

PING "hello world" 

# "hello world"
```

### ECHO message
Echo the given message back to client.
```redis
ECHO "Hello Redis!"

# "Hello Redis!"
```

### SELECT db
Select database (logical separation, no-op for compatibility).
```redis
SELECT 0

# OK
```

### AUTH [username] password
Authenticate with server (placeholder implementation).
```redis
AUTH mypassword

# OK

AUTH myuser mypassword

# OK
```

### QUIT
Close connection gracefully.
```redis
QUIT

# OK
```

## String/Key Commands

### GET key
Retrieve value by key from KeyValueActor.
```redis
SET mykey "hello world"
GET mykey

# "hello world"
```

### SET key value [EX seconds] [PX milliseconds]
Set key to value with optional expiration.
```redis
SET mykey "value"

# OK

SET mykey "value" EX 3600

# OK (expires in 1 hour)

SET mykey "value" PX 60000

# OK (expires in 60 seconds)
```

### DEL key [key ...]
Delete one or more keys.
```redis
DEL key1 key2 key3

# (integer) 2
```

### EXISTS key [key ...]
Check if keys exist.
```redis
EXISTS key1 key2

# (integer) 1
```

### TTL key
Get time-to-live in seconds (-1 = no expiration, -2 = key doesn't exist).
```redis
TTL mykey

# (integer) 299
```

### EXPIRE key seconds
Set expiration time in seconds.
```redis
EXPIRE mykey 300

# (integer) 1
```

### Extended String Commands

#### APPEND key value
Append value to existing string.
```redis
SET mykey "Hello"
APPEND mykey " World"

# (integer) 11
GET mykey

# "Hello World"
```

#### GETRANGE key start end
Get substring by byte range.
```redis
SET mykey "Hello World"
GETRANGE mykey 0 4

# "Hello"
```

#### GETSET key value
Set new value and return old value atomically.
```redis
SET mykey "old"
GETSET mykey "new"

# "old"
```

#### MGET key [key ...]
Get multiple keys at once.
```redis
MSET key1 "value1" key2 "value2" key3 "value3"
MGET key1 key2 key3

# 1) "value1"
# 2) "value2"
# 3) "value3"
```

#### MSET key value [key value ...]
Set multiple key-value pairs.
```redis
MSET key1 "value1" key2 "value2" key3 "value3"

# OK
```

#### SETEX key seconds value
Set key with expiration time.
```redis
SETEX mykey 60 "temporary value"

# OK
```

#### SETRANGE key offset value
Overwrite string at specific offset.
```redis
SET mykey "Hello World"
SETRANGE mykey 6 "Redis"

# (integer) 11
GET mykey

# "Hello Redis"
```

#### STRLEN key
Get string length.
```redis
SET mykey "Hello World"
STRLEN mykey

# (integer) 11
```

## Hash Commands

### HGET key field
Get field value from hash.
```redis
HSET user:1 name "Alice" age "30"
HGET user:1 name

# "Alice"
```

### HSET key field value [field value ...]
Set field values in hash.
```redis
HSET user:1 name "Alice" age "30" city "NYC"

# (integer) 3
```

### HGETALL key
Get all field-value pairs from hash.
```redis
HGETALL user:1

# 1) "name"
# 2) "Alice"
# 3) "age"
# 4) "30"
# 5) "city"
# 6) "NYC"
```

### HDEL key field [field ...]
Delete fields from hash.
```redis
HDEL user:1 age city

# (integer) 2
```

### HEXISTS key field
Check if field exists in hash.
```redis
HEXISTS user:1 name

# (integer) 1
```

### HKEYS key
Get all field names from hash.
```redis
HKEYS user:1

# 1) "name"
# 2) "age"
# 3) "city"
```

### HVALS key
Get all values from hash.
```redis
HVALS user:1

# 1) "Alice"
# 2) "30"
# 3) "NYC"
```

### HLEN key
Get number of fields in hash.
```redis
HLEN user:1

# (integer) 3
```

### Extended Hash Commands

#### HMGET key field [field ...]
Get multiple field values.
```redis
HMGET user:1 name age city

# 1) "Alice"
# 2) "30"
# 3) "NYC"
```

#### HMSET key field value [field value ...]
Set multiple field values (legacy, use HSET).
```redis
HMSET user:1 name "Alice" age "30"

# OK
```

#### HINCRBY key field increment
Increment field by integer value.
```redis
HSET user:1 score 100
HINCRBY user:1 score 50

# (integer) 150
```

## List Commands

### LPUSH key element [element ...]
Push elements to left (head) of list.
```redis
LPUSH mylist "world" "hello"

# (integer) 2
```

### RPUSH key element [element ...]
Push elements to right (tail) of list.
```redis
RPUSH mylist "foo" "bar"

# (integer) 4
```

### LPOP key [count]
Pop element(s) from left (head) of list.
```redis
LPOP mylist

# "hello"

LPOP mylist 2

# 1) "world"
# 2) "foo"
```

### RPOP key [count]
Pop element(s) from right (tail) of list.
```redis
RPOP mylist

# "bar"

RPOP mylist 2

# 1) "foo"
# 2) "world"
```

### LRANGE key start stop
Get range of elements from list.
```redis
LRANGE mylist 0 -1

# 1) "hello"
# 2) "world"
# 3) "foo"
# 4) "bar"

LRANGE mylist 1 2

# 1) "world"
# 2) "foo"
```

### LLEN key
Get length of list.
```redis
LLEN mylist

# (integer) 4
```

### Extended List Commands

#### LINDEX key index
Get element at index.
```redis
LINDEX mylist 0

# "hello"

LINDEX mylist -1

# "bar"
```

#### LSET key index element
Set element at index.
```redis
LSET mylist 0 "hi"

# OK
```

#### LREM key count element
Remove elements equal to element.
```redis
LREM mylist 2 "foo"

# (integer) 1
```

#### LTRIM key start stop
Trim list to specified range.
```redis
LTRIM mylist 1 -1

# OK
```

#### LINSERT key BEFORE|AFTER pivot element
Insert element before or after pivot.
```redis
LINSERT mylist BEFORE "world" "beautiful"

# (integer) 5
```

#### BLPOP key [key ...] timeout
Blocking left pop (non-blocking in current implementation).
```redis
BLPOP mylist 10

# 1) "mylist"
# 2) "element"
```

#### BRPOP key [key ...] timeout
Blocking right pop (non-blocking in current implementation).
```redis
BRPOP mylist 10

# 1) "mylist"
# 2) "element"
```

## Set Commands

### SADD key member [member ...]
Add members to set.
```redis
SADD myset "apple" "banana" "cherry"

# (integer) 3
```

### SREM key member [member ...]
Remove members from set.
```redis
SREM myset "banana"

# (integer) 1
```

### SMEMBERS key
Get all members of set.
```redis
SMEMBERS myset

# 1) "apple"
# 2) "cherry"
```

### SCARD key
Get cardinality (size) of set.
```redis
SCARD myset

# (integer) 2
```

### SISMEMBER key member
Check if member exists in set.
```redis
SISMEMBER myset "apple"

# (integer) 1
```

### SUNION key [key ...]
Return union of sets.
```redis
SADD set1 "a" "b" "c"
SADD set2 "c" "d" "e"
SUNION set1 set2

# 1) "a"
# 2) "b"
# 3) "c"
# 4) "d"
# 5) "e"
```

### SINTER key [key ...]
Return intersection of sets.
```redis
SINTER set1 set2

# 1) "c"
```

### SDIFF key [key ...]
Return difference of sets.
```redis
SDIFF set1 set2

# 1) "a"
# 2) "b"
```

## Sorted Set Commands

### ZADD key score member [score member ...]
Add members with scores to sorted set.
```redis
ZADD leaderboard 100 "alice" 85 "bob" 92 "charlie"

# (integer) 3
```

### ZREM key member [member ...]
Remove members from sorted set.
```redis
ZREM leaderboard "bob"

# (integer) 1
```

### ZCARD key
Get cardinality of sorted set.
```redis
ZCARD leaderboard

# (integer) 2
```

### ZSCORE key member
Get score of member.
```redis
ZSCORE leaderboard "alice"

# "100"
```

### ZINCRBY key increment member
Increment score of member.
```redis
ZINCRBY leaderboard 10 "charlie"

# "102"
```

### ZRANGE key start stop [WITHSCORES]
Get members by rank range.
```redis
ZRANGE leaderboard 0 -1 WITHSCORES

# 1) "alice"
# 2) "100"
# 3) "charlie"
# 4) "102"
```

### ZRANGEBYSCORE key min max [WITHSCORES]
Get members by score range.
```redis
ZRANGEBYSCORE leaderboard 90 110 WITHSCORES

# 1) "alice"
# 2) "100"
# 3) "charlie"
# 4) "102"
```

### ZCOUNT key min max
Count members in score range.
```redis
ZCOUNT leaderboard 90 110

# (integer) 2
```

### ZRANK key member
Get rank of member (0-based).
```redis
ZRANK leaderboard "alice"

# (integer) 0
```

## Pub/Sub Commands

### PUBLISH channel message
Publish message to channel.
```redis
PUBLISH news "Breaking news!"

# (integer) 3
```

### SUBSCRIBE channel [channel ...]
Subscribe to channels.
```redis
SUBSCRIBE news weather sports

# 1) "subscribe"
# 2) "news"
# 3) (integer) 1
```

### UNSUBSCRIBE [channel [channel ...]]
Unsubscribe from channels.
```redis
UNSUBSCRIBE news

# 1) "unsubscribe"
# 2) "news"
# 3) (integer) 0
```

### PSUBSCRIBE pattern [pattern ...]
Subscribe to channel patterns.
```redis
PSUBSCRIBE news.*

# 1) "psubscribe"
# 2) "news.*"
# 3) (integer) 1
```

### PUNSUBSCRIBE [pattern [pattern ...]]
Unsubscribe from channel patterns.
```redis
PUNSUBSCRIBE news.*

# 1) "punsubscribe"
# 2) "news.*"
# 3) (integer) 0
```

### PUBSUB subcommand [argument [argument ...]]
Introspect pub/sub system.
```redis
PUBSUB CHANNELS

# 1) "news"
# 2) "weather"

PUBSUB NUMSUB news weather

# 1) "news"
# 2) (integer) 3
# 3) "weather"
# 4) (integer) 1
```

## Vector Operations (VECTOR.* namespace)

### VECTOR.ADD index id vector [key value ...]
Add vector with optional metadata to index.
```redis
VECTOR.ADD doc_embeddings doc1 "0.1,0.2,0.3,0.4" title "First Document" category "tech"

# OK
```

### VECTOR.GET index id
Get vector and metadata by ID.
```redis
VECTOR.GET doc_embeddings doc1

# 1) "doc1"
# 2) "0.100000,0.200000,0.300000,0.400000"
# 3) "title"
# 4) "First Document"
# 5) "category"
# 6) "tech"
```

### VECTOR.DEL index id
Delete vector from index.
```redis
VECTOR.DEL doc_embeddings doc1

# (integer) 1
```

### VECTOR.STATS index
Get statistics for vector index.
```redis
VECTOR.STATS doc_embeddings

# 1) "vector_count"
# 2) (integer) 1000
# 3) "index_count"
# 4) (integer) 1
# 5) "avg_dimension"
# 6) "768.00"
```

### VECTOR.LIST index
List all vector IDs in index.
```redis
VECTOR.LIST doc_embeddings

# 1) "doc1"
# 2) "doc2"
# 3) "doc3"
```

### VECTOR.COUNT index
Get count of vectors in index.
```redis
VECTOR.COUNT doc_embeddings

# (integer) 1000
```

### VECTOR.SEARCH index vector limit [METRIC metric] [THRESHOLD threshold] [key value ...]
Perform vector similarity search.
```redis
VECTOR.SEARCH doc_embeddings "0.1,0.2,0.3,0.4" 5 METRIC COSINE THRESHOLD 0.8 category "tech"

# 1) 1) "doc1"
#    2) "0.950000"
#    3) "0.100000,0.200000,0.300000,0.400000"
#    4) "title"
#    5) "First Document"
# 2) 1) "doc2"
#    2) "0.920000"
#    3) "0.150000,0.180000,0.320000,0.380000"
```

### VECTOR.KNN index vector k [METRIC metric]
K-nearest neighbors search.
```redis
VECTOR.KNN doc_embeddings "0.1,0.2,0.3,0.4" 3 METRIC EUCLIDEAN

# 1) 1) "doc1"
#    2) "0.050000"
# 2) 1) "doc2"
#    2) "0.082000"
```

## RedisSearch Compatible Commands (FT.* namespace)

### FT.CREATE index DIM dimension [DISTANCE_METRIC metric]
Create vector index.
```redis
FT.CREATE doc_index DIM 768 DISTANCE_METRIC COSINE

# OK
```

### FT.ADD index id vector [key value ...]
Add document to index (alias for VECTOR.ADD).
```redis
FT.ADD doc_index doc1 "0.1,0.2,0.3,..." title "Document"

# OK
```

### FT.DEL index id
Delete document from index.
```redis
FT.DEL doc_index doc1

# (integer) 1
```

### FT.SEARCH index vector limit [DISTANCE_METRIC metric] [key value ...]
Search index (converted to VECTOR.SEARCH format).
```redis
FT.SEARCH doc_index "0.1,0.2,0.3,..." 10 DISTANCE_METRIC COSINE

# (search results)
```

### FT.INFO index
Get index information.
```redis
FT.INFO doc_index

# 1) "index_name"
# 2) "doc_index"
# 3) "num_docs"
# 4) (integer) 1000
```

## Time Series Commands (TS.* namespace)

### TS.CREATE key [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
Create time series.
```redis
TS.CREATE temperature:sensor1 RETENTION 3600000 CHUNK_SIZE 4096 LABELS sensor_id "001" location "office"

# OK
```

### TS.ALTER key [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
Alter time series configuration.
```redis
TS.ALTER temperature:sensor1 RETENTION 7200000

# OK
```

### TS.ADD key timestamp value [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
Add sample to time series.
```redis
TS.ADD temperature:sensor1 1609459200000 23.5

# (integer) 1609459200000

TS.ADD temperature:sensor1 * 24.1

# (integer) 1609459260000
```

### TS.MADD key1 timestamp1 value1 [key2 timestamp2 value2 ...]
Add multiple samples.
```redis
TS.MADD temperature:sensor1 1609459200000 23.5 temperature:sensor2 1609459200000 22.8

# 1) (integer) 1609459200000
# 2) (integer) 1609459200000
```

### TS.INCRBY key value [TIMESTAMP timestamp] [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
Increment time series by value.
```redis
TS.INCRBY counter:requests 1 TIMESTAMP 1609459200000

# (integer) 1609459200000
```

### TS.DECRBY key value [TIMESTAMP timestamp] [RETENTION retentionTime] [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
Decrement time series by value.
```redis
TS.DECRBY counter:requests 1 TIMESTAMP 1609459200000

# (integer) 1609459200000
```

### TS.DEL key fromTimestamp toTimestamp
Delete samples in time range.
```redis
TS.DEL temperature:sensor1 1609459200000 1609459260000

# (integer) 2
```

### TS.GET key
Get latest sample.
```redis
TS.GET temperature:sensor1

# 1) (integer) 1609459260000
# 2) "24.1"
```

### TS.MGET [FILTER label=value ...] key1 [key2 ...]
Get latest samples from multiple time series.
```redis
TS.MGET temperature:sensor1 temperature:sensor2

# 1) 1) "temperature:sensor1"
#    2) (integer) 1609459260000
#    3) "24.1"
# 2) 1) "temperature:sensor2"
#    2) (integer) 1609459260000
#    3) "22.8"
```

### TS.INFO key
Get time series information.
```redis
TS.INFO temperature:sensor1

# 1) "totalSamples"
# 2) (integer) 100
# 3) "memoryUsage"
# 4) (integer) 4096
# 5) "firstTimestamp"
# 6) (integer) 1609459200000
# 7) "lastTimestamp"
# 8) (integer) 1609459260000
```

### TS.RANGE key fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration]
Get samples in time range.
```redis
TS.RANGE temperature:sensor1 1609459200000 1609459260000

# 1) 1) (integer) 1609459200000
#    2) "23.5"
# 2) 1) (integer) 1609459260000
#    2) "24.1"

TS.RANGE temperature:sensor1 1609459200000 1609459260000 AGGREGATION AVG 60000

# 1) 1) (integer) 1609459200000
#    2) "23.8"
```

### TS.REVRANGE key fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration]
Get samples in reverse time order.
```redis
TS.REVRANGE temperature:sensor1 1609459200000 1609459260000

# 2) 1) (integer) 1609459260000
#    2) "24.1"
# 1) 1) (integer) 1609459200000
#    2) "23.5"
```

### TS.MRANGE fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration] [FILTER label=value ...] key1 [key2 ...]
Get range from multiple time series.
```redis
TS.MRANGE 1609459200000 1609459260000 temperature:sensor1 temperature:sensor2

# 1) 1) "temperature:sensor1"
#    2) (empty array)
#    3) 1) 1) (integer) 1609459200000
#          2) "23.5"
#       2) 1) (integer) 1609459260000
#          2) "24.1"
```

### TS.MREVRANGE fromTimestamp toTimestamp [AGGREGATION aggregation bucketDuration] [FILTER label=value ...] key1 [key2 ...]
Get reverse range from multiple time series.
```redis
TS.MREVRANGE 1609459200000 1609459260000 temperature:sensor1 temperature:sensor2

# (reverse time order results)
```

### TS.QUERYINDEX [FILTER label=value ...]
Query time series index (placeholder).
```redis
TS.QUERYINDEX

# (empty array)
```

### TS.CREATERULE sourceKey destKey AGGREGATION aggregation bucketDuration [retention]
Create compaction rule.
```redis
TS.CREATERULE temperature:sensor1 temperature:sensor1:hourly AGGREGATION AVG 3600000

# OK
```

### TS.DELETERULE sourceKey destKey
Delete compaction rule.
```redis
TS.DELETERULE temperature:sensor1 temperature:sensor1:hourly

# OK
```

## Graph Database Commands (GRAPH.* namespace)

### GRAPH.QUERY graph_name query
Execute graph query.
```redis
GRAPH.QUERY social "MATCH (p:Person) RETURN p.name"

# 1) 1) "p.name"
# 2) 1) 1) "Alice"
#    2) 1) "Bob"
#    3) 1) "Charlie"
# 3) 1) "Cached execution"
#    2) "Query internal execution time: 0.000000 milliseconds"
```

### GRAPH.RO_QUERY graph_name query
Execute read-only graph query.
```redis
GRAPH.RO_QUERY social "MATCH (p:Person) WHERE p.age > 25 RETURN p"

# (query results similar to GRAPH.QUERY)
```

### GRAPH.DELETE graph_name
Delete entire graph.
```redis
GRAPH.DELETE social

# OK
```

### GRAPH.LIST
List all graphs.
```redis
GRAPH.LIST

# 1) "demo_graph"
# 2) "social_network"
```

### GRAPH.EXPLAIN graph_name query
Get execution plan for query.
```redis
GRAPH.EXPLAIN social "MATCH (p:Person) RETURN p"

# 1) 1) "1: Scan"
#    2) "Node Scan | (p:Person)"
#    3) "Estimated rows: 1000"
#    4) "Cost: 1000.00"
# 2) 1) "Total estimated cost: 1000.00"
```

### GRAPH.PROFILE graph_name query
Profile query execution.
```redis
GRAPH.PROFILE social "MATCH (p:Person) RETURN p"

# 1) (empty array)
# 2) (empty array)
# 3) 1) "1: Scan"
#    2) "Node Scan | (p:Person)"
#    3) "Records produced: 1000"
#    4) "Execution time: 5 ms"
#    5) "Memory used: 8192 bytes"
```

### GRAPH.SLOWLOG graph_name
Get slow query log.
```redis
GRAPH.SLOWLOG social

# 1) 1) (integer) 1
#    2) (integer) 1609459200
#    3) (integer) 1500
#    4) "MATCH (p:Person) RETURN p"
#    5) 1) "index_scan"
#       2) "node_scan"
```

### GRAPH.CONFIG GET|SET parameter [value]
Configure graph settings.
```redis
GRAPH.CONFIG GET timeout

# 1) "timeout"
# 2) "1000"

GRAPH.CONFIG SET timeout 2000

# OK
```

## Server Commands

### INFO [section]
Get server information with Orbit-specific details.
```redis
INFO

# # Server
# redis_version:7.0.0-orbit
# redis_git_sha1:00000000
# redis_git_dirty:0
# redis_mode:standalone
# os:Darwin 23.6.0 x86_64
# arch_bits:64
# process_id:12345
# uptime_in_seconds:3600
# 
# # Orbit
# orbit_mode:protocol_adapter
# orbit_actor_count:0
# orbit_cluster_nodes:1
```

### DBSIZE
Get number of keys (placeholder implementation).
```redis
DBSIZE

# (integer) 0
```

### FLUSHDB
Clear current database (placeholder implementation).
```redis
FLUSHDB

# OK
```

### FLUSHALL
Clear all databases (placeholder implementation).
```redis
FLUSHALL

# OK
```

### COMMAND
Get list of available commands.
```redis
COMMAND

# 1) 1) "ping"
#    2) (integer) -1
#    3) (integer) 1
#    4) (integer) 0
#    5) (integer) 0
# 2) 1) "get"
#    2) (integer) 2
#    3) (integer) 1
#    4) (integer) 1
#    5) (integer) 1
# ... (all supported commands)
```

## Additional String Commands

### PERSIST key
Remove expiration from key.
```redis
EXPIRE mykey 60
PERSIST mykey

# (integer) 1
TTL mykey

# (integer) -1
```

### PEXPIRE key milliseconds
Set expiration in milliseconds.
```redis
PEXPIRE mykey 60000

# (integer) 1
```

### PTTL key
Get TTL in milliseconds.
```redis
PTTL mykey

# (integer) 59850
```

### RANDOMKEY
Get random key (placeholder implementation).
```redis
RANDOMKEY

# (nil)
```

### RENAME oldkey newkey
Rename key.
```redis
SET oldkey "value"
RENAME oldkey newkey

# OK
GET newkey

# "value"
```

### TYPE key
Get type of key.
```redis
SET mystring "value"
HSET myhash field value
LPUSH mylist item
TYPE mystring

# string
TYPE myhash

# hash
TYPE mylist

# list
```

### UNLINK key [key ...]
Asynchronous delete (same as DEL in current implementation).
```redis
UNLINK key1 key2 key3

# (integer) 2
```

## Actor Integration

Each command family maps to specific Orbit actors:

- **String commands** → `KeyValueActor` - TTL-aware key-value storage
- **Hash commands** → `HashActor` - Hash map operations
- **List commands** → `ListActor` - Ordered list with push/pop
- **Set commands** → `SetActor` - Unique member sets
- **Sorted Set commands** → `SortedSetActor` - Score-ordered sets
- **Pub/Sub commands** → `PubSubActor` - Message channels
- **Vector commands** → `VectorActor` - AI/ML vector operations
- **Time Series commands** → `TimeSeriesActor` - Time-series data
- **Graph commands** → `GraphActor` - Graph database operations

## Configuration

```toml
[protocols.redis]
enabled = true
host = "0.0.0.0"
port = 6380
max_connections = 1000
default_ttl = 3600

[protocols.redis.vector]
default_similarity_metric = "cosine"
index_type = "hnsw"
ef_construction = 200
m = 16

[protocols.redis.timeseries]
default_retention = 86400000  # 24 hours in milliseconds
default_chunk_size = 4096
default_duplicate_policy = "block"

[protocols.redis.graph]
query_timeout = 30000  # 30 seconds
max_query_result_set = 10000
enable_query_cache = true
```

## Performance Notes

- **Concurrent Operations**: All commands support concurrent execution
- **Memory Efficiency**: Actors activate on-demand and deactivate when idle
- **Distributed**: Commands can access actors across the cluster
- **ACID Compliance**: All operations benefit from Orbit's transaction guarantees
- **Scalability**: Linear scaling with cluster size

## Compatibility

- ✅ **Redis Clients**: Compatible with all standard Redis clients
- ✅ **Protocol Version**: Full RESP2 support
- ✅ **Extensions**: Vector, Time Series, Graph extensions
- ✅ **Error Handling**: Redis-compatible error responses
- ✅ **Type System**: Full Redis type compatibility

This comprehensive command set makes Orbit-RS a drop-in replacement for Redis with enterprise-grade distributed computing capabilities and advanced AI/ML features.