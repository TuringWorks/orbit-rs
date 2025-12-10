# Redis (RESP) Protocol Compatibility Specification

**Target**: Redis 7.x RESP3 Protocol + Redis Modules
**Reference**: https://redis.io/docs/reference/protocol-spec/
**Last Updated**: 2025-12-09
**Current Estimated Coverage**: ~65% (Core: ~75%, Modules: ~30%)

---

## Overview

This document specifies the Redis RESP (REdis Serialization Protocol) feature set and tracks OrbitRS implementation status. OrbitRS implements both RESP2 and RESP3 protocols for Redis compatibility.

## Table of Contents

1. [Commands](#commands)
2. [Data Structures](#data-structures)
3. [Protocol Features](#protocol-features)
4. [Implementation Status](#implementation-status)

---

## Commands

### Legend
- ✅ **Implemented** - Fully functional
- 🔶 **Partial** - Basic support, missing features
- ❌ **Not Implemented** - Not yet available

### String Commands

| Command | Status | Notes |
|---------|--------|-------|
| GET | ✅ | Get value |
| SET | ✅ | Set value with NX/XX/EX/PX |
| MGET | ✅ | Multiple get |
| MSET | ✅ | Multiple set |
| APPEND | ✅ | Append to string |
| INCR | ✅ | Increment |
| DECR | ✅ | Decrement |
| INCRBY | ✅ | Increment by |
| DECRBY | ✅ | Decrement by |
| INCRBYFLOAT | ✅ | Increment by float |
| STRLEN | ✅ | String length |
| GETRANGE | ✅ | Get substring |
| SETRANGE | ✅ | Set substring |
| GETSET | ✅ | Get and set |
| SETNX | ✅ | Set if not exists |
| SETEX | ✅ | Set with expiry |
| PSETEX | ✅ | Set with ms expiry |
| GETEX | ✅ | Get with expiry |
| GETDEL | ✅ | Get and delete |

### List Commands

| Command | Status | Notes |
|---------|--------|-------|
| LPUSH | ✅ | Push left |
| RPUSH | ✅ | Push right |
| LPOP | ✅ | Pop left |
| RPOP | ✅ | Pop right |
| LLEN | ✅ | List length |
| LRANGE | ✅ | Get range |
| LINDEX | ✅ | Get by index |
| LSET | ✅ | Set by index |
| LINSERT | ✅ | Insert element |
| LREM | ✅ | Remove elements |
| LTRIM | ✅ | Trim list |
| BLPOP | ✅ | Blocking pop left |
| BRPOP | ✅ | Blocking pop right |
| RPOPLPUSH | ✅ | Pop and push |
| BRPOPLPUSH | ✅ | Blocking pop/push |
| LMOVE | ✅ | Move element |
| BLMOVE | ✅ | Blocking move |
| LPOS | ✅ | Find position |

### Set Commands

| Command | Status | Notes |
|---------|--------|-------|
| SADD | ✅ | Add members |
| SREM | ✅ | Remove members |
| SMEMBERS | ✅ | Get all members |
| SISMEMBER | ✅ | Check membership |
| SCARD | ✅ | Set cardinality |
| SPOP | ✅ | Pop random |
| SRANDMEMBER | ✅ | Random member |
| SMOVE | ✅ | Move member |
| SUNION | ✅ | Union sets |
| SINTER | ✅ | Intersect sets |
| SDIFF | ✅ | Difference sets |
| SUNIONSTORE | ✅ | Union and store |
| SINTERSTORE | ✅ | Intersect and store |
| SDIFFSTORE | ✅ | Difference and store |
| SSCAN | ✅ | Scan set |

### Sorted Set Commands

| Command | Status | Notes |
|---------|--------|-------|
| ZADD | ✅ | Add members with scores |
| ZREM | ✅ | Remove members |
| ZSCORE | ✅ | Get score |
| ZINCRBY | ✅ | Increment score |
| ZCARD | ✅ | Set cardinality |
| ZRANGE | ✅ | Range by rank with WITHSCORES |
| ZCOUNT | ✅ | Count by score range |
| ZREVRANGE | ✅ | Reverse range with WITHSCORES |
| ZRANGEBYSCORE | ✅ | Range by score with WITHSCORES |
| ZREVRANGEBYSCORE | ✅ | Reverse range by score with WITHSCORES |
| ZRANK | ✅ | Get rank (0-based index) |
| ZREVRANK | ✅ | Get reverse rank |
| ZREMRANGEBYRANK | ✅ | Remove by rank range |
| ZREMRANGEBYSCORE | ✅ | Remove by score range |
| ZLEXCOUNT | ✅ | Count by lex range |
| ZPOPMIN | ✅ | Pop minimum score members |
| ZPOPMAX | ✅ | Pop maximum score members |
| ZSCAN | ✅ | Iterate sorted set |
| ZMSCORE | ✅ | Get multiple member scores |
| ZRANGEBYLEX | ❌ | Not implemented |
| ZREVRANGEBYLEX | ❌ | Not implemented |
| ZREMRANGEBYLEX | ❌ | Not implemented |
| ZUNION | ❌ | Not implemented |
| ZINTER | ❌ | Not implemented |
| ZDIFF | ❌ | Not implemented |
| ZUNIONSTORE | ❌ | Not implemented |
| ZINTERSTORE | ❌ | Not implemented |
| ZDIFFSTORE | ❌ | Not implemented |
| BZPOPMIN | ❌ | Blocking - not implemented |
| BZPOPMAX | ❌ | Blocking - not implemented |
| ZRANDMEMBER | ❌ | Not implemented |

### Hash Commands

| Command | Status | Notes |
|---------|--------|-------|
| HSET | ✅ | Set field |
| HGET | ✅ | Get field |
| HMSET | ✅ | Multiple set |
| HMGET | ✅ | Multiple get |
| HGETALL | ✅ | Get all fields |
| HDEL | ✅ | Delete fields |
| HEXISTS | ✅ | Field exists |
| HLEN | ✅ | Hash length |
| HKEYS | ✅ | Get keys |
| HVALS | ✅ | Get values |
| HINCRBY | ✅ | Increment by |
| HINCRBYFLOAT | ✅ | Increment by float |
| HSETNX | ✅ | Set if not exists |
| HSTRLEN | ✅ | Field length |
| HSCAN | ✅ | Scan hash |
| HRANDFIELD | ✅ | Random field |

### Key Commands

| Command | Status | Notes |
|---------|--------|-------|
| DEL | ✅ | Delete keys |
| EXISTS | ✅ | Check existence |
| EXPIRE | ✅ | Set expiry |
| EXPIREAT | ✅ | Set expiry at |
| PEXPIRE | ✅ | Set ms expiry |
| PEXPIREAT | ✅ | Set ms expiry at |
| TTL | ✅ | Get TTL |
| PTTL | ✅ | Get ms TTL |
| PERSIST | ✅ | Remove expiry |
| KEYS | ✅ | Find keys |
| SCAN | ✅ | Scan keys |
| RANDOMKEY | ✅ | Random key |
| RENAME | ✅ | Rename key |
| RENAMENX | ✅ | Rename if not exists |
| TYPE | ✅ | Get type |
| DUMP | ✅ | Serialize |
| RESTORE | ✅ | Deserialize |
| TOUCH | ✅ | Update access time |
| UNLINK | ✅ | Async delete |

### Transaction Commands

| Command | Status | Notes |
|---------|--------|-------|
| MULTI | ✅ | Start transaction |
| EXEC | ✅ | Execute transaction |
| DISCARD | ✅ | Discard transaction |
| WATCH | ✅ | Watch keys |
| UNWATCH | ✅ | Unwatch keys |

### Pub/Sub Commands

| Command | Status | Notes |
|---------|--------|-------|
| PUBLISH | 🔶 | Publish message |
| SUBSCRIBE | 🔶 | Subscribe to channels |
| UNSUBSCRIBE | 🔶 | Unsubscribe |
| PSUBSCRIBE | 🔶 | Pattern subscribe |
| PUNSUBSCRIBE | 🔶 | Pattern unsubscribe |
| PUBSUB | 🔶 | Pub/sub introspection |

### Server Commands

| Command | Status | Notes |
|---------|--------|-------|
| PING | ✅ | Ping server |
| ECHO | ✅ | Echo message |
| SELECT | ✅ | Select database |
| QUIT | ✅ | Close connection |
| INFO | ✅ | Server info |
| DBSIZE | ✅ | Database size |
| FLUSHDB | ✅ | Flush database |
| FLUSHALL | ✅ | Flush all databases |
| SAVE | 🔶 | Save to disk |
| BGSAVE | 🔶 | Background save |
| LASTSAVE | ✅ | Last save time |
| SHUTDOWN | ✅ | Shutdown server |
| CONFIG GET | ✅ | Get config |
| CONFIG SET | ✅ | Set config |
| CLIENT LIST | ✅ | List clients |
| CLIENT SETNAME | ✅ | Set client name |
| CLIENT GETNAME | ✅ | Get client name |
| TIME | ✅ | Server time |
| COMMAND | ✅ | Command info |
| COMMAND COUNT | ✅ | Command count |
| COMMAND INFO | ✅ | Command details |

---

## Data Structures

### Supported Types

| Type | Status | Notes |
|------|--------|-------|
| String | ✅ | Binary-safe strings |
| List | ✅ | Linked lists |
| Set | ✅ | Unordered sets |
| Sorted Set | ✅ | Scored sets |
| Hash | ✅ | Field-value maps |
| Bitmap | 🔶 | Bit operations |
| HyperLogLog | ❌ | Not implemented |
| Stream | ❌ | Not implemented |
| Geospatial | ❌ | Not implemented |

---

## Protocol Features

### RESP2 Protocol

| Feature | Status | Notes |
|---------|--------|-------|
| Simple Strings | ✅ | +OK |
| Errors | ✅ | -ERR |
| Integers | ✅ | :123 |
| Bulk Strings | ✅ | $6\r\nfoobar |
| Arrays | ✅ | *3\r\n... |
| Null | ✅ | $-1 |

### RESP3 Protocol

| Feature | Status | Notes |
|---------|--------|-------|
| Simple Strings | ✅ | +OK |
| Simple Errors | ✅ | -ERR |
| Integers | ✅ | :123 |
| Doubles | ✅ | ,1.23 |
| Booleans | ✅ | #t, #f |
| Bulk Strings | ✅ | $6\r\nfoobar |
| Bulk Errors | ✅ | !21\r\nERROR... |
| Verbatim Strings | ✅ | =15\r\ntxt:... |
| Arrays | ✅ | *3\r\n... |
| Maps | ✅ | %2\r\n... |
| Sets | ✅ | ~3\r\n... |
| Pushes | ✅ | >3\r\n... |
| Null | ✅ | _ |

### Connection Features

| Feature | Status | Notes |
|---------|--------|-------|
| Pipelining | ✅ | Full support |
| Blocking Commands | ✅ | BLPOP, BRPOP, etc. |
| Transactions | ✅ | MULTI/EXEC |
| Pub/Sub | 🔶 | Basic support |
| Client Tracking | ❌ | Not implemented |
| ACL | ❌ | Not implemented |

---

## Implementation Status

### Overall Coverage

| Category | Coverage | Notes |
|----------|----------|-------|
| String Commands | ~95% | Nearly complete |
| List Commands | ~100% | Full support |
| Set Commands | ~100% | Full support |
| Sorted Set Commands | ~61% | 19/31 commands implemented (core operations, ranking, score ranges, pop) |
| Hash Commands | ~100% | Full support |
| Key Commands | ~95% | Nearly complete |
| Transaction Commands | ~100% | Full support |
| Pub/Sub Commands | ~50% | Basic support |
| Server Commands | ~80% | Core commands work |
| RESP2 Protocol | ~100% | Full support |
| RESP3 Protocol | ~100% | Full support |

### Priority Roadmap

**High Priority**:
1. ✅ Core data structures (String, List, Set, Hash, ZSet)
2. ✅ RESP2/RESP3 protocols
3. ✅ Transactions
4. 🔶 Pub/Sub
5. ❌ Streams

**Medium Priority**:
1. ❌ HyperLogLog
2. ❌ Geospatial
3. ❌ Client tracking
4. ❌ ACL

**Low Priority**:
1. ❌ Cluster mode
2. ❌ Sentinel
3. ❌ Modules API

---

## Known Limitations

1. **Streams**: Not implemented
2. **HyperLogLog**: Not supported
3. **Geospatial**: Not implemented
4. **Client Tracking**: Not supported
5. **ACL**: Not implemented
6. **Cluster Mode**: Not supported
7. **Sentinel**: Not supported
8. **Modules**: Not supported
9. **Lua Scripting**: Not implemented
10. **Functions**: Not implemented

---

## Client Compatibility

### Tested Clients

| Client | Status | Notes |
|--------|--------|-------|
| redis-cli | ✅ | Full support |
| Python redis-py | ✅ | Full support |
| Node.js ioredis | ✅ | Full support |
| Node.js node-redis | ✅ | Full support |
| Go go-redis | ✅ | Full support |
| Java Jedis | ✅ | Full support |
| Java Lettuce | ✅ | Full support |

---

## Version Compatibility

| Redis Version | Compatibility | Notes |
|---------------|---------------|-------|
| Redis 6.x | ✅ | Full compatibility |
| Redis 7.x | ✅ | Target version |
| Valkey | ✅ | Compatible |
| KeyDB | ✅ | Compatible |

---

---

## Redis Modules

OrbitRS provides compatibility with popular Redis modules, enabling advanced functionality beyond core Redis commands.

### RedisGraph (Graph Database)

**Status**: 🔶 Partial Support (~40%)
**Reference**: https://redis.io/docs/stack/graph/

| Command | Status | Notes |
|---------|--------|-------|
| GRAPH.QUERY | 🔶 | Basic Cypher queries |
| GRAPH.RO_QUERY | 🔶 | Read-only queries |
| GRAPH.EXPLAIN | ❌ | Not implemented |
| GRAPH.PROFILE | ❌ | Not implemented |
| GRAPH.DELETE | 🔶 | Delete graph |
| GRAPH.SLOWLOG | ❌ | Not implemented |
| GRAPH.CONFIG GET | ❌ | Not implemented |
| GRAPH.CONFIG SET | ❌ | Not implemented |
| GRAPH.LIST | 🔶 | List graphs |

#### Cypher Query Support

| Feature | Status | Notes |
|---------|--------|-------|
| CREATE (nodes) | 🔶 | Basic node creation |
| CREATE (relationships) | 🔶 | Basic edge creation |
| MATCH | 🔶 | Pattern matching |
| WHERE | 🔶 | Filtering |
| RETURN | ✅ | Return results |
| SET | 🔶 | Update properties |
| DELETE | 🔶 | Delete nodes/edges |
| MERGE | ❌ | Not implemented |
| WITH | ❌ | Not implemented |
| UNWIND | ❌ | Not implemented |
| ORDER BY | 🔶 | Basic sorting |
| LIMIT/SKIP | ✅ | Pagination |
| Aggregations | 🔶 | COUNT, SUM, AVG |
| Path functions | ❌ | Not implemented |
| Shortest path | ❌ | Not implemented |

### RedisJSON (JSON Document Store)

**Status**: ✅ Full Support (~90%)
**Reference**: https://redis.io/docs/stack/json/

| Command | Status | Notes |
|---------|--------|-------|
| JSON.SET | ✅ | Set JSON value |
| JSON.GET | ✅ | Get JSON value |
| JSON.DEL | ✅ | Delete JSON path |
| JSON.MGET | ✅ | Multiple get |
| JSON.TYPE | ✅ | Get value type |
| JSON.NUMINCRBY | ✅ | Increment number |
| JSON.NUMMULTBY | ✅ | Multiply number |
| JSON.STRAPPEND | ✅ | Append to string |
| JSON.STRLEN | ✅ | String length |
| JSON.ARRAPPEND | ✅ | Append to array |
| JSON.ARRINDEX | ✅ | Find in array |
| JSON.ARRINSERT | ✅ | Insert into array |
| JSON.ARRLEN | ✅ | Array length |
| JSON.ARRPOP | ✅ | Pop from array |
| JSON.ARRTRIM | ✅ | Trim array |
| JSON.OBJKEYS | ✅ | Get object keys |
| JSON.OBJLEN | ✅ | Object length |
| JSON.TOGGLE | ✅ | Toggle boolean |
| JSON.CLEAR | ✅ | Clear value |
| JSON.DEBUG | 🔶 | Debug commands |
| JSON.RESP | ✅ | RESP encoding |

### RedisSearch (Full-Text Search & Secondary Indexing)

**Status**: 🔶 Partial Support (~50%)
**Reference**: https://redis.io/docs/stack/search/

| Command | Status | Notes |
|---------|--------|-------|
| FT.CREATE | ✅ | Create index |
| FT.SEARCH | ✅ | Search index |
| FT.AGGREGATE | 🔶 | Aggregation queries |
| FT.INFO | ✅ | Index info |
| FT.EXPLAIN | ❌ | Not implemented |
| FT.EXPLAINCLI | ❌ | Not implemented |
| FT.ALTER | ❌ | Not implemented |
| FT.DROPINDEX | ✅ | Drop index |
| FT.ALIASADD | ❌ | Not implemented |
| FT.ALIASDEL | ❌ | Not implemented |
| FT.ALIASUPDATE | ❌ | Not implemented |
| FT.TAGVALS | ❌ | Not implemented |
| FT.SUGADD | ❌ | Not implemented |
| FT.SUGGET | ❌ | Not implemented |
| FT.SUGDEL | ❌ | Not implemented |
| FT.SUGLEN | ❌ | Not implemented |
| FT.SYNUPDATE | ❌ | Not implemented |
| FT.SYNDUMP | ❌ | Not implemented |
| FT.SPELLCHECK | ❌ | Not implemented |
| FT.DICTADD | ❌ | Not implemented |
| FT.DICTDEL | ❌ | Not implemented |
| FT.DICTDUMP | ❌ | Not implemented |

#### Field Types

| Type | Status | Notes |
|------|--------|-------|
| TEXT | 🔶 | Full-text search |
| TAG | 🔶 | Exact match tags |
| NUMERIC | 🔶 | Numeric range |
| GEO | ❌ | Geospatial |
| VECTOR | ✅ | Vector similarity |

### RedisTimeSeries (Time Series Data)

**Status**: 🔶 Partial Support (~45%)
**Reference**: https://redis.io/docs/stack/timeseries/

| Command | Status | Notes |
|---------|--------|-------|
| TS.CREATE | 🔶 | Create time series |
| TS.ADD | 🔶 | Add sample |
| TS.MADD | 🔶 | Add multiple samples |
| TS.INCRBY | 🔶 | Increment value |
| TS.DECRBY | 🔶 | Decrement value |
| TS.CREATERULE | ❌ | Not implemented |
| TS.DELETERULE | ❌ | Not implemented |
| TS.RANGE | 🔶 | Query range |
| TS.REVRANGE | 🔶 | Reverse range |
| TS.MRANGE | 🔶 | Multi-key range |
| TS.MREVRANGE | 🔶 | Multi-key reverse |
| TS.GET | 🔶 | Get latest sample |
| TS.MGET | 🔶 | Multi-key get |
| TS.INFO | 🔶 | Series info |
| TS.QUERYINDEX | ❌ | Not implemented |
| TS.DEL | 🔶 | Delete range |
| TS.ALTER | ❌ | Not implemented |

#### Aggregation Functions

| Function | Status | Notes |
|----------|--------|-------|
| AVG | 🔶 | Average |
| SUM | 🔶 | Sum |
| MIN | 🔶 | Minimum |
| MAX | 🔶 | Maximum |
| RANGE | 🔶 | Range |
| COUNT | 🔶 | Count |
| FIRST | 🔶 | First value |
| LAST | 🔶 | Last value |
| STD.P | ❌ | Population stddev |
| STD.S | ❌ | Sample stddev |
| VAR.P | ❌ | Population variance |
| VAR.S | ❌ | Sample variance |
| TWA | ❌ | Time-weighted avg |

### RedisBloom (Probabilistic Data Structures)

**Status**: ❌ Not Implemented (~5%)
**Reference**: https://redis.io/docs/stack/bloom/

#### Bloom Filters

| Command | Status | Notes |
|---------|--------|-------|
| BF.RESERVE | ❌ | Create bloom filter |
| BF.ADD | ❌ | Add item |
| BF.MADD | ❌ | Add multiple items |
| BF.INSERT | ❌ | Insert with options |
| BF.EXISTS | ❌ | Check existence |
| BF.MEXISTS | ❌ | Check multiple |
| BF.SCANDUMP | ❌ | Dump filter |
| BF.LOADCHUNK | ❌ | Load chunk |
| BF.INFO | ❌ | Filter info |

#### Cuckoo Filters

| Command | Status | Notes |
|---------|--------|-------|
| CF.RESERVE | ❌ | Create cuckoo filter |
| CF.ADD | ❌ | Add item |
| CF.ADDNX | ❌ | Add if not exists |
| CF.INSERT | ❌ | Insert items |
| CF.INSERTNX | ❌ | Insert if not exists |
| CF.EXISTS | ❌ | Check existence |
| CF.DEL | ❌ | Delete item |
| CF.COUNT | ❌ | Count occurrences |
| CF.SCANDUMP | ❌ | Dump filter |
| CF.LOADCHUNK | ❌ | Load chunk |
| CF.INFO | ❌ | Filter info |

#### Count-Min Sketch

| Command | Status | Notes |
|---------|--------|-------|
| CMS.INITBYDIM | ❌ | Init by dimensions |
| CMS.INITBYPROB | ❌ | Init by probability |
| CMS.INCRBY | ❌ | Increment count |
| CMS.QUERY | ❌ | Query count |
| CMS.MERGE | ❌ | Merge sketches |
| CMS.INFO | ❌ | Sketch info |

#### Top-K

| Command | Status | Notes |
|---------|--------|-------|
| TOPK.RESERVE | ❌ | Create top-k |
| TOPK.ADD | ❌ | Add items |
| TOPK.INCRBY | ❌ | Increment items |
| TOPK.QUERY | ❌ | Query items |
| TOPK.COUNT | ❌ | Count items |
| TOPK.LIST | ❌ | List top items |
| TOPK.INFO | ❌ | Top-k info |

#### T-Digest

| Command | Status | Notes |
|---------|--------|-------|
| TDIGEST.CREATE | ❌ | Create t-digest |
| TDIGEST.RESET | ❌ | Reset digest |
| TDIGEST.ADD | ❌ | Add values |
| TDIGEST.MERGE | ❌ | Merge digests |
| TDIGEST.MIN | ❌ | Get minimum |
| TDIGEST.MAX | ❌ | Get maximum |
| TDIGEST.QUANTILE | ❌ | Get quantile |
| TDIGEST.CDF | ❌ | Cumulative distribution |
| TDIGEST.TRIMMED_MEAN | ❌ | Trimmed mean |
| TDIGEST.RANK | ❌ | Get rank |
| TDIGEST.REVRANK | ❌ | Reverse rank |
| TDIGEST.BYRANK | ❌ | Value by rank |
| TDIGEST.BYREVRANK | ❌ | Value by reverse rank |
| TDIGEST.INFO | ❌ | Digest info |

### RedisGears (Programmable Data Processing)

**Status**: ❌ Not Implemented
**Reference**: https://redis.io/docs/stack/gears/

| Command | Status | Notes |
|---------|--------|-------|
| RG.PYEXECUTE | ❌ | Execute Python |
| RG.ABORTEXECUTION | ❌ | Abort execution |
| RG.CONFIGGET | ❌ | Get config |
| RG.CONFIGSET | ❌ | Set config |
| RG.DUMPEXECUTIONS | ❌ | Dump executions |
| RG.DUMPREGISTRATIONS | ❌ | Dump registrations |
| RG.GETEXECUTION | ❌ | Get execution |
| RG.GETRESULTS | ❌ | Get results |
| RG.GETRESULTSBLOCKING | ❌ | Get results blocking |
| RG.INFOCLUSTER | ❌ | Cluster info |
| RG.PYSTATS | ❌ | Python stats |
| RG.PYDUMPREQS | ❌ | Dump requirements |
| RG.REFRESHCLUSTER | ❌ | Refresh cluster |
| RG.TRIGGER | ❌ | Trigger execution |
| RG.UNREGISTER | ❌ | Unregister function |

### RedisAI (Machine Learning)

**Status**: ❌ Not Implemented
**Reference**: https://redis.io/docs/stack/ai/

| Command | Status | Notes |
|---------|--------|-------|
| AI.TENSORSET | ❌ | Set tensor |
| AI.TENSORGET | ❌ | Get tensor |
| AI.MODELSTORE | ❌ | Store model |
| AI.MODELGET | ❌ | Get model |
| AI.MODELDEL | ❌ | Delete model |
| AI.MODELEXECUTE | ❌ | Execute model |
| AI.MODELSCAN | ❌ | Scan models |
| AI.SCRIPTSET | ❌ | Set script |
| AI.SCRIPTGET | ❌ | Get script |
| AI.SCRIPTDEL | ❌ | Delete script |
| AI.SCRIPTEXECUTE | ❌ | Execute script |
| AI.SCRIPTSCAN | ❌ | Scan scripts |
| AI.DAGEXECUTE | ❌ | Execute DAG |
| AI.DAGEXECUTE_RO | ❌ | Execute DAG read-only |
| AI.INFO | ❌ | AI info |
| AI.CONFIG | ❌ | AI config |

### Modules Implementation Summary

| Module | Coverage | Priority | Notes |
|--------|----------|----------|-------|
| RedisJSON | ~90% | ✅ High | Nearly complete |
| RedisGraph | ~40% | 🔶 Medium | Basic Cypher support |
| RedisSearch | ~85% | ✅ High | Core search complete |
| RedisTimeSeries | ~45% | 🔶 Medium | Basic time series |
| RedisBloom | ~5% | ❌ Low | Minimal support |
| RedisGears | 0% | ❌ Low | Not implemented |
| RedisAI | 0% | ❌ Low | Not implemented |

---

## References

- [Redis Commands](https://redis.io/commands/)
- [RESP Protocol Specification](https://redis.io/docs/reference/protocol-spec/)
- [RESP3 Specification](https://github.com/redis/redis-specifications/blob/master/protocol/RESP3.md)
