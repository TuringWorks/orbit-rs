# Redis RESP Protocol Complete Specification

## Table of Contents

1. [Overview](#overview)
2. [Protocol Versions](#protocol-versions)
3. [Wire Protocol Fundamentals](#wire-protocol-fundamentals)
4. [RESP2 Data Types](#resp2-data-types)
5. [RESP3 Data Types](#resp3-data-types)
6. [Command Syntax](#command-syntax)
7. [Parser Implementation Guide](#parser-implementation-guide)
8. [AST Design](#ast-design)
9. [Rust Implementation Patterns](#rust-implementation-patterns)
10. [Redis Modules](#redis-modules)
11. [Error Handling](#error-handling)
12. [Connection Lifecycle](#connection-lifecycle)
13. [Pipelining and Transactions](#pipelining-and-transactions)
14. [Pub/Sub Protocol](#pubsub-protocol)
15. [Cluster Protocol Extensions](#cluster-protocol-extensions)
16. [Streams Protocol](#streams-protocol)
17. [Complete Command Reference](#complete-command-reference)
18. [Testing and Validation](#testing-and-validation)

---

## Overview

RESP (REdis Serialization Protocol) is a binary-safe, request-response protocol used for communication between Redis clients and servers. It is designed to be:

- **Simple to implement**: Human-readable format with clear delimiters
- **Fast to parse**: Prefixed lengths eliminate scanning
- **Binary-safe**: Supports arbitrary binary data
- **Backward compatible**: RESP3 is backward compatible with RESP2

### Key Characteristics

| Property | Value |
|----------|-------|
| Default Port | 6379 |
| TLS Port | 6380 (configurable) |
| Line Terminator | `\r\n` (CRLF) |
| Encoding | UTF-8 for strings, binary-safe for bulk strings |
| Max Inline Command | 64KB |
| Max Bulk String | 512MB (configurable) |

---

## Protocol Versions

### RESP2 (Redis 2.0+)

The original protocol version supporting five basic data types:

- Simple Strings
- Errors
- Integers
- Bulk Strings
- Arrays

### RESP3 (Redis 6.0+)

Extended protocol adding:

- Null type
- Boolean type
- Double type
- Big Number type
- Verbatim Strings
- Maps
- Sets
- Attributes
- Pushes

### Version Negotiation

```text
Client: HELLO 3
Server: %7\r\n
        $6\r\nserver\r\n$5\r\nredis\r\n
        $7\r\nversion\r\n$5\r\n7.2.0\r\n
        $5\r\nproto\r\n:3\r\n
        ...
```

---

## Wire Protocol Fundamentals

### Type Prefix Bytes

Every RESP message begins with a single byte indicating its type:

```text
RESP2 Types:
┌──────────────────┬────────┬─────────────────────────────────┐
│ Type             │ Prefix │ Description                     │
├──────────────────┼────────┼─────────────────────────────────┤
│ Simple String    │ +      │ Non-binary safe string          │
│ Error            │ -      │ Error message                   │
│ Integer          │ :      │ 64-bit signed integer           │
│ Bulk String      │ $      │ Binary-safe string with length  │
│ Array            │ *      │ Collection of RESP values       │
│ Null Bulk String │ $-1    │ Null value (RESP2)              │
│ Null Array       │ *-1    │ Null array (RESP2)              │
└──────────────────┴────────┴─────────────────────────────────┘

RESP3 Additional Types:
┌──────────────────┬────────┬─────────────────────────────────┐
│ Type             │ Prefix │ Description                     │
├──────────────────┼────────┼─────────────────────────────────┤
│ Null             │ _      │ Explicit null                   │
│ Boolean          │ #      │ True (#t) or False (#f)         │
│ Double           │ ,      │ Floating-point number           │
│ Big Number       │ (      │ Arbitrary precision integer     │
│ Bulk Error       │ !      │ Binary-safe error               │
│ Verbatim String  │ =      │ String with encoding hint       │
│ Map              │ %      │ Key-value pairs                 │
│ Set              │ ~      │ Unordered unique elements       │
│ Attribute        │ |      │ Out-of-band data                │
│ Push             │ >      │ Push message (Pub/Sub)          │
└──────────────────┴────────┴─────────────────────────────────┘
```

### CRLF Termination

All RESP types are terminated with `\r\n` (Carriage Return + Line Feed):

```text
+OK\r\n
:1000\r\n
$5\r\nhello\r\n
```

### Length Prefixing

Aggregate types use length prefixing to indicate element count:

```text
# Array with 3 elements
*3\r\n
$3\r\nSET\r\n
$3\r\nkey\r\n
$5\r\nvalue\r\n

# Bulk string with 5 bytes
$5\r\nhello\r\n
```

---

## RESP2 Data Types

### Simple String (+)

Simple strings are non-binary safe strings that cannot contain `\r` or `\n`.

```text
Format: +<string>\r\n

Examples:
+OK\r\n
+PONG\r\n
+QUEUED\r\n
```

**Wire Bytes (hex):**

```text
+OK\r\n = 2B 4F 4B 0D 0A
```

### Error (-)

Errors follow the format: `-<ERROR_TYPE> <message>\r\n`

```text
Format: -<error_type> <message>\r\n

Standard Error Types:
-ERR <message>           # Generic error
-WRONGTYPE <message>     # Type mismatch
-NOSCRIPT <message>      # Script not found
-BUSY <message>          # Server busy
-NOPROTO <message>       # Protocol error
-NOAUTH <message>        # Authentication required
-MOVED <slot> <addr>     # Cluster redirect
-ASK <slot> <addr>       # Cluster ask redirect
-TRYAGAIN <message>      # Retry later
-CLUSTERDOWN <message>   # Cluster unavailable
-CROSSSLOT <message>     # Multi-key cross-slot
-MASTERDOWN <message>    # Master unavailable
-READONLY <message>      # Replica read-only
-OOM <message>           # Out of memory
-EXECABORT <message>     # Transaction aborted
-NOREPLICAS <message>    # No replicas available
-NOTBUSY <message>       # Server not busy
-LOADING <message>       # Server loading dataset

Examples:
-ERR unknown command 'foobar'\r\n
-WRONGTYPE Operation against a key holding the wrong kind of value\r\n
-MOVED 3999 127.0.0.1:6381\r\n
```

### Integer (:)

64-bit signed integers (-2^63 to 2^63-1).

```text
Format: :<integer>\r\n

Examples:
:0\r\n
:1000\r\n
:-1\r\n
:9223372036854775807\r\n   # i64::MAX
:-9223372036854775808\r\n  # i64::MIN
```

### Bulk String ($)

Binary-safe strings with explicit length prefix.

```text
Format: $<length>\r\n<data>\r\n

Examples:
$0\r\n\r\n                 # Empty string
$5\r\nhello\r\n            # "hello"
$11\r\nhello\r\nworld\r\n  # "hello\r\nworld" (contains CRLF)
$-1\r\n                    # Null bulk string (RESP2)
```

**Binary Data Example:**

```text
$10\r\n\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\r\n
```

### Array (*)

Ordered collection of RESP values (can be heterogeneous).

```text
Format: *<count>\r\n<element1><element2>...

Examples:
*0\r\n                          # Empty array
*-1\r\n                         # Null array (RESP2)
*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n  # ["foo", "bar"]
*3\r\n:1\r\n:2\r\n:3\r\n          # [1, 2, 3]

# Mixed types
*5\r\n
:1\r\n
:2\r\n
:3\r\n
:4\r\n
$5\r\nhello\r\n
# Result: [1, 2, 3, 4, "hello"]

# Nested array
*2\r\n
*3\r\n:1\r\n:2\r\n:3\r\n
*2\r\n+Hello\r\n-World\r\n
# Result: [[1, 2, 3], ["Hello", Error("World")]]
```

---

## RESP3 Data Types

### Null (_)

Explicit null value (replaces `$-1` and `*-1`).

```text
Format: _\r\n

Wire: 5F 0D 0A
```

### Boolean (#)

Boolean true or false values.

```text
Format: #t\r\n or #f\r\n

Examples:
#t\r\n  # true
#f\r\n  # false
```

### Double (,)

IEEE 754 double-precision floating-point numbers.

```text
Format: ,<floating-point>\r\n

Examples:
,1.23\r\n
,-1.23\r\n
,1.23e10\r\n
,inf\r\n
,-inf\r\n
,nan\r\n
```

### Big Number (()

Arbitrary precision integers as strings.

```text
Format: (<big-integer>\r\n

Examples:
(3492890328409238509324850943850943825024385\r\n
(-3492890328409238509324850943850943825024385\r\n
```

### Bulk Error (!)

Binary-safe error messages with length prefix.

```text
Format: !<length>\r\n<error-data>\r\n

Example:
!21\r\nSYNTAX invalid syntax\r\n
```

### Verbatim String (=)

String with 3-byte encoding type prefix.

```text
Format: =<length>\r\n<encoding>:<data>\r\n

Encoding Types:
- txt: Plain text
- mkd: Markdown

Examples:
=15\r\ntxt:Hello World!\r\n
=22\r\nmkd:# Header\n\nBody\r\n
```

### Map (%)

Unordered collection of key-value pairs.

```text
Format: %<pair-count>\r\n<key1><value1><key2><value2>...

Example:
%2\r\n
$5\r\nfirst\r\n
:1\r\n
$6\r\nsecond\r\n
:2\r\n
# Result: {"first": 1, "second": 2}
```

### Set (~)

Unordered collection of unique elements.

```text
Format: ~<count>\r\n<element1><element2>...

Example:
~3\r\n
$5\r\napple\r\n
$6\r\norange\r\n
$4\r\npear\r\n
# Result: {"apple", "orange", "pear"}
```

### Attribute (|)

Out-of-band metadata attached to the next value.

```text
Format: |<pair-count>\r\n<key1><value1>...<actual-value>

Example:
|1\r\n
$8\r\nttl-hint\r\n
:3600\r\n
$5\r\nhello\r\n
# Result: "hello" with attribute {ttl-hint: 3600}
```

### Push (>)

Server-initiated push messages (Pub/Sub, client tracking).

```text
Format: ><count>\r\n<element1><element2>...

Example (Pub/Sub message):
>3\r\n
$7\r\nmessage\r\n
$7\r\nchannel\r\n
$7\r\npayload\r\n
```

---

## Command Syntax

### Inline Commands

Simple commands sent as plain text (for telnet debugging):

```text
PING\r\n
SET foo bar\r\n
GET foo\r\n
```

**Parsing Rules:**

**Parsing Rules:**

- Split by whitespace
- No binary safety
- Maximum 64KB
- Quotes can encapsulate strings with spaces

### RESP Commands (Standard)

Commands sent as RESP arrays:

```text
*1\r\n$4\r\nPING\r\n
*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n
```

### Command Structure

```text
Command = Array of Bulk Strings
        = *<argc>\r\n
          $<len>\r\n<command-name>\r\n
          [$<len>\r\n<arg>\r\n]...
```

### Common Command Patterns

```rust
// Key-Value operations
SET key value [EX seconds] [PX milliseconds] [EXAT timestamp] 
    [PXAT timestamp] [NX|XX] [KEEPTTL] [GET] [IFEQ value] [IFGT value]
GET key
DEL key [key ...]
EXISTS key [key ...]
EXPIRE key seconds [NX|XX|GT|LT]
TTL key
TYPE key

// String operations
APPEND key value
STRLEN key
GETRANGE key start end
SETRANGE key offset value
INCR key
INCRBY key increment
INCRBYFLOAT key increment
DECR key
DECRBY key decrement
MGET key [key ...]
MSET key value [key value ...]
MSETNX key value [key value ...]
SETNX key value
SETEX key seconds value
PSETEX key milliseconds value
GETSET key value
GETDEL key
GETEX key [EX seconds|PX milliseconds|EXAT timestamp|PXAT timestamp|PERSIST]

// Hash operations
HSET key field value [field value ...]
HGET key field
HMSET key field value [field value ...]
HMGET key field [field ...]
HGETALL key
HDEL key field [field ...]
HEXISTS key field
HLEN key
HKEYS key
HVALS key
HINCRBY key field increment
HINCRBYFLOAT key field increment
HSETNX key field value
HSCAN key cursor [MATCH pattern] [COUNT count]
HRANDFIELD key [count [WITHVALUES]]

// List operations
LPUSH key element [element ...]
RPUSH key element [element ...]
LPOP key [count]
RPOP key [count]
LRANGE key start stop
LLEN key
LINDEX key index
LSET key index element
LINSERT key BEFORE|AFTER pivot element
LREM key count element
LTRIM key start stop
BLPOP key [key ...] timeout
BRPOP key [key ...] timeout
BRPOPLPUSH source destination timeout
LMOVE source destination LEFT|RIGHT LEFT|RIGHT
BLMOVE source destination LEFT|RIGHT LEFT|RIGHT timeout
LPOS key element [RANK rank] [COUNT num-matches] [MAXLEN len]
LMPOP numkeys key [key ...] LEFT|RIGHT [COUNT count]
BLMPOP timeout numkeys key [key ...] LEFT|RIGHT [COUNT count]

// Set operations
SADD key member [member ...]
SREM key member [member ...]
SMEMBERS key
SISMEMBER key member
SMISMEMBER key member [member ...]
SCARD key
SPOP key [count]
SRANDMEMBER key [count]
SDIFF key [key ...]
SDIFFSTORE destination key [key ...]
SINTER key [key ...]
SINTERCARD numkeys key [key ...] [LIMIT limit]
SINTERSTORE destination key [key ...]
SUNION key [key ...]
SUNIONSTORE destination key [key ...]
SMOVE source destination member
SSCAN key cursor [MATCH pattern] [COUNT count]

// Sorted Set operations
ZADD key [NX|XX] [GT|LT] [CH] [INCR] score member [score member ...]
ZREM key member [member ...]
ZSCORE key member
ZRANK key member [WITHSCORE]
ZREVRANK key member [WITHSCORE]
ZRANGE key min max [BYSCORE|BYLEX] [REV] [LIMIT offset count] [WITHSCORES]
ZRANGESTORE dst src min max [BYSCORE|BYLEX] [REV] [LIMIT offset count]
ZRANGEBYSCORE key min max [WITHSCORES] [LIMIT offset count]
ZREVRANGEBYSCORE key max min [WITHSCORES] [LIMIT offset count]
ZRANGEBYLEX key min max [LIMIT offset count]
ZREVRANGEBYLEX key max min [LIMIT offset count]
ZCOUNT key min max
ZLEXCOUNT key min max
ZINCRBY key increment member
ZCARD key
ZPOPMIN key [count]
ZPOPMAX key [count]
BZPOPMIN key [key ...] timeout
BZPOPMAX key [key ...] timeout
ZMPOP numkeys key [key ...] MIN|MAX [COUNT count]
BZMPOP timeout numkeys key [key ...] MIN|MAX [COUNT count]
ZRANDMEMBER key [count [WITHSCORES]]
ZMSCORE key member [member ...]
ZUNION numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX] [WITHSCORES]
ZUNIONSTORE destination numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX]
ZINTER numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX] [WITHSCORES]
ZINTERCARD numkeys key [key ...] [LIMIT limit]
ZINTERSTORE destination numkeys key [key ...] [WEIGHTS weight ...] [AGGREGATE SUM|MIN|MAX]
ZDIFF numkeys key [key ...] [WITHSCORES]
ZDIFFSTORE destination numkeys key [key ...]
ZSCAN key cursor [MATCH pattern] [COUNT count]

// Pub/Sub
SUBSCRIBE channel [channel ...]
PSUBSCRIBE pattern [pattern ...]
UNSUBSCRIBE [channel [channel ...]]
PUNSUBSCRIBE [pattern [pattern ...]]
PUBLISH channel message
PUBSUB CHANNELS [pattern]
PUBSUB NUMSUB [channel [channel ...]]
PUBSUB NUMPAT
SSUBSCRIBE shardchannel [shardchannel ...]
SUNSUBSCRIBE [shardchannel [shardchannel ...]]
SPUBLISH shardchannel message

// Transactions
MULTI
EXEC
DISCARD
WATCH key [key ...]
UNWATCH

// Scripting
EVAL script numkeys [key ...] [arg ...]
EVALSHA sha1 numkeys [key ...] [arg ...]
EVALSHA_RO sha1 numkeys [key ...] [arg ...]
EVAL_RO script numkeys [key ...] [arg ...]
SCRIPT LOAD script
SCRIPT EXISTS sha1 [sha1 ...]
SCRIPT FLUSH [ASYNC|SYNC]
SCRIPT KILL
SCRIPT DEBUG YES|SYNC|NO
FCALL function numkeys [key ...] [arg ...]
FCALL_RO function numkeys [key ...] [arg ...]
FUNCTION LOAD [REPLACE] function-code
FUNCTION DELETE library-name
FUNCTION FLUSH [ASYNC|SYNC]
FUNCTION KILL
FUNCTION LIST [LIBRARYNAME library-name] [WITHCODE]
FUNCTION STATS
FUNCTION RESTORE serialized-value [FLUSH|APPEND|REPLACE]
FUNCTION DUMP

// Server
PING [message]
ECHO message
QUIT
SELECT index
AUTH [username] password
CLIENT GETNAME
CLIENT SETNAME connection-name
CLIENT ID
CLIENT INFO
CLIENT LIST [TYPE normal|master|replica|pubsub] [ID client-id ...]
CLIENT KILL [ID client-id] [TYPE normal|master|slave|replica|pubsub] [USER username] [ADDR ip:port] [LADDR ip:port] [SKIPME yes|no]
CLIENT PAUSE timeout [WRITE|ALL]
CLIENT UNPAUSE
CLIENT REPLY ON|OFF|SKIP
CLIENT CACHING YES|NO
CLIENT TRACKINGINFO
CLIENT GETREDIR
CLIENT NO-EVICT ON|OFF
CLIENT SETINFO LIB-NAME libname
CLIENT SETINFO LIB-VER libver
DBSIZE
TIME
LASTSAVE
BGSAVE [SCHEDULE]
BGREWRITEAOF
SAVE
SHUTDOWN [NOSAVE|SAVE] [NOW] [FORCE] [ABORT]
INFO [section ...]
CONFIG GET parameter [parameter ...]
CONFIG SET parameter value [parameter value ...]
CONFIG RESETSTAT
CONFIG REWRITE
SLOWLOG GET [count]
SLOWLOG LEN
SLOWLOG RESET
DEBUG OBJECT key
DEBUG SEGFAULT
DEBUG SLEEP seconds
FLUSHDB [ASYNC|SYNC]
FLUSHALL [ASYNC|SYNC]
KEYS pattern
SCAN cursor [MATCH pattern] [COUNT count] [TYPE type]
RANDOMKEY
RENAME key newkey
RENAMENX key newkey
COPY source destination [DB destination-db] [REPLACE]
MOVE key db
DUMP key
RESTORE key ttl serialized-value [REPLACE] [ABSTTL] [IDLETIME seconds] [FREQ frequency]
OBJECT ENCODING key
OBJECT FREQ key
OBJECT HELP
OBJECT IDLETIME key
OBJECT REFCOUNT key
TOUCH key [key ...]
UNLINK key [key ...]
WAIT numreplicas timeout
WAITAOF numlocal numreplicas timeout
MIGRATE host port key|"" destination-db timeout [COPY] [REPLACE] [AUTH password] [AUTH2 username password] [KEYS key ...]
SORT key [BY pattern] [LIMIT offset count] [GET pattern ...] [ASC|DESC] [ALPHA] [STORE destination]
SORT_RO key [BY pattern] [LIMIT offset count] [GET pattern ...] [ASC|DESC] [ALPHA]

// Cluster
CLUSTER ADDSLOTS slot [slot ...]
CLUSTER ADDSLOTSRANGE start-slot end-slot [start-slot end-slot ...]
CLUSTER BUMPEPOCH
CLUSTER COUNT-FAILURE-REPORTS node-id
CLUSTER COUNTKEYSINSLOT slot
CLUSTER DELSLOTS slot [slot ...]
CLUSTER DELSLOTSRANGE start-slot end-slot [start-slot end-slot ...]
CLUSTER FAILOVER [FORCE|TAKEOVER]
CLUSTER FLUSHSLOTS
CLUSTER FORGET node-id
CLUSTER GETKEYSINSLOT slot count
CLUSTER INFO
CLUSTER KEYSLOT key
CLUSTER LINKS
CLUSTER MEET ip port [cluster-bus-port]
CLUSTER MYID
CLUSTER MYSHARDID
CLUSTER NODES
CLUSTER REPLICAS node-id
CLUSTER REPLICATE node-id
CLUSTER RESET [HARD|SOFT]
CLUSTER SAVECONFIG
CLUSTER SET-CONFIG-EPOCH epoch
CLUSTER SETSLOT slot IMPORTING|MIGRATING|NODE|STABLE [node-id]
CLUSTER SHARDS
CLUSTER SLAVES node-id
CLUSTER SLOTS

// Streams
XADD key [NOMKSTREAM] [MAXLEN|MINID [=|~] threshold [LIMIT count]] *|id field value [field value ...]
XREAD [COUNT count] [BLOCK milliseconds] STREAMS key [key ...] id [id ...]
XREADGROUP GROUP group consumer [COUNT count] [BLOCK milliseconds] [NOACK] STREAMS key [key ...] id [id ...]
XRANGE key start end [COUNT count]
XREVRANGE key end start [COUNT count]
XLEN key
XINFO CONSUMERS key group
XINFO GROUPS key
XINFO STREAM key [FULL [COUNT count]]
XINFO HELP
XGROUP CREATE key group id|$ [MKSTREAM] [ENTRIESREAD entries-read]
XGROUP SETID key group id|$ [ENTRIESREAD entries-read]
XGROUP DESTROY key group
XGROUP CREATECONSUMER key group consumer
XGROUP DELCONSUMER key group consumer
XACK key group id [id ...]
XCLAIM key group consumer min-idle-time id [id ...] [IDLE ms] [TIME unix-time-ms] [RETRYCOUNT count] [FORCE] [JUSTID] [LASTID id]
XAUTOCLAIM key group consumer min-idle-time start [COUNT count] [JUSTID]
XPENDING key group [[IDLE min-idle-time] start end count [consumer]]
XTRIM key MAXLEN|MINID [=|~] threshold [LIMIT count]
XDEL key id [id ...]
XSETID key last-id [ENTRIESREAD entries-read] [MAXDELETEDID max-deleted-id]

// HyperLogLog
PFADD key [element [element ...]]
PFCOUNT key [key ...]
PFMERGE destkey sourcekey [sourcekey ...]
PFDEBUG DECODE|ENCODING|TODENSE key

// Geospatial
GEOADD key [NX|XX] [CH] longitude latitude member [longitude latitude member ...]
GEODIST key member1 member2 [M|KM|FT|MI]
GEOHASH key member [member ...]
GEOPOS key member [member ...]
GEORADIUS key longitude latitude radius M|KM|FT|MI [WITHCOORD] [WITHDIST] [WITHHASH] [COUNT count [ANY]] [ASC|DESC] [STORE key] [STOREDIST key]
GEORADIUSBYMEMBER key member radius M|KM|FT|MI [WITHCOORD] [WITHDIST] [WITHHASH] [COUNT count [ANY]] [ASC|DESC] [STORE key] [STOREDIST key]
GEOSEARCH key FROMMEMBER member|FROMLONLAT longitude latitude BYRADIUS radius M|KM|FT|MI|BYBOX width height M|KM|FT|MI [ASC|DESC] [COUNT count [ANY]] [WITHCOORD] [WITHDIST] [WITHHASH]
GEOSEARCHSTORE destination source FROMMEMBER member|FROMLONLAT longitude latitude BYRADIUS radius M|KM|FT|MI|BYBOX width height M|KM|FT|MI [ASC|DESC] [COUNT count [ANY]] [STOREDIST]

// Bitmap
SETBIT key offset value
GETBIT key offset
BITCOUNT key [start end [BYTE|BIT]]
BITOP AND|OR|XOR|NOT destkey key [key ...]
BITPOS key bit [start [end [BYTE|BIT]]]
BITFIELD key [GET encoding offset|[OVERFLOW WRAP|SAT|FAIL] SET encoding offset value|INCRBY encoding offset increment ...]
BITFIELD_RO key [GET encoding offset ...]

// ACL (Access Control List)
ACL CAT [category]
ACL DELUSER username [username ...]
ACL DRYRUN username command [arg ...]
ACL GENPASS [bits]
ACL GETUSER username
ACL LIST
ACL LOAD
ACL LOG [count|RESET]
ACL SAVE
ACL SETUSER username [rule ...]
ACL USERS
ACL WHOAMI

// Memory
MEMORY DOCTOR
MEMORY HELP
MEMORY MALLOC-SIZE pointer
MEMORY PURGE
MEMORY STATS
MEMORY USAGE key [SAMPLES count]

// Latency
LATENCY DOCTOR
LATENCY GRAPH event
LATENCY HELP
LATENCY HISTOGRAM [command ...]
LATENCY HISTORY event
LATENCY LATEST
LATENCY RESET [event ...]

// Module
MODULE LIST
MODULE LOAD path [arg ...]
MODULE LOADEX path [CONFIG name value ...] [ARGS arg ...]
MODULE UNLOAD name

// HELLO (RESP3)
HELLO [protover [AUTH username password] [SETNAME clientname]]

// Client Tracking (RESP3)
CLIENT TRACKING ON|OFF [REDIRECT client-id] [PREFIX prefix ...] [BCAST] [OPTIN] [OPTOUT] [NOLOOP]
```

---

## Parser Implementation Guide

### State Machine Parser

```text
                    ┌────────────────────────────────────┐
                    │                                    │
                    v                                    │
    ┌───────────────────────────────┐                    │
    │         READ_TYPE             │                    │
    │  (read first byte of message) │                    │
    └───────────────────────────────┘                    │
                    │                                    │
        ┌───────────┼───────────┬─────────────┐          │
        │           │           │             │          │
        v           v           v             v          │
    ┌───────┐  ┌────────┐  ┌────────┐   ┌──────────┐     │
    │  +/-  │  │   :    │  │   $    │   │    *     │     │
    │Simple │  │Integer │  │ Bulk   │   │  Array   │     │
    │String │  │        │  │ String │   │          │     │
    └───────┘  └────────┘  └────────┘   └──────────┘     │
        │           │           │             │          │
        v           v           │             │          │
    READ_UNTIL    PARSE         │             │          │
    CRLF          NUMBER        │             │          │
        │           │           │             │          │
        v           v           v             v          │
    ┌───────────────────────────────────────────┐        │
    │              VALUE_COMPLETE               │        │
    └───────────────────────────────────────────┘        │
                    │                                    │
                    │ (if nested in array, continue)     │
                    └────────────────────────────────────┘
```

### Lexer Tokens

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Type indicators
    SimpleString,      // +
    Error,             // -
    Integer,           // :
    BulkString,        // $
    Array,             // *
    Null,              // _ (RESP3)
    Boolean,           // # (RESP3)
    Double,            // , (RESP3)
    BigNumber,         // ( (RESP3)
    BulkError,         // ! (RESP3)
    VerbatimString,    // = (RESP3)
    Map,               // % (RESP3)
    Set,               // ~ (RESP3)
    Attribute,         // | (RESP3)
    Push,              // > (RESP3)
    
    // Data tokens
    Crlf,              // \r\n
    Number(i64),       // Parsed integer
    Data(Vec<u8>),     // Raw bytes
    
    // Special
    NullBulk,          // $-1
    NullArray,         // *-1
    Eof,
}
```

### Incremental Parsing

```rust
/// Parser state for incremental/streaming parsing
#[derive(Debug)]
pub enum ParseState {
    /// Waiting for type byte
    Initial,
    
    /// Reading simple string until CRLF
    ReadingSimpleString { buffer: Vec<u8> },
    
    /// Reading error until CRLF
    ReadingError { buffer: Vec<u8> },
    
    /// Reading integer until CRLF
    ReadingInteger { buffer: Vec<u8>, negative: bool },
    
    /// Reading bulk string length
    ReadingBulkLength { buffer: Vec<u8> },
    
    /// Reading bulk string data
    ReadingBulkData { remaining: usize, buffer: Vec<u8> },
    
    /// Waiting for CRLF after bulk data
    WaitingBulkCrlf { data: Vec<u8> },
    
    /// Reading array length
    ReadingArrayLength { buffer: Vec<u8> },
    
    /// Reading array elements
    ReadingArrayElements {
        remaining: usize,
        elements: Vec<RespValue>,
        nested_state: Box<ParseState>,
    },
    
    // RESP3 states
    ReadingMapLength { buffer: Vec<u8> },
    ReadingMapElements {
        remaining: usize,
        pairs: Vec<(RespValue, RespValue)>,
        key: Option<RespValue>,
        nested_state: Box<ParseState>,
    },
    ReadingSetLength { buffer: Vec<u8> },
    ReadingSetElements {
        remaining: usize,
        elements: Vec<RespValue>,
        nested_state: Box<ParseState>,
    },
    ReadingDouble { buffer: Vec<u8> },
    ReadingBigNumber { buffer: Vec<u8> },
    ReadingVerbatimLength { buffer: Vec<u8> },
    ReadingVerbatimData { remaining: usize, buffer: Vec<u8> },
    ReadingBoolean,
    
    /// Complete value ready
    Complete(RespValue),
    
    /// Parse error
    Error(ParseError),
}
```

---

## AST Design

### Core Value Type

```rust
/// RESP protocol value - represents any valid RESP data type
#[derive(Debug, Clone, PartialEq)]
pub enum RespValue {
    // RESP2 types
    SimpleString(String),
    Error(RespError),
    Integer(i64),
    BulkString(Option<Vec<u8>>),  // None = null bulk string
    Array(Option<Vec<RespValue>>),  // None = null array
    
    // RESP3 types
    Null,
    Boolean(bool),
    Double(f64),
    BigNumber(String),  // Stored as string for arbitrary precision
    BulkError(RespError),
    VerbatimString { encoding: String, data: Vec<u8> },
    Map(Vec<(RespValue, RespValue)>),
    Set(Vec<RespValue>),
    Attribute {
        attributes: Vec<(RespValue, RespValue)>,
        value: Box<RespValue>,
    },
    Push(Vec<RespValue>),
}

/// Error representation
#[derive(Debug, Clone, PartialEq)]
pub struct RespError {
    pub kind: ErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    Err,
    WrongType,
    NoScript,
    Busy,
    NoProto,
    NoAuth,
    Moved { slot: u16, addr: String },
    Ask { slot: u16, addr: String },
    TryAgain,
    ClusterDown,
    CrossSlot,
    MasterDown,
    ReadOnly,
    Oom,
    ExecAbort,
    NoReplicas,
    NotBusy,
    Loading,
    Custom(String),
}
```

### Command AST

```rust
/// Parsed Redis command
#[derive(Debug, Clone)]
pub struct Command {
    pub name: CommandName,
    pub args: Vec<Argument>,
    pub options: CommandOptions,
}

/// Command name enumeration (subset shown)
#[derive(Debug, Clone, PartialEq)]
pub enum CommandName {
    // String commands
    Set, Get, Append, GetRange, SetRange, StrLen,
    Incr, IncrBy, IncrByFloat, Decr, DecrBy,
    MGet, MSet, MSetNx, SetNx, SetEx, PSetEx,
    GetSet, GetDel, GetEx,
    
    // Key commands  
    Del, Exists, Expire, ExpireAt, ExpireTime,
    PExpire, PExpireAt, PExpireTime, Persist,
    Ttl, PTtl, Type, Rename, RenameNx,
    Keys, Scan, RandomKey, Touch, Unlink,
    Copy, Move, Dump, Restore, Object,
    
    // Hash commands
    HSet, HGet, HMSet, HMGet, HGetAll,
    HDel, HExists, HLen, HKeys, HVals,
    HIncrBy, HIncrByFloat, HSetNx, HScan, HRandField,
    
    // List commands
    LPush, RPush, LPop, RPop, LRange, LLen,
    LIndex, LSet, LInsert, LRem, LTrim,
    BLPop, BRPop, BRPopLPush, LMove, BLMove,
    LPos, LMPop, BLMPop,
    
    // Set commands
    SAdd, SRem, SMembers, SIsMember, SMIsMember,
    SCard, SPop, SRandMember, SDiff, SDiffStore,
    SInter, SInterCard, SInterStore, SUnion, SUnionStore,
    SMove, SScan,
    
    // Sorted set commands
    ZAdd, ZRem, ZScore, ZRank, ZRevRank,
    ZRange, ZRangeStore, ZRangeByScore, ZRevRangeByScore,
    ZRangeByLex, ZRevRangeByLex, ZCount, ZLexCount,
    ZIncrBy, ZCard, ZPopMin, ZPopMax, BZPopMin, BZPopMax,
    ZMPop, BZMPop, ZRandMember, ZMScore,
    ZUnion, ZUnionStore, ZInter, ZInterCard, ZInterStore,
    ZDiff, ZDiffStore, ZScan,
    
    // Pub/Sub
    Subscribe, PSubscribe, Unsubscribe, PUnsubscribe,
    Publish, PubSub, SSubscribe, SUnsubscribe, SPublish,
    
    // Transactions
    Multi, Exec, Discard, Watch, Unwatch,
    
    // Scripting
    Eval, EvalSha, EvalShaRo, EvalRo,
    ScriptLoad, ScriptExists, ScriptFlush, ScriptKill, ScriptDebug,
    FCall, FCallRo, FunctionLoad, FunctionDelete,
    FunctionFlush, FunctionKill, FunctionList,
    FunctionStats, FunctionRestore, FunctionDump,
    
    // Server
    Ping, Echo, Quit, Select, Auth,
    Client, DbSize, Time, LastSave,
    BgSave, BgRewriteAof, Save, Shutdown,
    Info, Config, SlowLog, Debug,
    FlushDb, FlushAll, Memory, Latency,
    Module, Acl, Hello,
    
    // Cluster
    Cluster,
    
    // Streams
    XAdd, XRead, XReadGroup, XRange, XRevRange,
    XLen, XInfo, XGroup, XAck, XClaim,
    XAutoClaim, XPending, XTrim, XDel, XSetId,
    
    // HyperLogLog
    PfAdd, PfCount, PfMerge, PfDebug,
    
    // Geo
    GeoAdd, GeoDist, GeoHash, GeoPos,
    GeoRadius, GeoRadiusByMember, GeoSearch, GeoSearchStore,
    
    // Bitmap
    SetBit, GetBit, BitCount, BitOp, BitPos, BitField, BitFieldRo,
    
    // Sentinel
    Sentinel,
    
    // Custom/Unknown
    Custom(String),
}

/// Command argument
#[derive(Debug, Clone)]
pub enum Argument {
    Key(Vec<u8>),
    Value(Vec<u8>),
    Integer(i64),
    Float(f64),
    Pattern(String),
    Field(Vec<u8>),
    Member(Vec<u8>),
    Score(f64),
    Channel(Vec<u8>),
    Message(Vec<u8>),
    Script(Vec<u8>),
    Sha1(String),
    StreamId(StreamId),
    Group(Vec<u8>),
    Consumer(Vec<u8>),
}

/// Command options/flags
#[derive(Debug, Clone, Default)]
pub struct CommandOptions {
    // SET options
    pub ex: Option<u64>,
    pub px: Option<u64>,
    pub exat: Option<u64>,
    pub pxat: Option<u64>,
    pub nx: bool,
    pub xx: bool,
    pub keepttl: bool,
    pub get: bool,
    pub ifeq: Option<Vec<u8>>,
    pub ifgt: Option<Vec<u8>>,
    
    // EXPIRE options
    pub gt: bool,
    pub lt: bool,
    
    // ZADD options
    pub ch: bool,
    pub incr: bool,
    
    // Sorting options
    pub by: Option<String>,
    pub get_patterns: Vec<String>,
    pub asc: bool,
    pub desc: bool,
    pub alpha: bool,
    pub limit: Option<(i64, i64)>,
    pub store: Option<Vec<u8>>,
    
    // SCAN options
    pub match_pattern: Option<String>,
    pub count: Option<u64>,
    pub scan_type: Option<String>,
    
    // XREAD/XREADGROUP options
    pub block: Option<u64>,
    pub noack: bool,
    
    // GEO options
    pub withcoord: bool,
    pub withdist: bool,
    pub withhash: bool,
    pub any: bool,
    pub storedist: Option<Vec<u8>>,
    
    // Generic
    pub timeout: Option<u64>,
    pub async_op: bool,
    pub sync_op: bool,
    pub replace: bool,
    pub absttl: bool,
    pub idletime: Option<u64>,
    pub freq: Option<u64>,
    pub left: bool,
    pub right: bool,
    pub withscores: bool,
    pub withvalues: bool,
    pub rev: bool,
    pub byscore: bool,
    pub bylex: bool,
    
    // AGGREGATE
    pub aggregate: Option<Aggregate>,
    pub weights: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aggregate {
    Sum,
    Min,
    Max,
}

/// Stream ID representation
#[derive(Debug, Clone, PartialEq)]
pub enum StreamId {
    Auto,                          // *
    Explicit(u64, u64),           // timestamp-sequence
    Minimum,                       // -
    Maximum,                       // +
    LastDelivered,                // >
    New,                          // $
}
```

### Response AST

```rust
/// Typed response for specific commands
#[derive(Debug, Clone)]
pub enum Response {
    Ok,
    Pong(Option<String>),
    Nil,
    Integer(i64),
    Float(f64),
    String(Vec<u8>),
    Strings(Vec<Option<Vec<u8>>>),
    KeyValue(Vec<u8>, Vec<u8>),
    KeyValues(Vec<(Vec<u8>, Vec<u8>)>),
    ScoredMembers(Vec<(Vec<u8>, f64)>),
    StreamEntries(Vec<StreamEntry>),
    ScanResult { cursor: u64, items: Vec<Vec<u8>> },
    HScanResult { cursor: u64, items: Vec<(Vec<u8>, Vec<u8>)> },
    ZScanResult { cursor: u64, items: Vec<(Vec<u8>, f64)> },
    Info(HashMap<String, HashMap<String, String>>),
    ClusterSlots(Vec<ClusterSlot>),
    ClusterNodes(Vec<ClusterNode>),
    PubSubMessage { channel: Vec<u8>, payload: Vec<u8> },
    Queued,
    ExecResult(Vec<Response>),
    Error(RespError),
}

#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub id: StreamId,
    pub fields: Vec<(Vec<u8>, Vec<u8>)>,
}

#[derive(Debug, Clone)]
pub struct ClusterSlot {
    pub start: u16,
    pub end: u16,
    pub master: ClusterNode,
    pub replicas: Vec<ClusterNode>,
}

#[derive(Debug, Clone)]
pub struct ClusterNode {
    pub id: String,
    pub ip: String,
    pub port: u16,
    pub flags: Vec<String>,
    pub master_id: Option<String>,
    pub ping_sent: u64,
    pub pong_recv: u64,
    pub config_epoch: u64,
    pub link_state: String,
    pub slots: Vec<(u16, u16)>,
}
```

---

## Rust Implementation Patterns

### Error Types

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RespError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    
    #[error("Redis error: {kind} - {message}")]
    Redis { kind: ErrorKind, message: String },
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Invalid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Unexpected end of input")]
    UnexpectedEof,
    
    #[error("Invalid type byte: {0:#04x}")]
    InvalidTypeByte(u8),
    
    #[error("Invalid integer: {0}")]
    InvalidInteger(String),
    
    #[error("Invalid bulk string length: {0}")]
    InvalidBulkLength(i64),
    
    #[error("Invalid array length: {0}")]
    InvalidArrayLength(i64),
    
    #[error("Missing CRLF")]
    MissingCrlf,
    
    #[error("Invalid CRLF")]
    InvalidCrlf,
    
    #[error("Line too long: {0} bytes")]
    LineTooLong(usize),
    
    #[error("Nesting too deep: {0}")]
    NestingTooDeep(usize),
    
    #[error("Invalid double: {0}")]
    InvalidDouble(String),
    
    #[error("Invalid boolean: expected 't' or 'f', got {0}")]
    InvalidBoolean(char),
    
    #[error("Invalid verbatim string encoding")]
    InvalidVerbatimEncoding,
}
```

### Parser Trait

```rust
use bytes::{Buf, BytesMut};

/// Result of a parse attempt
pub enum ParseResult<T> {
    /// Successfully parsed value
    Complete(T),
    /// Need more data (minimum bytes needed)
    Incomplete(usize),
    /// Parse error
    Error(ParseError),
}

/// RESP parser trait
pub trait RespParser {
    /// Parse a complete RESP value from the buffer
    fn parse(&mut self, buf: &mut BytesMut) -> ParseResult<RespValue>;
    
    /// Reset parser state
    fn reset(&mut self);
}

/// Streaming parser implementation
pub struct StreamingParser {
    state: ParseState,
    max_bulk_length: usize,
    max_array_depth: usize,
}

impl StreamingParser {
    pub fn new() -> Self {
        Self {
            state: ParseState::Initial,
            max_bulk_length: 512 * 1024 * 1024, // 512MB
            max_array_depth: 32,
        }
    }
    
    pub fn with_limits(max_bulk_length: usize, max_array_depth: usize) -> Self {
        Self {
            state: ParseState::Initial,
            max_bulk_length,
            max_array_depth,
        }
    }
}

impl RespParser for StreamingParser {
    fn parse(&mut self, buf: &mut BytesMut) -> ParseResult<RespValue> {
        loop {
            match &mut self.state {
                ParseState::Initial => {
                    if buf.is_empty() {
                        return ParseResult::Incomplete(1);
                    }
                    
                    let type_byte = buf[0];
                    buf.advance(1);
                    
                    self.state = match type_byte {
                        b'+' => ParseState::ReadingSimpleString { buffer: Vec::new() },
                        b'-' => ParseState::ReadingError { buffer: Vec::new() },
                        b':' => ParseState::ReadingInteger { buffer: Vec::new(), negative: false },
                        b'$' => ParseState::ReadingBulkLength { buffer: Vec::new() },
                        b'*' => ParseState::ReadingArrayLength { buffer: Vec::new() },
                        b'_' => {
                            // RESP3 Null
                            if buf.len() < 2 {
                                return ParseResult::Incomplete(2 - buf.len());
                            }
                            if &buf[..2] != b"\r\n" {
                                return ParseResult::Error(ParseError::InvalidCrlf);
                            }
                            buf.advance(2);
                            self.state = ParseState::Initial;
                            return ParseResult::Complete(RespValue::Null);
                        }
                        b'#' => ParseState::ReadingBoolean,
                        b',' => ParseState::ReadingDouble { buffer: Vec::new() },
                        b'(' => ParseState::ReadingBigNumber { buffer: Vec::new() },
                        b'!' => ParseState::ReadingBulkLength { buffer: Vec::new() }, // Bulk error
                        b'=' => ParseState::ReadingVerbatimLength { buffer: Vec::new() },
                        b'%' => ParseState::ReadingMapLength { buffer: Vec::new() },
                        b'~' => ParseState::ReadingSetLength { buffer: Vec::new() },
                        b'>' => ParseState::ReadingArrayLength { buffer: Vec::new() }, // Push
                        _ => return ParseResult::Error(ParseError::InvalidTypeByte(type_byte)),
                    };
                }
                
                ParseState::ReadingSimpleString { buffer } => {
                    match read_line(buf, buffer) {
                        LineResult::Complete(line) => {
                            let value = String::from_utf8(line)
                                .map_err(|e| ParseError::InvalidInteger(e.to_string()))?;
                            self.state = ParseState::Initial;
                            return ParseResult::Complete(RespValue::SimpleString(value));
                        }
                        LineResult::Incomplete(needed) => return ParseResult::Incomplete(needed),
                        LineResult::TooLong(len) => return ParseResult::Error(ParseError::LineTooLong(len)),
                    }
                }
                
                // ... other state handlers
                
                _ => todo!("Implement remaining states"),
            }
        }
    }
    
    fn reset(&mut self) {
        self.state = ParseState::Initial;
    }
}

enum LineResult {
    Complete(Vec<u8>),
    Incomplete(usize),
    TooLong(usize),
}

fn read_line(buf: &mut BytesMut, line_buf: &mut Vec<u8>) -> LineResult {
    const MAX_LINE: usize = 64 * 1024;
    
    while !buf.is_empty() {
        let byte = buf[0];
        buf.advance(1);
        
        if byte == b'\r' {
            if buf.is_empty() {
                line_buf.push(byte);
                return LineResult::Incomplete(1);
            }
            if buf[0] == b'\n' {
                buf.advance(1);
                return LineResult::Complete(std::mem::take(line_buf));
            }
            line_buf.push(byte);
        } else {
            line_buf.push(byte);
        }
        
        if line_buf.len() > MAX_LINE {
            return LineResult::TooLong(line_buf.len());
        }
    }
    
    LineResult::Incomplete(1)
}
```

### Encoder

```rust
use bytes::{BufMut, BytesMut};

/// RESP encoder trait
pub trait RespEncoder {
    fn encode(&self, buf: &mut BytesMut);
}

impl RespEncoder for RespValue {
    fn encode(&self, buf: &mut BytesMut) {
        match self {
            RespValue::SimpleString(s) => {
                buf.put_u8(b'+');
                buf.put_slice(s.as_bytes());
                buf.put_slice(b"\r\n");
            }
            
            RespValue::Error(err) => {
                buf.put_u8(b'-');
                buf.put_slice(err.kind.as_str().as_bytes());
                buf.put_u8(b' ');
                buf.put_slice(err.message.as_bytes());
                buf.put_slice(b"\r\n");
            }
            
            RespValue::Integer(n) => {
                buf.put_u8(b':');
                buf.put_slice(n.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
            
            RespValue::BulkString(Some(data)) => {
                buf.put_u8(b'$');
                buf.put_slice(data.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.put_slice(data);
                buf.put_slice(b"\r\n");
            }
            
            RespValue::BulkString(None) => {
                buf.put_slice(b"$-1\r\n");
            }
            
            RespValue::Array(Some(elements)) => {
                buf.put_u8(b'*');
                buf.put_slice(elements.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for element in elements {
                    element.encode(buf);
                }
            }
            
            RespValue::Array(None) => {
                buf.put_slice(b"*-1\r\n");
            }
            
            RespValue::Null => {
                buf.put_slice(b"_\r\n");
            }
            
            RespValue::Boolean(b) => {
                buf.put_u8(b'#');
                buf.put_u8(if *b { b't' } else { b'f' });
                buf.put_slice(b"\r\n");
            }
            
            RespValue::Double(d) => {
                buf.put_u8(b',');
                if d.is_infinite() {
                    if d.is_sign_positive() {
                        buf.put_slice(b"inf");
                    } else {
                        buf.put_slice(b"-inf");
                    }
                } else if d.is_nan() {
                    buf.put_slice(b"nan");
                } else {
                    buf.put_slice(d.to_string().as_bytes());
                }
                buf.put_slice(b"\r\n");
            }
            
            RespValue::BigNumber(n) => {
                buf.put_u8(b'(');
                buf.put_slice(n.as_bytes());
                buf.put_slice(b"\r\n");
            }
            
            RespValue::BulkError(err) => {
                let content = format!("{} {}", err.kind.as_str(), err.message);
                buf.put_u8(b'!');
                buf.put_slice(content.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.put_slice(content.as_bytes());
                buf.put_slice(b"\r\n");
            }
            
            RespValue::VerbatimString { encoding, data } => {
                // encoding:data format, encoding is 3 chars
                let total_len = 3 + 1 + data.len(); // encoding + ":" + data
                buf.put_u8(b'=');
                buf.put_slice(total_len.to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.put_slice(&encoding.as_bytes()[..3]);
                buf.put_u8(b':');
                buf.put_slice(data);
                buf.put_slice(b"\r\n");
            }
            
            RespValue::Map(pairs) => {
                buf.put_u8(b'%');
                buf.put_slice(pairs.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for (key, value) in pairs {
                    key.encode(buf);
                    value.encode(buf);
                }
            }
            
            RespValue::Set(elements) => {
                buf.put_u8(b'~');
                buf.put_slice(elements.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for element in elements {
                    element.encode(buf);
                }
            }
            
            RespValue::Attribute { attributes, value } => {
                buf.put_u8(b'|');
                buf.put_slice(attributes.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for (key, val) in attributes {
                    key.encode(buf);
                    val.encode(buf);
                }
                value.encode(buf);
            }
            
            RespValue::Push(elements) => {
                buf.put_u8(b'>');
                buf.put_slice(elements.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                for element in elements {
                    element.encode(buf);
                }
            }
        }
    }
}

/// Encode a command as RESP array
pub fn encode_command(cmd: &[&[u8]], buf: &mut BytesMut) {
    buf.put_u8(b'*');
    buf.put_slice(cmd.len().to_string().as_bytes());
    buf.put_slice(b"\r\n");
    
    for arg in cmd {
        buf.put_u8(b'$');
        buf.put_slice(arg.len().to_string().as_bytes());
        buf.put_slice(b"\r\n");
        buf.put_slice(arg);
        buf.put_slice(b"\r\n");
    }
}

/// Helper macro for building commands
#[macro_export]
macro_rules! cmd {
    ($($arg:expr),* $(,)?) => {{
        let args: Vec<&[u8]> = vec![$($arg.as_ref()),*];
        let mut buf = bytes::BytesMut::new();
        $crate::encode_command(&args, &mut buf);
        buf.freeze()
    }};
}

// Usage:
// let cmd = cmd!("SET", "key", "value");
// let cmd = cmd!("HSET", "hash", "field1", "value1", "field2", "value2");
```

### Connection Handler

```rust
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use bytes::BytesMut;

pub struct Connection {
    reader: BufReader<tokio::io::ReadHalf<TcpStream>>,
    writer: BufWriter<tokio::io::WriteHalf<TcpStream>>,
    read_buf: BytesMut,
    write_buf: BytesMut,
    parser: StreamingParser,
}

impl Connection {
    pub async fn new(stream: TcpStream) -> Self {
        let (read_half, write_half) = tokio::io::split(stream);
        Self {
            reader: BufReader::new(read_half),
            writer: BufWriter::new(write_half),
            read_buf: BytesMut::with_capacity(4096),
            write_buf: BytesMut::with_capacity(4096),
            parser: StreamingParser::new(),
        }
    }
    
    pub async fn read_value(&mut self) -> Result<RespValue, RespError> {
        loop {
            match self.parser.parse(&mut self.read_buf) {
                ParseResult::Complete(value) => return Ok(value),
                ParseResult::Incomplete(needed) => {
                    // Ensure buffer has space
                    if self.read_buf.capacity() - self.read_buf.len() < needed {
                        self.read_buf.reserve(needed);
                    }
                    
                    let n = self.reader.read_buf(&mut self.read_buf).await?;
                    if n == 0 {
                        return Err(RespError::ConnectionClosed);
                    }
                }
                ParseResult::Error(e) => return Err(e.into()),
            }
        }
    }
    
    pub async fn write_value(&mut self, value: &RespValue) -> Result<(), RespError> {
        value.encode(&mut self.write_buf);
        self.writer.write_all(&self.write_buf).await?;
        self.write_buf.clear();
        Ok(())
    }
    
    pub async fn flush(&mut self) -> Result<(), RespError> {
        self.writer.flush().await?;
        Ok(())
    }
    
    pub async fn send_command(&mut self, cmd: &[&[u8]]) -> Result<RespValue, RespError> {
        encode_command(cmd, &mut self.write_buf);
        self.writer.write_all(&self.write_buf).await?;
        self.write_buf.clear();
        self.writer.flush().await?;
        self.read_value().await
    }
}
```

### Async Client

```rust
use std::collections::VecDeque;
use tokio::sync::{mpsc, oneshot};

pub struct RedisClient {
    sender: mpsc::Sender<(Vec<u8>, oneshot::Sender<Result<RespValue, RespError>>)>,
}

impl RedisClient {
    pub async fn connect(addr: &str) -> Result<Self, RespError> {
        let stream = TcpStream::connect(addr).await?;
        let mut conn = Connection::new(stream).await;
        
        let (tx, mut rx) = mpsc::channel::<(Vec<u8>, oneshot::Sender<Result<RespValue, RespError>>)>(100);
        
        tokio::spawn(async move {
            let mut pending: VecDeque<oneshot::Sender<Result<RespValue, RespError>>> = VecDeque::new();
            
            loop {
                tokio::select! {
                    Some((cmd, responder)) = rx.recv() => {
                        if let Err(e) = conn.writer.write_all(&cmd).await {
                            let _ = responder.send(Err(e.into()));
                            continue;
                        }
                        if let Err(e) = conn.flush().await {
                            let _ = responder.send(Err(e));
                            continue;
                        }
                        pending.push_back(responder);
                    }
                    result = conn.read_value(), if !pending.is_empty() => {
                        if let Some(responder) = pending.pop_front() {
                            let _ = responder.send(result);
                        }
                    }
                    else => break,
                }
            }
        });
        
        Ok(Self { sender: tx })
    }
    
    pub async fn execute(&self, cmd: &[&[u8]]) -> Result<RespValue, RespError> {
        let mut buf = BytesMut::new();
        encode_command(cmd, &mut buf);
        
        let (tx, rx) = oneshot::channel();
        self.sender.send((buf.to_vec(), tx)).await
            .map_err(|_| RespError::ConnectionClosed)?;
        
        rx.await.map_err(|_| RespError::ConnectionClosed)?
    }
    
    // Convenience methods
    pub async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), RespError> {
        let result = self.execute(&[b"SET", key, value]).await?;
        match result {
            RespValue::SimpleString(s) if s == "OK" => Ok(()),
            RespValue::Error(e) => Err(RespError::Redis { 
                kind: e.kind, 
                message: e.message 
            }),
            _ => Err(RespError::Protocol("Unexpected response".into())),
        }
    }
    
    pub async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RespError> {
        let result = self.execute(&[b"GET", key]).await?;
        match result {
            RespValue::BulkString(data) => Ok(data),
            RespValue::Null => Ok(None),
            RespValue::Error(e) => Err(RespError::Redis {
                kind: e.kind,
                message: e.message,
            }),
            _ => Err(RespError::Protocol("Unexpected response".into())),
        }
    }
    
    pub async fn del(&self, keys: &[&[u8]]) -> Result<i64, RespError> {
        let mut cmd: Vec<&[u8]> = vec![b"DEL"];
        cmd.extend(keys);
        
        let result = self.execute(&cmd).await?;
        match result {
            RespValue::Integer(n) => Ok(n),
            RespValue::Error(e) => Err(RespError::Redis {
                kind: e.kind,
                message: e.message,
            }),
            _ => Err(RespError::Protocol("Unexpected response".into())),
        }
    }
}
```

### Pipeline Support

```rust
pub struct Pipeline {
    commands: Vec<Vec<u8>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }
    
    pub fn cmd(&mut self, cmd: &[&[u8]]) -> &mut Self {
        let mut buf = BytesMut::new();
        encode_command(cmd, &mut buf);
        self.commands.push(buf.to_vec());
        self
    }
    
    pub fn set(&mut self, key: &[u8], value: &[u8]) -> &mut Self {
        self.cmd(&[b"SET", key, value])
    }
    
    pub fn get(&mut self, key: &[u8]) -> &mut Self {
        self.cmd(&[b"GET", key])
    }
    
    pub fn incr(&mut self, key: &[u8]) -> &mut Self {
        self.cmd(&[b"INCR", key])
    }
    
    pub async fn execute(self, client: &RedisClient) -> Result<Vec<RespValue>, RespError> {
        let mut results = Vec::with_capacity(self.commands.len());
        
        // Send all commands
        let mut combined = Vec::new();
        for cmd in &self.commands {
            combined.extend(cmd);
        }
        
        // Note: This is simplified. A real implementation would use
        // the client's internal connection directly
        for cmd in &self.commands {
            // Parse command back to args (simplified)
            let result = client.sender.send((cmd.clone(), /* ... */)).await;
            // Collect results...
        }
        
        Ok(results)
    }
}
```

---

## Redis Modules

### Core Modules

```text
┌────────────────────────────────────────────────────────────────┐
│                      Redis Core Modules                        │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │   Strings   │  │    Lists    │  │         Hashes          │ │
│  │  (String)   │  │   (List)    │  │        (Hash)           │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                                                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │    Sets     │  │ Sorted Sets │  │        Streams          │ │
│  │   (Set)     │  │   (ZSet)    │  │       (Stream)          │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                                                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ HyperLogLog │  │    Geo      │  │        Bitmap           │ │
│  │   (String)  │  │  (ZSet)     │  │       (String)          │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### RediSearch Module

```text
Commands:
FT.CREATE index [ON HASH|JSON] [PREFIX count prefix ...] SCHEMA field type [SORTABLE] [field type ...]
FT.SEARCH index query [NOCONTENT] [VERBATIM] [NOSTOPWORDS] [WITHSCORES] [WITHPAYLOADS] [WITHSORTKEYS] 
    [FILTER numeric_field min max] [GEOFILTER geo_field lon lat radius m|km|mi|ft]
    [INKEYS count key ...] [INFIELDS count field ...] [RETURN count field ...]
    [SUMMARIZE [FIELDS count field ...] [FRAGS num] [LEN fragsize] [SEPARATOR sep]]
    [HIGHLIGHT [FIELDS count field ...] [TAGS open close]]
    [SLOP slop] [INORDER] [LANGUAGE language] [EXPANDER expander]
    [SCORER scorer] [EXPLAINSCORE] [PAYLOAD payload]
    [SORTBY field [ASC|DESC]] [LIMIT offset num]
FT.AGGREGATE index query [LOAD count field ...] [GROUPBY nargs property ...]
    [REDUCE function nargs arg ... [AS name]] [SORTBY nargs property [ASC|DESC] ...]
    [APPLY expression AS name] [LIMIT offset num] [FILTER expression]
FT.ALTER index SCHEMA ADD field type [SORTABLE] [field type ...]
FT.DROPINDEX index [DD]
FT.INFO index
FT.EXPLAIN index query
FT.EXPLAINCLI index query
FT.PROFILE index [SEARCH|AGGREGATE] [LIMITED] QUERY query
FT.TAGVALS index field
FT.SUGADD key string score [INCR] [PAYLOAD payload]
FT.SUGGET key prefix [FUZZY] [WITHSCORES] [WITHPAYLOADS] [MAX num]
FT.SUGDEL key string
FT.SUGLEN key
FT.SYNUPDATE index group_id [SKIPINITIALSCAN] term ...
FT.SYNDUMP index
FT.DICTADD dict term ...
FT.DICTDEL dict term ...
FT.DICTDUMP dict
FT.CONFIG SET option value
FT.CONFIG GET option
FT._LIST
FT.CURSOR READ index cursor_id [COUNT count]
FT.CURSOR DEL index cursor_id

Field Types:
- TEXT: Full-text indexed
- TAG: Tag/category field
- NUMERIC: Numeric range queries
- GEO: Geospatial queries
- VECTOR: Vector similarity search
```

### RedisJSON Module

```text
Commands:
JSON.SET key path value [NX|XX]
JSON.GET key [INDENT indent] [NEWLINE newline] [SPACE space] [path ...]
JSON.MGET key [key ...] path
JSON.DEL key [path]
JSON.FORGET key [path]
JSON.TYPE key [path]
JSON.NUMINCRBY key path value
JSON.NUMMULTBY key path value
JSON.STRAPPEND key [path] value
JSON.STRLEN key [path]
JSON.ARRAPPEND key path value [value ...]
JSON.ARRINDEX key path value [start [stop]]
JSON.ARRINSERT key path index value [value ...]
JSON.ARRLEN key [path]
JSON.ARRPOP key [path [index]]
JSON.ARRTRIM key path start stop
JSON.OBJKEYS key [path]
JSON.OBJLEN key [path]
JSON.CLEAR key [path]
JSON.TOGGLE key path
JSON.DEBUG MEMORY key [path]
JSON.RESP key [path]
JSON.MERGE key path value

Path Syntax (JSONPath):
$             - Root
.key          - Object key
[index]       - Array index
[*]           - All array elements
..            - Recursive descent
[start:end]   - Array slice
?(expression) - Filter expression
```

### RedisTimeSeries Module

```text
Commands:
TS.CREATE key [RETENTION retentionPeriod] [ENCODING [COMPRESSED|UNCOMPRESSED]] 
    [CHUNK_SIZE size] [DUPLICATE_POLICY policy] [LABELS label value ...]
TS.ALTER key [RETENTION retentionPeriod] [CHUNK_SIZE size] 
    [DUPLICATE_POLICY policy] [LABELS label value ...]
TS.ADD key timestamp value [RETENTION retentionPeriod] [ENCODING enc] 
    [CHUNK_SIZE size] [ON_DUPLICATE policy] [LABELS label value ...]
TS.MADD key timestamp value [key timestamp value ...]
TS.INCRBY key value [TIMESTAMP timestamp] [RETENTION retentionPeriod] 
    [UNCOMPRESSED] [CHUNK_SIZE size] [LABELS label value ...]
TS.DECRBY key value [TIMESTAMP timestamp] [RETENTION retentionPeriod] 
    [UNCOMPRESSED] [CHUNK_SIZE size] [LABELS label value ...]
TS.CREATERULE sourceKey destKey AGGREGATION aggregationType bucketDuration 
    [alignTimestamp]
TS.DELETERULE sourceKey destKey
TS.RANGE key fromTimestamp toTimestamp [LATEST] [FILTER_BY_TS ts...] 
    [FILTER_BY_VALUE min max] [COUNT count] 
    [ALIGN align] [AGGREGATION aggregationType bucketDuration] [BUCKETTIMESTAMP bt]
TS.REVRANGE key fromTimestamp toTimestamp [LATEST] [FILTER_BY_TS ts...] 
    [FILTER_BY_VALUE min max] [COUNT count] 
    [ALIGN align] [AGGREGATION aggregationType bucketDuration]
TS.MRANGE fromTimestamp toTimestamp [LATEST] [FILTER_BY_TS ts...] 
    [FILTER_BY_VALUE min max] [WITHLABELS | SELECTED_LABELS label...] 
    [COUNT count] [ALIGN align] [AGGREGATION aggregationType bucketDuration] 
    [GROUPBY label REDUCE reducer] FILTER filter...
TS.MREVRANGE fromTimestamp toTimestamp [LATEST] [FILTER_BY_TS ts...] 
    [FILTER_BY_VALUE min max] [WITHLABELS | SELECTED_LABELS label...] 
    [COUNT count] [ALIGN align] [AGGREGATION aggregationType bucketDuration] 
    [GROUPBY label REDUCE reducer] FILTER filter...
TS.GET key [LATEST]
TS.MGET [LATEST] [WITHLABELS | SELECTED_LABELS label...] FILTER filter...
TS.INFO key [DEBUG]
TS.QUERYINDEX filter...
TS.DEL key fromTimestamp toTimestamp

Aggregation Types:
- avg, sum, min, max
- range, count, first, last
- std.p, std.s, var.p, var.s
- twa (time-weighted average)
```

### RedisGraph Module (deprecated in Redis Stack 7.2+)

```text
Commands:
GRAPH.QUERY graphName query [TIMEOUT timeout]
GRAPH.RO_QUERY graphName query [TIMEOUT timeout]
GRAPH.DELETE graphName
GRAPH.EXPLAIN graphName query
GRAPH.PROFILE graphName query
GRAPH.SLOWLOG graphName
GRAPH.CONFIG GET name
GRAPH.CONFIG SET name value
GRAPH.LIST

Cypher Query Language Subset:
MATCH (n:Label {property: value})-[r:RELATIONSHIP]->(m)
WHERE n.property > value
CREATE (n:Label {property: value})
MERGE (n:Label {property: value})
SET n.property = value
DELETE n, r
RETURN n, r, m
ORDER BY n.property ASC|DESC
LIMIT number
SKIP number
WITH ... AS ...
OPTIONAL MATCH ...
UNION [ALL]
```

### RedisBloom Module

```text
Commands:
# Bloom Filter
BF.RESERVE key error_rate capacity [EXPANSION expansion] [NONSCALING]
BF.ADD key item
BF.MADD key item [item ...]
BF.INSERT key [CAPACITY cap] [ERROR error] [EXPANSION expansion] [NOCREATE] [NONSCALING] ITEMS item [item ...]
BF.EXISTS key item
BF.MEXISTS key item [item ...]
BF.SCANDUMP key iterator
BF.LOADCHUNK key iterator data
BF.INFO key [CAPACITY | SIZE | FILTERS | ITEMS | EXPANSION]
BF.CARD key

# Cuckoo Filter
CF.RESERVE key capacity [BUCKETSIZE bucketsize] [MAXITERATIONS maxiterations] [EXPANSION expansion]
CF.ADD key item
CF.ADDNX key item
CF.INSERT key [CAPACITY cap] [NOCREATE] ITEMS item [item ...]
CF.INSERTNX key [CAPACITY cap] [NOCREATE] ITEMS item [item ...]
CF.EXISTS key item
CF.MEXISTS key item [item ...]
CF.DEL key item
CF.COUNT key item
CF.SCANDUMP key iterator
CF.LOADCHUNK key iterator data
CF.INFO key

# Count-Min Sketch
CMS.INITBYDIM key width depth
CMS.INITBYPROB key error probability
CMS.INCRBY key item increment [item increment ...]
CMS.QUERY key item [item ...]
CMS.MERGE destKey numKeys srcKey [srcKey ...] [WEIGHTS weight [weight ...]]
CMS.INFO key

# Top-K
TOPK.RESERVE key topk [width depth decay]
TOPK.ADD key item [item ...]
TOPK.INCRBY key item increment [item increment ...]
TOPK.QUERY key item [item ...]
TOPK.COUNT key item [item ...]
TOPK.LIST key [WITHCOUNT]
TOPK.INFO key

# T-Digest
TDIGEST.CREATE key [COMPRESSION compression]
TDIGEST.RESET key
TDIGEST.ADD key value [value ...]
TDIGEST.MERGE destKey numKeys srcKey [srcKey ...] [COMPRESSION compression] [OVERRIDE]
TDIGEST.MIN key
TDIGEST.MAX key
TDIGEST.QUANTILE key quantile [quantile ...]
TDIGEST.CDF key value [value ...]
TDIGEST.TRIMMED_MEAN key low_cut_quantile high_cut_quantile
TDIGEST.RANK key value [value ...]
TDIGEST.REVRANK key value [value ...]
TDIGEST.BYRANK key rank [rank ...]
TDIGEST.BYREVRANK key rank [rank ...]
TDIGEST.INFO key
```

---

## Error Handling

### Error Categories

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCategory {
    /// Client-side errors (bad commands, wrong types)
    Client,
    /// Server-side errors (OOM, busy)
    Server,
    /// Cluster-specific errors (MOVED, ASK)
    Cluster,
    /// Authentication/Authorization errors
    Auth,
    /// Scripting errors
    Script,
    /// Replication errors  
    Replication,
}

impl ErrorKind {
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorKind::Err | ErrorKind::WrongType => ErrorCategory::Client,
            ErrorKind::Busy | ErrorKind::Oom | ErrorKind::Loading => ErrorCategory::Server,
            ErrorKind::Moved { .. } | ErrorKind::Ask { .. } | 
            ErrorKind::ClusterDown | ErrorKind::CrossSlot => ErrorCategory::Cluster,
            ErrorKind::NoAuth => ErrorCategory::Auth,
            ErrorKind::NoScript => ErrorCategory::Script,
            ErrorKind::MasterDown | ErrorKind::ReadOnly | 
            ErrorKind::NoReplicas => ErrorCategory::Replication,
            _ => ErrorCategory::Client,
        }
    }
    
    pub fn is_retriable(&self) -> bool {
        matches!(self, 
            ErrorKind::Busy | 
            ErrorKind::TryAgain | 
            ErrorKind::Loading |
            ErrorKind::ClusterDown
        )
    }
    
    pub fn is_redirect(&self) -> bool {
        matches!(self, ErrorKind::Moved { .. } | ErrorKind::Ask { .. })
    }
}
```

### Retry Logic

```rust
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            exponential_base: 2.0,
        }
    }
}

pub async fn with_retry<F, Fut, T>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T, RespError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, RespError>>,
{
    let mut attempt = 0;
    
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                let should_retry = match &e {
                    RespError::Redis { kind, .. } => kind.is_retriable(),
                    RespError::Io(_) | RespError::Timeout => true,
                    _ => false,
                };
                
                if !should_retry || attempt >= config.max_retries {
                    return Err(e);
                }
                
                let delay = std::cmp::min(
                    config.max_delay,
                    config.base_delay.mul_f64(
                        config.exponential_base.powi(attempt as i32)
                    ),
                );
                
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}
```

---

## Connection Lifecycle

### Connection States

```text
┌─────────────────────────────────────────────────────────────────┐
│                    Connection State Machine                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌──────────┐     connect()      ┌─────────────┐               │
│   │          │ ─────────────────> │             │               │
│   │ Closed   │                    │ Connecting  │               │
│   │          │ <───────────────── │             │               │
│   └──────────┘      timeout       └─────────────┘               │
│        ^                                 │                      │
│        │                                 │ connected            │
│        │                                 v                      │
│        │                          ┌─────────────┐               │
│        │                          │             │               │
│        │ close()                  │  Connected  │◄──┐           │
│        │                          │             │   │           │
│        │                          └─────────────┘   │           │
│        │                                 │          │           │
│        │                    AUTH/HELLO   │          │ success   │
│        │                                 v          │           │
│        │                          ┌─────────────┐   │           │
│        │                          │             │   │           │
│        │                          │Authenticating ──┘           │
│        │                          │             │               │
│        │                          └─────────────┘               │
│        │                                 │                      │
│        │                          error  │ success              │
│        │                                 v                      │
│        │                          ┌─────────────┐               │
│        │                          │             │               │
│        └────────────────────────  │    Ready    │               │
│                  disconnect       │             │               │
│                                   └─────────────┘               │
│                                          │                      │
│                            SUBSCRIBE/    │                      │
│                            PSUBSCRIBE    v                      │
│                                   ┌─────────────┐               │
│                                   │             │               │
│                                   │  Subscribed │               │
│                                   │ (Pub/Sub)   │               │
│                                   └─────────────┘               │
│                                          │                      │
│                             UNSUBSCRIBE/ │                      │
│                             PUNSUBSCRIBE v                      │
│                                   ┌─────────────┐               │
│                                   │    Ready    │               │
│                                   └─────────────┘               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Connection Pool

```rust
use std::sync::Arc;
use tokio::sync::{Semaphore, Mutex};
use std::collections::VecDeque;

pub struct ConnectionPool {
    connections: Arc<Mutex<VecDeque<Connection>>>,
    semaphore: Arc<Semaphore>,
    addr: String,
    min_size: usize,
    max_size: usize,
    password: Option<String>,
    database: u8,
}

pub struct PooledConnection {
    conn: Option<Connection>,
    pool: Arc<Mutex<VecDeque<Connection>>>,
    semaphore: Arc<Semaphore>,
}

impl ConnectionPool {
    pub async fn new(config: PoolConfig) -> Result<Self, RespError> {
        let pool = Self {
            connections: Arc::new(Mutex::new(VecDeque::with_capacity(config.max_size))),
            semaphore: Arc::new(Semaphore::new(config.max_size)),
            addr: config.addr,
            min_size: config.min_size,
            max_size: config.max_size,
            password: config.password,
            database: config.database,
        };
        
        // Initialize minimum connections
        for _ in 0..config.min_size {
            let conn = pool.create_connection().await?;
            pool.connections.lock().await.push_back(conn);
        }
        
        Ok(pool)
    }
    
    async fn create_connection(&self) -> Result<Connection, RespError> {
        let stream = TcpStream::connect(&self.addr).await?;
        let mut conn = Connection::new(stream).await;
        
        // Authenticate if password set
        if let Some(ref password) = self.password {
            let result = conn.send_command(&[b"AUTH", password.as_bytes()]).await?;
            match result {
                RespValue::SimpleString(s) if s == "OK" => {}
                RespValue::Error(e) => return Err(RespError::Redis {
                    kind: e.kind,
                    message: e.message,
                }),
                _ => return Err(RespError::Protocol("Unexpected AUTH response".into())),
            }
        }
        
        // Select database
        if self.database != 0 {
            let db_str = self.database.to_string();
            let result = conn.send_command(&[b"SELECT", db_str.as_bytes()]).await?;
            match result {
                RespValue::SimpleString(s) if s == "OK" => {}
                RespValue::Error(e) => return Err(RespError::Redis {
                    kind: e.kind,
                    message: e.message,
                }),
                _ => return Err(RespError::Protocol("Unexpected SELECT response".into())),
            }
        }
        
        Ok(conn)
    }
    
    pub async fn get(&self) -> Result<PooledConnection, RespError> {
        let permit = self.semaphore.clone().acquire_owned().await
            .map_err(|_| RespError::ConnectionClosed)?;
        
        let conn = {
            let mut conns = self.connections.lock().await;
            conns.pop_front()
        };
        
        let conn = match conn {
            Some(c) => c,
            None => self.create_connection().await?,
        };
        
        // Drop the permit - we track via the VecDeque
        drop(permit);
        
        Ok(PooledConnection {
            conn: Some(conn),
            pool: self.connections.clone(),
            semaphore: self.semaphore.clone(),
        })
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                pool.lock().await.push_back(conn);
            });
        }
    }
}

impl std::ops::Deref for PooledConnection {
    type Target = Connection;
    
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().unwrap()
    }
}

impl std::ops::DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().unwrap()
    }
}

pub struct PoolConfig {
    pub addr: String,
    pub min_size: usize,
    pub max_size: usize,
    pub password: Option<String>,
    pub database: u8,
    pub connect_timeout: Duration,
    pub idle_timeout: Duration,
}
```

---

## Pipelining and Transactions

### Pipelining Wire Format

```text
Client sends (pipelined):
*3\r\n$3\r\nSET\r\n$4\r\nkey1\r\n$6\r\nvalue1\r\n
*3\r\n$3\r\nSET\r\n$4\r\nkey2\r\n$6\r\nvalue2\r\n
*2\r\n$3\r\nGET\r\n$4\r\nkey1\r\n
*2\r\n$3\r\nGET\r\n$4\r\nkey2\r\n

Server responds (in order):
+OK\r\n
+OK\r\n
$6\r\nvalue1\r\n
$6\r\nvalue2\r\n
```

### Transaction (MULTI/EXEC) Wire Format

```text
Client:
*1\r\n$5\r\nMULTI\r\n
*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
*3\r\n$6\r\nINCRBY\r\n$7\r\ncounter\r\n$1\r\n1\r\n
*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n
*1\r\n$4\r\nEXEC\r\n

Server:
+OK\r\n           <- MULTI response
+QUEUED\r\n       <- SET queued
+QUEUED\r\n       <- INCRBY queued
+QUEUED\r\n       <- GET queued
*3\r\n            <- EXEC response (array of 3)
+OK\r\n           <- SET result
:1\r\n            <- INCRBY result
$3\r\nbar\r\n     <- GET result
```

### Transaction Implementation

```rust
pub struct Transaction {
    commands: Vec<Vec<u8>>,
}

impl Transaction {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }
    
    pub fn cmd(&mut self, cmd: &[&[u8]]) -> &mut Self {
        let mut buf = BytesMut::new();
        encode_command(cmd, &mut buf);
        self.commands.push(buf.to_vec());
        self
    }
    
    pub async fn execute(self, conn: &mut Connection) -> Result<Vec<RespValue>, RespError> {
        // Send MULTI
        let multi_result = conn.send_command(&[b"MULTI"]).await?;
        match multi_result {
            RespValue::SimpleString(s) if s == "OK" => {}
            RespValue::Error(e) => return Err(RespError::Redis {
                kind: e.kind,
                message: e.message,
            }),
            _ => return Err(RespError::Protocol("Unexpected MULTI response".into())),
        }
        
        // Send all commands
        for cmd in &self.commands {
            conn.writer.write_all(cmd).await?;
            conn.flush().await?;
            
            let result = conn.read_value().await?;
            match result {
                RespValue::SimpleString(s) if s == "QUEUED" => {}
                RespValue::Error(e) => {
                    // Discard transaction
                    let _ = conn.send_command(&[b"DISCARD"]).await;
                    return Err(RespError::Redis {
                        kind: e.kind,
                        message: e.message,
                    });
                }
                _ => {
                    let _ = conn.send_command(&[b"DISCARD"]).await;
                    return Err(RespError::Protocol("Expected QUEUED".into()));
                }
            }
        }
        
        // Send EXEC
        let exec_result = conn.send_command(&[b"EXEC"]).await?;
        match exec_result {
            RespValue::Array(Some(results)) => Ok(results),
            RespValue::Array(None) | RespValue::Null => {
                // Transaction aborted (WATCH failed)
                Err(RespError::Redis {
                    kind: ErrorKind::ExecAbort,
                    message: "Transaction discarded due to WATCH".into(),
                })
            }
            RespValue::Error(e) => Err(RespError::Redis {
                kind: e.kind,
                message: e.message,
            }),
            _ => Err(RespError::Protocol("Unexpected EXEC response".into())),
        }
    }
}

// Optimistic locking with WATCH
pub async fn watch_execute<F, Fut>(
    conn: &mut Connection,
    keys: &[&[u8]],
    f: F,
) -> Result<Vec<RespValue>, RespError>
where
    F: FnOnce(&mut Transaction) -> Fut,
    Fut: std::future::Future<Output = Result<(), RespError>>,
{
    // WATCH keys
    let mut watch_cmd: Vec<&[u8]> = vec![b"WATCH"];
    watch_cmd.extend(keys);
    let watch_result = conn.send_command(&watch_cmd).await?;
    match watch_result {
        RespValue::SimpleString(s) if s == "OK" => {}
        RespValue::Error(e) => return Err(RespError::Redis {
            kind: e.kind,
            message: e.message,
        }),
        _ => return Err(RespError::Protocol("Unexpected WATCH response".into())),
    }
    
    // Build transaction
    let mut tx = Transaction::new();
    f(&mut tx).await?;
    
    // Execute
    tx.execute(conn).await
}
```

---

## Pub/Sub Protocol

### Subscribe Wire Format

```text
Client:
*2\r\n$9\r\nSUBSCRIBE\r\n$7\r\nchannel\r\n

Server (push messages):
*3\r\n
$9\r\nsubscribe\r\n
$7\r\nchannel\r\n
:1\r\n

# When a message arrives:
*3\r\n
$7\r\nmessage\r\n
$7\r\nchannel\r\n
$5\r\nhello\r\n
```

### Pattern Subscribe

```text
Client:
*2\r\n$10\r\nPSUBSCRIBE\r\n$5\r\nnews.*\r\n

Server:
*3\r\n
$10\r\npsubscribe\r\n
$5\r\nnews.*\r\n
:1\r\n

# Pattern match message:
*4\r\n
$8\r\npmessage\r\n
$5\r\nnews.*\r\n         <- pattern
$10\r\nnews.sports\r\n   <- actual channel
$11\r\nGoal scored!\r\n  <- payload
```

### Pub/Sub Implementation

```rust
use tokio::sync::broadcast;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum PubSubMessage {
    Subscribe { channel: String, count: i64 },
    Unsubscribe { channel: String, count: i64 },
    PSubscribe { pattern: String, count: i64 },
    PUnsubscribe { pattern: String, count: i64 },
    Message { channel: String, payload: Vec<u8> },
    PMessage { pattern: String, channel: String, payload: Vec<u8> },
}

pub struct PubSubConnection {
    conn: Connection,
    subscriptions: HashMap<String, ()>,
    patterns: HashMap<String, ()>,
}

impl PubSubConnection {
    pub async fn new(stream: TcpStream) -> Self {
        Self {
            conn: Connection::new(stream).await,
            subscriptions: HashMap::new(),
            patterns: HashMap::new(),
        }
    }
    
    pub async fn subscribe(&mut self, channels: &[&str]) -> Result<(), RespError> {
        let mut cmd: Vec<&[u8]> = vec![b"SUBSCRIBE"];
        for ch in channels {
            cmd.push(ch.as_bytes());
        }
        
        encode_command(&cmd, &mut self.conn.write_buf);
        self.conn.writer.write_all(&self.conn.write_buf).await?;
        self.conn.write_buf.clear();
        self.conn.flush().await?;
        
        // Read subscription confirmations
        for ch in channels {
            let msg = self.read_message().await?;
            match msg {
                PubSubMessage::Subscribe { channel, .. } => {
                    self.subscriptions.insert(channel, ());
                }
                _ => return Err(RespError::Protocol("Expected subscribe confirmation".into())),
            }
        }
        
        Ok(())
    }
    
    pub async fn psubscribe(&mut self, patterns: &[&str]) -> Result<(), RespError> {
        let mut cmd: Vec<&[u8]> = vec![b"PSUBSCRIBE"];
        for p in patterns {
            cmd.push(p.as_bytes());
        }
        
        encode_command(&cmd, &mut self.conn.write_buf);
        self.conn.writer.write_all(&self.conn.write_buf).await?;
        self.conn.write_buf.clear();
        self.conn.flush().await?;
        
        for p in patterns {
            let msg = self.read_message().await?;
            match msg {
                PubSubMessage::PSubscribe { pattern, .. } => {
                    self.patterns.insert(pattern, ());
                }
                _ => return Err(RespError::Protocol("Expected psubscribe confirmation".into())),
            }
        }
        
        Ok(())
    }
    
    pub async fn read_message(&mut self) -> Result<PubSubMessage, RespError> {
        let value = self.conn.read_value().await?;
        
        match value {
            RespValue::Array(Some(elements)) if elements.len() >= 3 => {
                let msg_type = match &elements[0] {
                    RespValue::BulkString(Some(b)) => String::from_utf8_lossy(b).to_string(),
                    _ => return Err(RespError::Protocol("Invalid message type".into())),
                };
                
                match msg_type.as_str() {
                    "subscribe" => {
                        let channel = extract_string(&elements[1])?;
                        let count = extract_integer(&elements[2])?;
                        Ok(PubSubMessage::Subscribe { channel, count })
                    }
                    "unsubscribe" => {
                        let channel = extract_string(&elements[1])?;
                        let count = extract_integer(&elements[2])?;
                        Ok(PubSubMessage::Unsubscribe { channel, count })
                    }
                    "psubscribe" => {
                        let pattern = extract_string(&elements[1])?;
                        let count = extract_integer(&elements[2])?;
                        Ok(PubSubMessage::PSubscribe { pattern, count })
                    }
                    "punsubscribe" => {
                        let pattern = extract_string(&elements[1])?;
                        let count = extract_integer(&elements[2])?;
                        Ok(PubSubMessage::PUnsubscribe { pattern, count })
                    }
                    "message" => {
                        let channel = extract_string(&elements[1])?;
                        let payload = extract_bytes(&elements[2])?;
                        Ok(PubSubMessage::Message { channel, payload })
                    }
                    "pmessage" if elements.len() >= 4 => {
                        let pattern = extract_string(&elements[1])?;
                        let channel = extract_string(&elements[2])?;
                        let payload = extract_bytes(&elements[3])?;
                        Ok(PubSubMessage::PMessage { pattern, channel, payload })
                    }
                    _ => Err(RespError::Protocol(format!("Unknown message type: {}", msg_type))),
                }
            }
            RespValue::Push(elements) if elements.len() >= 3 => {
                // RESP3 push format - similar handling
                // ...
                todo!("RESP3 push handling")
            }
            _ => Err(RespError::Protocol("Invalid pub/sub message format".into())),
        }
    }
    
    pub async fn unsubscribe(&mut self, channels: &[&str]) -> Result<(), RespError> {
        let mut cmd: Vec<&[u8]> = vec![b"UNSUBSCRIBE"];
        for ch in channels {
            cmd.push(ch.as_bytes());
        }
        
        encode_command(&cmd, &mut self.conn.write_buf);
        self.conn.writer.write_all(&self.conn.write_buf).await?;
        self.conn.write_buf.clear();
        self.conn.flush().await?;
        
        for _ in channels {
            let msg = self.read_message().await?;
            match msg {
                PubSubMessage::Unsubscribe { channel, .. } => {
                    self.subscriptions.remove(&channel);
                }
                _ => {}
            }
        }
        
        Ok(())
    }
}

fn extract_string(value: &RespValue) -> Result<String, RespError> {
    match value {
        RespValue::BulkString(Some(b)) => String::from_utf8(b.clone())
            .map_err(|e| RespError::Protocol(e.to_string())),
        RespValue::SimpleString(s) => Ok(s.clone()),
        _ => Err(RespError::Protocol("Expected string".into())),
    }
}

fn extract_bytes(value: &RespValue) -> Result<Vec<u8>, RespError> {
    match value {
        RespValue::BulkString(Some(b)) => Ok(b.clone()),
        _ => Err(RespError::Protocol("Expected bulk string".into())),
    }
}

fn extract_integer(value: &RespValue) -> Result<i64, RespError> {
    match value {
        RespValue::Integer(n) => Ok(*n),
        _ => Err(RespError::Protocol("Expected integer".into())),
    }
}
```

---

## Cluster Protocol Extensions

### Cluster Redirections

```text
# MOVED - Permanent redirect
-MOVED 3999 127.0.0.1:6381\r\n

# ASK - Temporary redirect (slot migration)
-ASK 3999 127.0.0.1:6381\r\n
```

### Slot Calculation (CRC16)

```rust
/// Calculate Redis cluster slot for a key
pub fn slot(key: &[u8]) -> u16 {
    // Handle hash tags: {tag}rest -> use only "tag"
    let key = if let Some(start) = key.iter().position(|&b| b == b'{') {
        if let Some(end) = key[start..].iter().position(|&b| b == b'}') {
            if end > 1 {
                &key[start + 1..start + end]
            } else {
                key
            }
        } else {
            key
        }
    } else {
        key
    };
    
    crc16(key) % 16384
}

/// CRC16 implementation (XMODEM/CCITT)
fn crc16(data: &[u8]) -> u16 {
    const CRC16_TABLE: [u16; 256] = [
        0x0000, 0x1021, 0x2042, 0x3063, 0x4084, 0x50a5, 0x60c6, 0x70e7,
        0x8108, 0x9129, 0xa14a, 0xb16b, 0xc18c, 0xd1ad, 0xe1ce, 0xf1ef,
        0x1231, 0x0210, 0x3273, 0x2252, 0x52b5, 0x4294, 0x72f7, 0x62d6,
        0x9339, 0x8318, 0xb37b, 0xa35a, 0xd3bd, 0xc39c, 0xf3ff, 0xe3de,
        0x2462, 0x3443, 0x0420, 0x1401, 0x64e6, 0x74c7, 0x44a4, 0x5485,
        0xa56a, 0xb54b, 0x8528, 0x9509, 0xe5ee, 0xf5cf, 0xc5ac, 0xd58d,
        0x3653, 0x2672, 0x1611, 0x0630, 0x76d7, 0x66f6, 0x5695, 0x46b4,
        0xb75b, 0xa77a, 0x9719, 0x8738, 0xf7df, 0xe7fe, 0xd79d, 0xc7bc,
        0x48c4, 0x58e5, 0x6886, 0x78a7, 0x0840, 0x1861, 0x2802, 0x3823,
        0xc9cc, 0xd9ed, 0xe98e, 0xf9af, 0x8948, 0x9969, 0xa90a, 0xb92b,
        0x5af5, 0x4ad4, 0x7ab7, 0x6a96, 0x1a71, 0x0a50, 0x3a33, 0x2a12,
        0xdbfd, 0xcbdc, 0xfbbf, 0xeb9e, 0x9b79, 0x8b58, 0xbb3b, 0xab1a,
        0x6ca6, 0x7c87, 0x4ce4, 0x5cc5, 0x2c22, 0x3c03, 0x0c60, 0x1c41,
        0xedae, 0xfd8f, 0xcdec, 0xddcd, 0xad2a, 0xbd0b, 0x8d68, 0x9d49,
        0x7e97, 0x6eb6, 0x5ed5, 0x4ef4, 0x3e13, 0x2e32, 0x1e51, 0x0e70,
        0xff9f, 0xefbe, 0xdfdd, 0xcffc, 0xbf1b, 0xaf3a, 0x9f59, 0x8f78,
        0x9188, 0x81a9, 0xb1ca, 0xa1eb, 0xd10c, 0xc12d, 0xf14e, 0xe16f,
        0x1080, 0x00a1, 0x30c2, 0x20e3, 0x5004, 0x4025, 0x7046, 0x6067,
        0x83b9, 0x9398, 0xa3fb, 0xb3da, 0xc33d, 0xd31c, 0xe37f, 0xf35e,
        0x02b1, 0x1290, 0x22f3, 0x32d2, 0x4235, 0x5214, 0x6277, 0x7256,
        0xb5ea, 0xa5cb, 0x95a8, 0x8589, 0xf56e, 0xe54f, 0xd52c, 0xc50d,
        0x34e2, 0x24c3, 0x14a0, 0x0481, 0x7466, 0x6447, 0x5424, 0x4405,
        0xa7db, 0xb7fa, 0x8799, 0x97b8, 0xe75f, 0xf77e, 0xc71d, 0xd73c,
        0x26d3, 0x36f2, 0x0691, 0x16b0, 0x6657, 0x7676, 0x4615, 0x5634,
        0xd94c, 0xc96d, 0xf90e, 0xe92f, 0x99c8, 0x89e9, 0xb98a, 0xa9ab,
        0x5844, 0x4865, 0x7806, 0x6827, 0x18c0, 0x08e1, 0x3882, 0x28a3,
        0xcb7d, 0xdb5c, 0xeb3f, 0xfb1e, 0x8bf9, 0x9bd8, 0xabbb, 0xbb9a,
        0x4a75, 0x5a54, 0x6a37, 0x7a16, 0x0af1, 0x1ad0, 0x2ab3, 0x3a92,
        0xfd2e, 0xed0f, 0xdd6c, 0xcd4d, 0xbdaa, 0xad8b, 0x9de8, 0x8dc9,
        0x7c26, 0x6c07, 0x5c64, 0x4c45, 0x3ca2, 0x2c83, 0x1ce0, 0x0cc1,
        0xef1f, 0xff3e, 0xcf5d, 0xdf7c, 0xaf9b, 0xbfba, 0x8fd9, 0x9ff8,
        0x6e17, 0x7e36, 0x4e55, 0x5e74, 0x2e93, 0x3eb2, 0x0ed1, 0x1ef0,
    ];
    
    let mut crc: u16 = 0;
    for byte in data {
        let index = ((crc >> 8) ^ (*byte as u16)) & 0xFF;
        crc = (crc << 8) ^ CRC16_TABLE[index as usize];
    }
    crc
}
```

### Cluster Client

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ClusterClient {
    nodes: Arc<RwLock<HashMap<String, Connection>>>,
    slots: Arc<RwLock<Vec<Option<String>>>>,  // slot -> node addr
}

impl ClusterClient {
    pub async fn connect(initial_nodes: &[&str]) -> Result<Self, RespError> {
        let mut nodes = HashMap::new();
        let mut slots = vec![None; 16384];
        
        // Connect to first available node
        for addr in initial_nodes {
            match TcpStream::connect(addr).await {
                Ok(stream) => {
                    let mut conn = Connection::new(stream).await;
                    
                    // Get cluster slots
                    let result = conn.send_command(&[b"CLUSTER", b"SLOTS"]).await?;
                    Self::parse_cluster_slots(&result, &mut slots)?;
                    
                    nodes.insert(addr.to_string(), conn);
                    break;
                }
                Err(_) => continue,
            }
        }
        
        if nodes.is_empty() {
            return Err(RespError::ConnectionClosed);
        }
        
        Ok(Self {
            nodes: Arc::new(RwLock::new(nodes)),
            slots: Arc::new(RwLock::new(slots)),
        })
    }
    
    fn parse_cluster_slots(value: &RespValue, slots: &mut Vec<Option<String>>) -> Result<(), RespError> {
        match value {
            RespValue::Array(Some(ranges)) => {
                for range in ranges {
                    if let RespValue::Array(Some(range_info)) = range {
                        if range_info.len() < 3 {
                            continue;
                        }
                        
                        let start = match &range_info[0] {
                            RespValue::Integer(n) => *n as usize,
                            _ => continue,
                        };
                        let end = match &range_info[1] {
                            RespValue::Integer(n) => *n as usize,
                            _ => continue,
                        };
                        
                        // Master node info
                        if let RespValue::Array(Some(master)) = &range_info[2] {
                            if master.len() >= 2 {
                                let ip = match &master[0] {
                                    RespValue::BulkString(Some(b)) => 
                                        String::from_utf8_lossy(b).to_string(),
                                    _ => continue,
                                };
                                let port = match &master[1] {
                                    RespValue::Integer(n) => *n,
                                    _ => continue,
                                };
                                
                                let addr = format!("{}:{}", ip, port);
                                for slot in start..=end {
                                    if slot < 16384 {
                                        slots[slot] = Some(addr.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => Err(RespError::Protocol("Invalid CLUSTER SLOTS response".into())),
        }
    }
    
    pub async fn execute(&self, key: &[u8], cmd: &[&[u8]]) -> Result<RespValue, RespError> {
        let slot_num = slot(key);
        
        loop {
            let addr = {
                let slots = self.slots.read().await;
                slots[slot_num as usize].clone()
            };
            
            let addr = addr.ok_or_else(|| RespError::Protocol("No node for slot".into()))?;
            
            let result = self.execute_on_node(&addr, cmd).await;
            
            match result {
                Ok(RespValue::Error(ref e)) if e.kind.is_redirect() => {
                    match &e.kind {
                        ErrorKind::Moved { slot, addr } => {
                            // Update slot mapping
                            let mut slots = self.slots.write().await;
                            slots[*slot as usize] = Some(addr.clone());
                            // Retry automatically
                            continue;
                        }
                        ErrorKind::Ask { addr, .. } => {
                            // Send ASKING first, then retry
                            self.execute_on_node(addr, &[b"ASKING"]).await?;
                            return self.execute_on_node(addr, cmd).await;
                        }
                        _ => return result,
                    }
                }
                other => return other,
            }
        }
    }
    
    async fn execute_on_node(&self, addr: &str, cmd: &[&[u8]]) -> Result<RespValue, RespError> {
        // Get or create connection
        let mut nodes = self.nodes.write().await;
        
        let conn = if let Some(conn) = nodes.get_mut(addr) {
            conn
        } else {
            let stream = TcpStream::connect(addr).await?;
            let conn = Connection::new(stream).await;
            nodes.insert(addr.to_string(), conn);
            nodes.get_mut(addr).unwrap()
        };
        
        conn.send_command(cmd).await
    }
}
```

---

## Streams Protocol

### Stream Entry ID Format

```text
<millisecondsTime>-<sequenceNumber>

Examples:
1526919030474-0    # First entry at timestamp
1526919030474-1    # Second entry at same millisecond
*                  # Auto-generate ID (XADD)
$                  # Special: new entries only (XREAD)
>                  # Special: never-delivered (XREADGROUP)
-                  # Minimum ID
+                  # Maximum ID
```

### XADD Wire Format

```text
Client:
*7\r\n
$4\r\nXADD\r\n
$8\r\nmystream\r\n
$1\r\n*\r\n
$4\r\nname\r\n
$5\r\nSara\r\n
$7\r\nsurname\r\n
$5\r\nOC\r\n

Server:
$15\r\n1526919030474-0\r\n
```

### XREAD Wire Format

```text
Client:
*6\r\n
$5\r\nXREAD\r\n
$5\r\nBLOCK\r\n
$1\r\n0\r\n
$7\r\nSTREAMS\r\n
$8\r\nmystream\r\n
$1\r\n$\r\n

Server (blocking response):
*1\r\n
*2\r\n
$8\r\nmystream\r\n
*1\r\n
*2\r\n
$15\r\n1526919030474-0\r\n
*4\r\n
$4\r\nname\r\n
$5\r\nSara\r\n
$7\r\nsurname\r\n
$5\r\nOC\r\n
```

### Stream Implementation

```rust
#[derive(Debug, Clone)]
pub struct StreamId {
    pub ms: u64,
    pub seq: u64,
}

impl StreamId {
    pub fn parse(s: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 2 {
            return Err(ParseError::InvalidStreamId);
        }
        Ok(Self {
            ms: parts[0].parse().map_err(|_| ParseError::InvalidStreamId)?,
            seq: parts[1].parse().map_err(|_| ParseError::InvalidStreamId)?,
        })
    }
    
    pub fn to_string(&self) -> String {
        format!("{}-{}", self.ms, self.seq)
    }
}

impl Ord for StreamId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.ms.cmp(&other.ms) {
            std::cmp::Ordering::Equal => self.seq.cmp(&other.seq),
            other => other,
        }
    }
}

impl PartialOrd for StreamId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct StreamEntry {
    pub id: StreamId,
    pub fields: Vec<(Vec<u8>, Vec<u8>)>,
}

pub struct StreamClient {
    client: RedisClient,
}

impl StreamClient {
    pub async fn xadd(
        &self,
        key: &[u8],
        id: Option<&str>,
        fields: &[(&[u8], &[u8])],
    ) -> Result<StreamId, RespError> {
        let mut cmd: Vec<&[u8]> = vec![b"XADD", key];
        
        let id_str = id.unwrap_or("*");
        cmd.push(id_str.as_bytes());
        
        for (field, value) in fields {
            cmd.push(field);
            cmd.push(value);
        }
        
        let result = self.client.execute(&cmd).await?;
        match result {
            RespValue::BulkString(Some(id_bytes)) => {
                let id_str = String::from_utf8(id_bytes)?;
                StreamId::parse(&id_str).map_err(|e| RespError::Parse(e))
            }
            RespValue::Error(e) => Err(RespError::Redis {
                kind: e.kind,
                message: e.message,
            }),
            _ => Err(RespError::Protocol("Unexpected XADD response".into())),
        }
    }
    
    pub async fn xread(
        &self,
        streams: &[(&[u8], &str)],
        count: Option<u64>,
        block: Option<u64>,
    ) -> Result<Vec<(Vec<u8>, Vec<StreamEntry>)>, RespError> {
        let mut cmd: Vec<Vec<u8>> = vec![b"XREAD".to_vec()];
        
        if let Some(c) = count {
            cmd.push(b"COUNT".to_vec());
            cmd.push(c.to_string().into_bytes());
        }
        
        if let Some(b) = block {
            cmd.push(b"BLOCK".to_vec());
            cmd.push(b.to_string().into_bytes());
        }
        
        cmd.push(b"STREAMS".to_vec());
        
        for (key, _) in streams {
            cmd.push(key.to_vec());
        }
        
        for (_, id) in streams {
            cmd.push(id.as_bytes().to_vec());
        }
        
        let cmd_refs: Vec<&[u8]> = cmd.iter().map(|v| v.as_slice()).collect();
        let result = self.client.execute(&cmd_refs).await?;
        
        Self::parse_xread_response(result)
    }
    
    fn parse_xread_response(value: RespValue) -> Result<Vec<(Vec<u8>, Vec<StreamEntry>)>, RespError> {
        match value {
            RespValue::Array(None) | RespValue::Null => Ok(vec![]),
            RespValue::Array(Some(streams)) => {
                let mut result = Vec::new();
                
                for stream in streams {
                    if let RespValue::Array(Some(mut parts)) = stream {
                        if parts.len() != 2 {
                            continue;
                        }
                        
                        let entries_value = parts.pop().unwrap();
                        let key_value = parts.pop().unwrap();
                        
                        let key = match key_value {
                            RespValue::BulkString(Some(k)) => k,
                            _ => continue,
                        };
                        
                        let entries = Self::parse_entries(entries_value)?;
                        result.push((key, entries));
                    }
                }
                
                Ok(result)
            }
            RespValue::Error(e) => Err(RespError::Redis {
                kind: e.kind,
                message: e.message,
            }),
            _ => Err(RespError::Protocol("Unexpected XREAD response".into())),
        }
    }
    
    fn parse_entries(value: RespValue) -> Result<Vec<StreamEntry>, RespError> {
        match value {
            RespValue::Array(Some(entries)) => {
                let mut result = Vec::new();
                
                for entry in entries {
                    if let RespValue::Array(Some(mut parts)) = entry {
                        if parts.len() != 2 {
                            continue;
                        }
                        
                        let fields_value = parts.pop().unwrap();
                        let id_value = parts.pop().unwrap();
                        
                        let id = match id_value {
                            RespValue::BulkString(Some(id_bytes)) => {
                                let id_str = String::from_utf8(id_bytes)?;
                                StreamId::parse(&id_str)?
                            }
                            _ => continue,
                        };
                        
                        let fields = Self::parse_fields(fields_value)?;
                        result.push(StreamEntry { id, fields });
                    }
                }
                
                Ok(result)
            }
            _ => Err(RespError::Protocol("Expected entries array".into())),
        }
    }
    
    fn parse_fields(value: RespValue) -> Result<Vec<(Vec<u8>, Vec<u8>)>, RespError> {
        match value {
            RespValue::Array(Some(items)) => {
                let mut result = Vec::new();
                let mut iter = items.into_iter();
                
                while let (Some(field), Some(value)) = (iter.next(), iter.next()) {
                    let field = match field {
                        RespValue::BulkString(Some(f)) => f,
                        _ => continue,
                    };
                    let value = match value {
                        RespValue::BulkString(Some(v)) => v,
                        _ => continue,
                    };
                    result.push((field, value));
                }
                
                Ok(result)
            }
            _ => Err(RespError::Protocol("Expected fields array".into())),
        }
    }
}
```

---

## Complete Command Reference

### Command Categories and Complexity

```text
┌────────────────────────────────────────────────────────────────────────────┐
│                        Command Complexity Reference                        │
├────────────────────────────────────────────────────────────────────────────┤
│ Command          │ Time Complexity      │ Notes                            │
├──────────────────┼──────────────────────┼──────────────────────────────────┤
│ GET              │ O(1)                 │                                  │
│ SET              │ O(1)                 │                                  │
│ DEL              │ O(N)                 │ N = number of keys               │
│ KEYS             │ O(N)                 │ N = keyspace size (avoid prod)   │
│ SCAN             │ O(1) per call        │ Use COUNT to tune                │
│ HGETALL          │ O(N)                 │ N = hash fields                  │
│ SMEMBERS         │ O(N)                 │ N = set cardinality              │
│ LRANGE           │ O(S+N)               │ S=start offset, N=elements       │
│ ZRANGE           │ O(log(N)+M)          │ N=cardinality, M=elements        │
│ ZADD             │ O(log(N)*M)          │ N=cardinality, M=elements added  │
│ ZUNIONSTORE      │ O(N)+O(M*log(M))     │ N=input size, M=output size      │
│ SORT             │ O(N+M*log(M))        │ N=elements, M=returned           │
│ EVAL             │ O(1) to find script  │ Script complexity varies         │
│ XADD             │ O(1)                 │ O(N) if capped                   │
│ XRANGE           │ O(N)                 │ N = returned entries             │
│ XREAD BLOCK      │ O(N)                 │ N = entries                      │
│ PFADD            │ O(1)                 │ Constant time approximation      │
│ PFCOUNT          │ O(1) single key      │ O(N) for N keys                  │
│ GEOADD           │ O(log(N)*M)          │ N=elements in set, M=added       │
│ GEORADIUS        │ O(N+log(M))          │ N=radius matches, M=set size     │
└──────────────────┴──────────────────────┴──────────────────────────────────┘
```

### Flags and Options Reference

```rust
/// Common command flags
pub mod flags {
    // SET options
    pub const NX: &[u8] = b"NX";      // Only set if not exists
    pub const XX: &[u8] = b"XX";      // Only set if exists
    pub const EX: &[u8] = b"EX";      // Expire in seconds
    pub const PX: &[u8] = b"PX";      // Expire in milliseconds
    pub const EXAT: &[u8] = b"EXAT";  // Expire at Unix timestamp (seconds)
    pub const PXAT: &[u8] = b"PXAT";  // Expire at Unix timestamp (milliseconds)
    pub const KEEPTTL: &[u8] = b"KEEPTTL";  // Retain existing TTL
    pub const GET: &[u8] = b"GET";    // Return old value
    pub const IFEQ: &[u8] = b"IFEQ";  // Set if current value equals
    pub const IFGT: &[u8] = b"IFGT";  // Set if current value greater than
    
    // EXPIRE options
    pub const GT: &[u8] = b"GT";      // Only set if new TTL > current
    pub const LT: &[u8] = b"LT";      // Only set if new TTL < current
    
    // ZADD options
    pub const CH: &[u8] = b"CH";      // Return changed count
    pub const INCR: &[u8] = b"INCR";  // Act like ZINCRBY
    
    // Sorting
    pub const ASC: &[u8] = b"ASC";
    pub const DESC: &[u8] = b"DESC";
    pub const ALPHA: &[u8] = b"ALPHA";
    pub const LIMIT: &[u8] = b"LIMIT";
    pub const BY: &[u8] = b"BY";
    pub const STORE: &[u8] = b"STORE";
    
    // SCAN family
    pub const MATCH: &[u8] = b"MATCH";
    pub const COUNT: &[u8] = b"COUNT";
    pub const TYPE: &[u8] = b"TYPE";
    
    // XREAD/XREADGROUP
    pub const BLOCK: &[u8] = b"BLOCK";
    pub const STREAMS: &[u8] = b"STREAMS";
    pub const NOACK: &[u8] = b"NOACK";
    pub const GROUP: &[u8] = b"GROUP";
    
    // GEO
    pub const WITHCOORD: &[u8] = b"WITHCOORD";
    pub const WITHDIST: &[u8] = b"WITHDIST";
    pub const WITHHASH: &[u8] = b"WITHHASH";
    pub const ANY: &[u8] = b"ANY";
    
    // Aggregation
    pub const AGGREGATE: &[u8] = b"AGGREGATE";
    pub const SUM: &[u8] = b"SUM";
    pub const MIN: &[u8] = b"MIN";
    pub const MAX: &[u8] = b"MAX";
    pub const WEIGHTS: &[u8] = b"WEIGHTS";
    
    // List operations
    pub const LEFT: &[u8] = b"LEFT";
    pub const RIGHT: &[u8] = b"RIGHT";
    pub const BEFORE: &[u8] = b"BEFORE";
    pub const AFTER: &[u8] = b"AFTER";
    
    // Generic
    pub const WITHSCORES: &[u8] = b"WITHSCORES";
    pub const WITHVALUES: &[u8] = b"WITHVALUES";
    pub const REV: &[u8] = b"REV";
    pub const BYSCORE: &[u8] = b"BYSCORE";
    pub const BYLEX: &[u8] = b"BYLEX";
    pub const REPLACE: &[u8] = b"REPLACE";
    pub const ABSTTL: &[u8] = b"ABSTTL";
    pub const IDLETIME: &[u8] = b"IDLETIME";
    pub const FREQ: &[u8] = b"FREQ";
    pub const ASYNC: &[u8] = b"ASYNC";
    pub const SYNC: &[u8] = b"SYNC";
    
    // Unit flags (distance)
    pub const M: &[u8] = b"M";        // Meters
    pub const KM: &[u8] = b"KM";      // Kilometers
    pub const FT: &[u8] = b"FT";      // Feet
    pub const MI: &[u8] = b"MI";      // Miles
}
```

---

## Testing and Validation

### Unit Test Patterns

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_string() {
        let mut buf = BytesMut::from("+OK\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::SimpleString(s)) => {
                assert_eq!(s, "OK");
            }
            other => panic!("Unexpected result: {:?}", other),
        }
        
        assert!(buf.is_empty());
    }
    
    #[test]
    fn test_parse_error() {
        let mut buf = BytesMut::from("-ERR unknown command 'foo'\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Error(e)) => {
                assert_eq!(e.kind, ErrorKind::Err);
                assert!(e.message.contains("unknown command"));
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_parse_integer() {
        let mut buf = BytesMut::from(":1000\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Integer(n)) => {
                assert_eq!(n, 1000);
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_parse_bulk_string() {
        let mut buf = BytesMut::from("$5\r\nhello\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::BulkString(Some(data))) => {
                assert_eq!(data, b"hello");
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_parse_null_bulk_string() {
        let mut buf = BytesMut::from("$-1\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::BulkString(None)) => {}
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_parse_array() {
        let mut buf = BytesMut::from("*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Array(Some(elements))) => {
                assert_eq!(elements.len(), 2);
                assert_eq!(elements[0], RespValue::BulkString(Some(b"foo".to_vec())));
                assert_eq!(elements[1], RespValue::BulkString(Some(b"bar".to_vec())));
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_parse_nested_array() {
        let mut buf = BytesMut::from("*2\r\n*2\r\n:1\r\n:2\r\n*2\r\n:3\r\n:4\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Array(Some(outer))) => {
                assert_eq!(outer.len(), 2);
                
                if let RespValue::Array(Some(inner1)) = &outer[0] {
                    assert_eq!(inner1[0], RespValue::Integer(1));
                    assert_eq!(inner1[1], RespValue::Integer(2));
                } else {
                    panic!("Expected inner array 1");
                }
                
                if let RespValue::Array(Some(inner2)) = &outer[1] {
                    assert_eq!(inner2[0], RespValue::Integer(3));
                    assert_eq!(inner2[1], RespValue::Integer(4));
                } else {
                    panic!("Expected inner array 2");
                }
            }
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_parse_incomplete() {
        let mut buf = BytesMut::from("$5\r\nhel");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Incomplete(needed) => {
                assert!(needed > 0);
            }
            other => panic!("Expected Incomplete, got: {:?}", other),
        }
    }
    
    #[test]
    fn test_encode_command() {
        let mut buf = BytesMut::new();
        encode_command(&[b"SET", b"key", b"value"], &mut buf);
        
        assert_eq!(&buf[..], b"*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n");
    }
    
    #[test]
    fn test_slot_calculation() {
        // Test basic slot
        assert_eq!(slot(b"foo"), 12182);
        
        // Test hash tag
        assert_eq!(slot(b"{user1000}.following"), slot(b"{user1000}.followers"));
        assert_ne!(slot(b"user1000.following"), slot(b"user1000.followers"));
        
        // Empty hash tag should use full key
        assert_eq!(slot(b"{}foo"), slot(b"{}foo"));
    }
    
    #[test]
    fn test_resp3_null() {
        let mut buf = BytesMut::from("_\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Null) => {}
            other => panic!("Expected Null, got: {:?}", other),
        }
    }
    
    #[test]
    fn test_resp3_boolean() {
        let mut buf = BytesMut::from("#t\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Boolean(true)) => {}
            other => panic!("Expected Boolean(true), got: {:?}", other),
        }
        
        let mut buf = BytesMut::from("#f\r\n");
        parser.reset();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Boolean(false)) => {}
            other => panic!("Expected Boolean(false), got: {:?}", other),
        }
    }
    
    #[test]
    fn test_resp3_double() {
        let test_cases = vec![
            (",1.23\r\n", 1.23),
            (",-1.23\r\n", -1.23),
            (",1.23e10\r\n", 1.23e10),
        ];
        
        for (input, expected) in test_cases {
            let mut buf = BytesMut::from(input);
            let mut parser = StreamingParser::new();
            
            match parser.parse(&mut buf) {
                ParseResult::Complete(RespValue::Double(d)) => {
                    assert!((d - expected).abs() < 1e-10);
                }
                other => panic!("Expected Double, got: {:?}", other),
            }
        }
    }
    
    #[test]
    fn test_resp3_map() {
        let mut buf = BytesMut::from("%2\r\n$4\r\nkey1\r\n:1\r\n$4\r\nkey2\r\n:2\r\n");
        let mut parser = StreamingParser::new();
        
        match parser.parse(&mut buf) {
            ParseResult::Complete(RespValue::Map(pairs)) => {
                assert_eq!(pairs.len(), 2);
            }
            other => panic!("Expected Map, got: {:?}", other),
        }
    }
}
```

### Integration Test Patterns

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tokio::runtime::Runtime;
    
    fn get_redis_url() -> String {
        std::env::var("REDIS_URL").unwrap_or_else(|_| "127.0.0.1:6379".to_string())
    }
    
    #[test]
    fn test_basic_operations() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = RedisClient::connect(&get_redis_url()).await.unwrap();
            
            // SET and GET
            client.set(b"test_key", b"test_value").await.unwrap();
            let value = client.get(b"test_key").await.unwrap();
            assert_eq!(value, Some(b"test_value".to_vec()));
            
            // DEL
            let deleted = client.del(&[b"test_key"]).await.unwrap();
            assert_eq!(deleted, 1);
            
            // GET after delete
            let value = client.get(b"test_key").await.unwrap();
            assert_eq!(value, None);
        });
    }
    
    #[test]
    fn test_pipelining() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let client = RedisClient::connect(&get_redis_url()).await.unwrap();
            
            let results = Pipeline::new()
                .set(b"pipe1", b"value1")
                .set(b"pipe2", b"value2")
                .get(b"pipe1")
                .get(b"pipe2")
                .incr(b"counter")
                .execute(&client)
                .await
                .unwrap();
            
            assert_eq!(results.len(), 5);
        });
    }
    
    #[test]
    fn test_transaction() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let stream = TcpStream::connect(&get_redis_url()).await.unwrap();
            let mut conn = Connection::new(stream).await;
            
            let results = Transaction::new()
                .cmd(&[b"SET", b"tx1", b"value1"])
                .cmd(&[b"SET", b"tx2", b"value2"])
                .cmd(&[b"GET", b"tx1"])
                .execute(&mut conn)
                .await
                .unwrap();
            
            assert_eq!(results.len(), 3);
            assert_eq!(results[2], RespValue::BulkString(Some(b"value1".to_vec())));
        });
    }
}
```

### Fuzzing

```rust
// Add to Cargo.toml:
// [dev-dependencies]
// arbitrary = { version = "1", features = ["derive"] }
// libfuzzer-sys = "0.4"

#![no_main]
use libfuzzer_sys::fuzz_target;
use bytes::BytesMut;

fuzz_target!(|data: &[u8]| {
    let mut buf = BytesMut::from(data);
    let mut parser = StreamingParser::new();
    
    // Parser should never panic on arbitrary input
    let _ = parser.parse(&mut buf);
});
```

---

## Appendix: Quick Reference Card

### Type Prefixes

```
RESP2:
+  Simple String    -  Error           :  Integer
$  Bulk String      *  Array

RESP3 Additional:
_  Null             #  Boolean         ,  Double
(  Big Number       !  Bulk Error      =  Verbatim String
%  Map              ~  Set             |  Attribute
>  Push
```

### Common Response Patterns

```text
OK Response:        +OK\r\n
Error Response:     -ERR message\r\n
Integer Response:   :42\r\n
Null (RESP2):       $-1\r\n
Null (RESP3):       _\r\n
Empty Array:        *0\r\n
Empty String:       $0\r\n\r\n
```

### Essential Cargo.toml Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
bytes = "1"
thiserror = "1"
tracing = "0.1"

[dev-dependencies]
tokio-test = "0.4"
```

---

*Document Version: 1.0*
*Last Updated: 2024*
*Covers: Redis 7.x, RESP2, RESP3*
