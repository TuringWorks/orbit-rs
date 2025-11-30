# RESP Protocol Fix Implementation Plan

**Created:** 2025-11-22
**Status:** Ready for Implementation
**Priority:** High

## Overview

This document outlines the specific code changes needed to fix the 15 failing RESP protocol tests.

---

## Phase 1: Critical Fixes (Estimated: 2-3 hours)

### Fix 1.1: DEL Command Serialization Error

**File:** `orbit/protocols/src/resp/commands/string.rs` (and similar files)
**Line:** ~116-135

**Current Issue:**
The `cmd_del` function is returning the deleted value instead of the count of deleted keys.

**Current Code Pattern:**
```rust
async fn cmd_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let mut count = 0;
    for arg in args {
        let key = self.get_string_arg(&[arg.clone()], 0, "DEL")?;
        let result = self.invoke_actor("delete", &key).await?;  // Returns the value
        if result != RespValue::Null {
            count += 1;
        }
    }
    Ok(RespValue::Integer(count))
}
```

**Problem:** The actor invocation is returning `null` for deleted keys, causing serialization error.

**Required Fix:**
```rust
async fn cmd_del(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let mut count = 0;
    for arg in args {
        let key = self.get_string_arg(&[arg.clone()], 0, "DEL")?;
        // Check if key exists first
        let exists = self.invoke_actor("exists", &key).await?;
        if matches!(exists, RespValue::Integer(1)) {
            let _ = self.invoke_actor("delete", &key).await;  // Ignore return value
            count += 1;
        }
    }
    Ok(RespValue::Integer(count))
}
```

**Files to Update:**
- `orbit/protocols/src/resp/commands/string.rs`
- `orbit/protocols/src/resp/commands/string_simple.rs`
- `orbit/protocols/src/resp/commands/string_persistent.rs`

---

### Fix 1.2: EXPIRE Command Not Registered

**File:** `orbit/protocols/src/resp/commands/mod.rs`
**Line:** ~202 (command routing)

**Current Issue:**
EXPIRE command is listed in the routing but the handler is not implemented.

**Current Code:**
```rust
"GET" | "SET" | "DEL" | "EXISTS" | "TTL" | "EXPIRE" | "KEYS" | "APPEND"
    | "STRLEN" | "INCR" | "DECR" | "INCRBY" | "DECRBY" | "GETSET" | "SETNX"
    | "SETEX" | "MGET" | "MSET" | "MSETNX" => {
        string_handler.handle_command(cmd, args).await
    }
```

**Problem:** The string handler's `handle_command` doesn't have EXPIRE case.

**Required Fix:**

1. Add to `orbit/protocols/src/resp/commands/string.rs`:
```rust
async fn cmd_expire(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    if args.len() != 2 {
        return Err(ProtocolError::InvalidCommand(
            "ERR wrong number of arguments for 'expire' command".to_string(),
        ));
    }

    let key = self.get_string_arg(args, 0, "EXPIRE")?;
    let seconds = self.get_int_arg(args, 1, "EXPIRE")?;

    // Check if key exists
    let exists = self.invoke_actor("exists", &key).await?;
    if !matches!(exists, RespValue::Integer(1)) {
        return Ok(RespValue::Integer(0));
    }

    // Set expiration
    let expiration_ms = seconds * 1000;
    self.invoke_actor("expire", &format!("{}:{}", key, expiration_ms)).await?;
    Ok(RespValue::Integer(1))
}
```

2. Add to match statement in `handle_command`:
```rust
"EXPIRE" => self.cmd_expire(args).await,
```

---

## Phase 2: List Command Fixes (Estimated: 3-4 hours)

### Fix 2.1: LLEN Returns 0

**File:** `orbit/protocols/src/resp/commands/list.rs`
**Function:** `cmd_llen`

**Current Issue:** Not returning actual list length

**Investigation Needed:**
- Check how list length is stored in actor
- Verify actor response format
- Ensure proper integer conversion

**Expected Fix Pattern:**
```rust
async fn cmd_llen(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let key = self.get_string_arg(args, 0, "LLEN")?;
    let result = self.invoke_actor("llen", &key).await?;

    // Convert actor response to integer
    match result {
        RespValue::Integer(len) => Ok(RespValue::Integer(len)),
        RespValue::BulkString(s) => {
            let len: i64 = s.parse().unwrap_or(0);
            Ok(RespValue::Integer(len))
        }
        _ => Ok(RespValue::Integer(0))
    }
}
```

---

### Fix 2.2: RPUSH Serialization Error

**File:** `orbit/protocols/src/resp/commands/list.rs`
**Function:** `cmd_rpush`

**Current Issue:** Returning string instead of integer length

**Expected Fix:**
```rust
async fn cmd_rpush(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let key = self.get_string_arg(args, 0, "RPUSH")?;
    let mut length = 0;

    for value in &args[1..] {
        let val_str = self.get_string_arg(&[value.clone()], 0, "RPUSH")?;
        let _ = self.invoke_actor("rpush", &format!("{}:{}", key, val_str)).await?;
        length += 1;
    }

    // Return total length after pushes
    let final_len = self.invoke_actor("llen", &key).await?;
    Ok(final_len)  // Should already be Integer
}
```

---

### Fix 2.3: LRANGE Invalid Argument

**File:** `orbit/protocols/src/resp/commands/list.rs`
**Function:** `cmd_lrange`

**Current Issue:** Integer argument parsing fails

**Investigation:** Check `get_int_arg` implementation

**Expected Fix:**
```rust
async fn cmd_lrange(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let key = self.get_string_arg(args, 0, "LRANGE")?;

    // Parse integers more robustly
    let start = match &args[1] {
        RespValue::BulkString(s) => s.parse::<i64>()
            .map_err(|_| ProtocolError::InvalidCommand(
                "ERR invalid start index".to_string()
            ))?,
        RespValue::Integer(i) => *i,
        _ => return Err(ProtocolError::InvalidCommand(
            "ERR invalid start index type".to_string()
        ))
    };

    let end = match &args[2] {
        RespValue::BulkString(s) => s.parse::<i64>()
            .map_err(|_| ProtocolError::InvalidCommand(
                "ERR invalid end index".to_string()
            ))?,
        RespValue::Integer(i) => *i,
        _ => return Err(ProtocolError::InvalidCommand(
            "ERR invalid end index type".to_string()
        ))
    };

    let result = self.invoke_actor("lrange", &format!("{}:{}:{}", key, start, end)).await?;
    Ok(result)
}
```

---

### Fix 2.4: LINDEX Invalid Argument

**File:** `orbit/protocols/src/resp/commands/list.rs`
**Function:** `cmd_lindex`

**Similar fix to LRANGE** - robust integer parsing

---

### Fix 2.5: LPOP Returns Null

**File:** `orbit/protocols/src/resp/commands/list.rs`
**Function:** `cmd_lpop`

**Current Issue:** Not returning the popped value

**Expected Fix:**
```rust
async fn cmd_lpop(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let key = self.get_string_arg(args, 0, "LPOP")?;

    // First check if list exists and has items
    let len_result = self.invoke_actor("llen", &key).await?;
    if matches!(len_result, RespValue::Integer(0)) {
        return Ok(RespValue::Null);
    }

    // Pop and return value
    let result = self.invoke_actor("lpop", &key).await?;
    Ok(result)  // Should return the actual value
}
```

---

## Phase 3: Set Command Fixes (Estimated: 2 hours)

### Fix 3.1: SCARD Returns 0

**File:** `orbit/protocols/src/resp/commands/set.rs`
**Function:** `cmd_scard`

**Similar to LLEN fix** - ensure proper length retrieval from actor

---

### Fix 3.2: SISMEMBER Returns 0

**File:** `orbit/protocols/src/resp/commands/set.rs`
**Function:** `cmd_sismember`

**Expected Fix:**
```rust
async fn cmd_sismember(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    let key = self.get_string_arg(args, 0, "SISMEMBER")?;
    let member = self.get_string_arg(args, 1, "SISMEMBER")?;

    let result = self.invoke_actor("sismember", &format!("{}:{}", key, member)).await?;

    // Ensure we return 1 or 0
    match result {
        RespValue::Integer(i) => Ok(RespValue::Integer(if i > 0 { 1 } else { 0 })),
        RespValue::BulkString(s) if s == "1" || s.to_lowercase() == "true" => {
            Ok(RespValue::Integer(1))
        }
        _ => Ok(RespValue::Integer(0))
    }
}
```

---

## Phase 4: Sorted Set Implementation (Estimated: 4-6 hours)

### Implementation Files

**File:** `orbit/protocols/src/resp/commands/sorted_set.rs`

Currently returns "not yet implemented" errors. Need to implement:

1. **ZADD** - Add members with scores
2. **ZCARD** - Get cardinality
3. **ZSCORE** - Get score of member
4. **ZRANGE** - Get range of members
5. **ZINCRBY** - Increment score
6. **ZREM** - Remove members

**Data Structure Needed:**
- Sorted set backed by TreeMap or BTreeMap
- Store as `{key: {member: score, ...}}`
- Maintain sort order by score

**Basic Implementation Template:**
```rust
async fn cmd_zadd(&self, args: &[RespValue]) -> ProtocolResult<RespValue> {
    if args.len() < 3 || (args.len() - 1) % 2 != 0 {
        return Err(ProtocolError::InvalidCommand(
            "ERR wrong number of arguments for 'zadd' command".to_string(),
        ));
    }

    let key = self.get_string_arg(args, 0, "ZADD")?;
    let mut added = 0;

    // Process score-member pairs
    for i in (1..args.len()).step_by(2) {
        let score = self.get_float_arg(args, i, "ZADD")?;
        let member = self.get_string_arg(args, i + 1, "ZADD")?;

        let payload = format!("{}:{}:{}", key, member, score);
        let result = self.invoke_actor("zadd", &payload).await?;

        if matches!(result, RespValue::Integer(1)) {
            added += 1;
        }
    }

    Ok(RespValue::Integer(added))
}
```

---

## Testing Strategy

### Unit Tests
Add to each fixed command file:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_del_returns_count() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_expire_sets_ttl() {
        // Test implementation
    }
}
```

### Integration Tests
Run `resp-comprehensive-test` after each phase to verify fixes.

---

## Implementation Order

1. ✅ Create bug report (COMPLETED)
2. ✅ Create fix plan (COMPLETED)
3. ⏳ Phase 1.1: Fix DEL command
4. ⏳ Phase 1.2: Fix EXPIRE command
5. ⏳ Test Phase 1 (run resp-comprehensive-test)
6. ⏳ Phase 2: Fix all list commands
7. ⏳ Test Phase 2
8. ⏳ Phase 3: Fix set commands
9. ⏳ Test Phase 3
10. ⏳ Phase 4: Implement sorted sets
11. ⏳ Final integration test
12. ⏳ Document fixes and update RESP_TEST_FAILURES.md

---

## Success Criteria

- All 15 failing tests pass
- Success rate increases from 71.7% to 100%
- No regression in currently passing tests (38 tests)
- Clean compilation with no warnings

---

## Estimated Total Time: 11-15 hours

- Phase 1: 2-3 hours
- Phase 2: 3-4 hours
- Phase 3: 2 hours
- Phase 4: 4-6 hours

---

## Next Steps

Run this command to start fixing:
```bash
cd /Users/ravindraboddipalli/sources/git/orbit-rs/orbit/protocols
```

Then edit files in order of phases defined above.
