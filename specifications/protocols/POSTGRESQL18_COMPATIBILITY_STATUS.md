# PostgreSQL 18 Compatibility Status

**Last Updated**: 2025-12-07
**Purpose**: Track OrbitRS implementation status of PostgreSQL 18 features
**Reference**: See [postgresql18-reference-rust.md](./Protocol-specs/postgresql18-reference-rust.md) for full PostgreSQL 18 specification

---

## Executive Summary

OrbitRS implements PostgreSQL wire protocol (v3.0) with extensive SQL support. This document tracks compatibility with PostgreSQL 18 features released September 2025.

| Category | Implemented | Partial | Not Started | Total |
|----------|-------------|---------|-------------|-------|
| Wire Protocol | 15 | 2 | 1 | 18 |
| SQL Syntax (PG18 New) | 4 | 1 | 2 | 7 |
| Functions (PG18 New) | 6 | 0 | 0 | 6 |
| Data Types | 25+ | 3 | 2 | 30+ |

---

## 1. Wire Protocol Support

### Frontend (Client) Messages

| Message | Type Byte | Status | Notes |
|---------|-----------|--------|-------|
| StartupMessage | (none) | ✅ Implemented | Protocol 3.0 |
| PasswordMessage | `p` | ✅ Implemented | MD5, SCRAM-SHA-256 |
| Query | `Q` | ✅ Implemented | Simple query protocol |
| Parse | `P` | ✅ Implemented | Extended query |
| Bind | `B` | ✅ Implemented | Parameter binding |
| Execute | `E` | ✅ Implemented | Portal execution |
| Describe | `D` | ✅ Implemented | Statement/portal metadata |
| Close | `C` | ✅ Implemented | Close statement/portal |
| Sync | `S` | ✅ Implemented | Sync point |
| Flush | `H` | ✅ Implemented | Force result send |
| Terminate | `X` | ✅ Implemented | Connection close |
| CopyData | `d` | ⚠️ Partial | Basic COPY support |
| CopyDone | `c` | ⚠️ Partial | Basic COPY support |
| CopyFail | `f` | ✅ Implemented | Copy error handling |
| FunctionCall | `F` | ❌ Not Started | Legacy, rarely used |

### Backend (Server) Messages

| Message | Type Byte | Status | Notes |
|---------|-----------|--------|-------|
| Authentication* | `R` | ✅ Implemented | Multiple auth types |
| BackendKeyData | `K` | ✅ Implemented | Fixed 4-byte key (PG18 supports variable) |
| ReadyForQuery | `Z` | ✅ Implemented | Transaction status |
| RowDescription | `T` | ✅ Implemented | Column metadata |
| DataRow | `D` | ✅ Implemented | Result data |
| CommandComplete | `C` | ✅ Implemented | Affected row counts |
| ErrorResponse | `E` | ✅ Implemented | All error fields |
| NoticeResponse | `N` | ✅ Implemented | Warnings |
| ParameterStatus | `S` | ✅ Implemented | Runtime parameters |
| ParseComplete | `1` | ✅ Implemented | Extended query |
| BindComplete | `2` | ✅ Implemented | Extended query |
| CloseComplete | `3` | ✅ Implemented | Extended query |
| NoData | `n` | ✅ Implemented | No results |
| ParameterDescription | `t` | ✅ Implemented | Parameter types |
| EmptyQueryResponse | `I` | ✅ Implemented | Empty query |
| NotificationResponse | `A` | ⚠️ Partial | NOTIFY/LISTEN basic |
| CopyInResponse | `G` | ✅ Implemented | COPY IN |
| CopyOutResponse | `H` | ✅ Implemented | COPY OUT |
| NegotiateProtocolVersion | `v` | ❌ Not Started | Protocol 3.2 feature |

---

## 2. PostgreSQL 18 New SQL Features

### 2.1 UUIDv7 Generation

**Status**: ✅ Implemented

```sql
-- Generate timestamp-ordered UUID (PostgreSQL 18)
SELECT uuidv7();
SELECT uuid_generate_v7();

-- Use as primary key default
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    customer_id INT,
    created_at TIMESTAMP DEFAULT NOW()
);
```

**Implementation Location**: `orbit/server/src/protocols/postgres_wire/sql/expression_evaluator.rs`

**Functions Supported**:
| Function | Status | Notes |
|----------|--------|-------|
| `uuidv7()` | ✅ | PostgreSQL 18 timestamp-ordered UUID |
| `uuid_generate_v7()` | ✅ | Alias for uuidv7() |
| `gen_random_uuid()` | ✅ | Standard PostgreSQL v4 UUID |
| `uuid_generate_v4()` | ✅ | Alias for gen_random_uuid() |
| `uuid_nil()` | ✅ | All-zeros UUID |
| `uuid_max()` | ✅ | All-ones UUID |

---

### 2.2 Generated Columns (STORED and VIRTUAL)

**Status**: ✅ Implemented (STORED and VIRTUAL)

```sql
-- PostgreSQL 12+ syntax (STORED) - FULLY WORKING
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    price NUMERIC(10,2),
    quantity INT,
    total NUMERIC(10,2) GENERATED ALWAYS AS (price * quantity) STORED
);

-- PostgreSQL 18 syntax (VIRTUAL) - PARSING ONLY
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    price NUMERIC(10,2),
    quantity INT,
    total NUMERIC(10,2) GENERATED ALWAYS AS (price * quantity) VIRTUAL
);
```

**Implementation Status**:
| Component | Status | Notes |
|-----------|--------|-------|
| Lexer tokens (GENERATED, ALWAYS, STORED, VIRTUAL) | ✅ Done | Added to `lexer.rs` |
| AST types (ColumnConstraint::Generated, GeneratedColumnStorage) | ✅ Done | Added to `ast.rs` |
| DDL parsing | ✅ Done | Parses `GENERATED ALWAYS AS (expr) [STORED|VIRTUAL]` |
| Schema storage | ✅ Done | `GeneratedColumnSchema` stores expression and storage type |
| STORED column execution (INSERT) | ✅ Done | Auto-computes value on INSERT |
| STORED column execution (UPDATE) | ✅ Done | Re-computes value on UPDATE |
| Reject direct INSERT/UPDATE | ✅ Done | Error if user tries to set generated column |
| VIRTUAL column execution (SELECT) | ✅ Done | Computed on-the-fly during query execution |
| Unit tests | ✅ Done | Added 4 parsing tests, all passing |

**Implementation Location**:
- Lexer: `orbit/server/src/protocols/postgres_wire/sql/lexer.rs`
- AST: `orbit/server/src/protocols/postgres_wire/sql/ast.rs`
- Parser: `orbit/server/src/protocols/postgres_wire/sql/parser/ddl.rs`
- Schema: `orbit/server/src/protocols/postgres_wire/sql/executor.rs` (GeneratedColumnSchema, ColumnSchema)
- Execution: `orbit/server/src/protocols/postgres_wire/sql/executor.rs` (compute_generated_columns, compute_virtual_columns, execute_insert, execute_update, execute_single_table)
- Tests: `orbit/server/src/protocols/postgres_wire/sql/tests.rs`

---

### 2.3 Temporal Constraints (WITHOUT OVERLAPS)

**Status**: ❌ Not Started

```sql
-- PostgreSQL 18 temporal PRIMARY KEY
CREATE TABLE employee_positions (
    employee_id INT,
    department_id INT,
    valid_period TSTZRANGE,
    PRIMARY KEY (employee_id, valid_period WITHOUT OVERLAPS)
);

-- Temporal FOREIGN KEY
CREATE TABLE salary_history (
    employee_id INT,
    valid_period TSTZRANGE,
    salary NUMERIC(10,2),
    FOREIGN KEY (employee_id, PERIOD valid_period)
        REFERENCES employee_positions (employee_id, PERIOD valid_period)
);
```

**Implementation Required**:
1. Add `WithoutOverlaps` flag to `TableConstraint::PrimaryKey`
2. Add `PERIOD` keyword support for temporal foreign keys
3. Implement overlap checking in constraint validation
4. Range type support for temporal columns

---

### 2.4 OLD/NEW in RETURNING Clause

**Status**: ✅ Implemented

```sql
-- PostgreSQL 18: Access OLD values in UPDATE RETURNING
UPDATE users
SET email = 'new@example.com'
WHERE id = 1
RETURNING
    OLD.email AS previous_email,
    NEW.email AS current_email;

-- DELETE with OLD
DELETE FROM users
WHERE id = 1
RETURNING OLD.*;
```

**Implementation Status**:
| Component | Status | Notes |
|-----------|--------|-------|
| Lexer tokens (OLD, NEW) | ✅ Done | Added to `lexer.rs` |
| Parser support for OLD.column/NEW.column | ✅ Done | Added to expression parser and RETURNING clause parser |
| Parser support for OLD.*/NEW.* | ✅ Done | QualifiedWildcard handling in RETURNING |
| Executor tracking of old row values | ✅ Done | execute_update/execute_delete track pre-modification values |
| evaluate_returning_expr_with_old_new | ✅ Done | Expression evaluation with OLD/NEW context |
| evaluate_returning_clause_with_old_new | ✅ Done | Full RETURNING clause evaluation |
| Unit tests | ✅ Done | 5 parsing tests |

**Implementation Location**:
- Lexer: `orbit/server/src/protocols/postgres_wire/sql/lexer.rs`
- Expression Parser: `orbit/server/src/protocols/postgres_wire/sql/parser/expressions.rs`
- DML Parser (RETURNING): `orbit/server/src/protocols/postgres_wire/sql/parser/dml.rs`
- Utilities: `orbit/server/src/protocols/postgres_wire/sql/parser/utilities.rs`
- Executor: `orbit/server/src/protocols/postgres_wire/sql/executor.rs`
- Tests: `orbit/server/src/protocols/postgres_wire/sql/tests.rs`

---

### 2.5 MERGE Enhancements

**Status**: ⚠️ Partial (MERGE exists, RETURNING not supported)

```sql
-- PostgreSQL 18 MERGE with RETURNING
MERGE INTO target_table t
USING source_table s ON t.id = s.id
WHEN MATCHED THEN
    UPDATE SET value = s.value
    RETURNING OLD.value, NEW.value
WHEN NOT MATCHED THEN
    INSERT (id, value) VALUES (s.id, s.value)
    RETURNING *;
```

**Current Status**:
- ✅ Basic MERGE syntax parsed
- ✅ WHEN MATCHED / WHEN NOT MATCHED
- ❌ RETURNING clause in MERGE
- ✅ OLD/NEW references (implemented for UPDATE/DELETE, pending for MERGE)

---

## 3. Protocol Version 3.2 Changes

### 3.1 Variable-Length Cancellation Keys

**Status**: ❌ Not Started

PostgreSQL 18 (protocol 3.2) allows cancellation keys of 4-256 bytes. OrbitRS currently uses fixed 4-byte keys.

```rust
// Current implementation (fixed 4 bytes)
pub struct BackendKeyData {
    pub process_id: i32,
    pub secret_key: i32,  // Fixed 4 bytes
}

// PostgreSQL 18 (variable length)
pub struct BackendKeyData {
    pub process_id: i32,
    pub secret_key: Vec<u8>,  // 4-256 bytes
}
```

### 3.2 OAuth Authentication

**Status**: ❌ Not Started

PostgreSQL 18 introduces OAuth-based authentication.

**Current Authentication Methods**:
- ✅ Trust (no password)
- ✅ Password (cleartext)
- ✅ MD5
- ✅ SCRAM-SHA-256
- ❌ OAuth

### 3.3 Protocol Negotiation

**Status**: ❌ Not Started

PostgreSQL 18 supports protocol version negotiation via `NegotiateProtocolVersion` message.

---

## 4. Performance Features (PostgreSQL 18)

| Feature | Status | Notes |
|---------|--------|-------|
| Asynchronous I/O (io_uring) | ❌ | Linux-specific, not applicable |
| Skip scan for B-tree | ⚠️ | Different storage architecture |
| Parallel GIN index builds | ❌ | Not applicable to current storage |
| Preserved statistics on upgrade | N/A | OrbitRS doesn't use pg_upgrade |

---

## 5. Data Type Support

### Core Types

| Type | OID | Status | Notes |
|------|-----|--------|-------|
| boolean | 16 | ✅ | |
| bytea | 17 | ✅ | |
| char | 18 | ✅ | |
| int8 (bigint) | 20 | ✅ | |
| int2 (smallint) | 21 | ✅ | |
| int4 (integer) | 23 | ✅ | |
| text | 25 | ✅ | |
| oid | 26 | ✅ | |
| json | 114 | ✅ | |
| float4 (real) | 700 | ✅ | |
| float8 (double) | 701 | ✅ | |
| varchar | 1043 | ✅ | |
| date | 1082 | ✅ | |
| time | 1083 | ✅ | |
| timestamp | 1114 | ✅ | |
| timestamptz | 1184 | ✅ | |
| interval | 1186 | ✅ | |
| numeric | 1700 | ✅ | |
| uuid | 2950 | ✅ | |
| jsonb | 3802 | ✅ | |
| tsvector | 3614 | ⚠️ | Basic support |
| tsquery | 3615 | ⚠️ | Basic support |

### Extension Types

| Type | Status | Notes |
|------|--------|-------|
| vector | ✅ | pgvector compatible |
| inet | ✅ | Network addresses |
| cidr | ✅ | Network blocks |
| macaddr | ✅ | MAC addresses |
| tstzrange | ⚠️ | Partial (needed for temporal) |

---

## 6. Implementation Roadmap

### Phase 1: PostgreSQL 18 Core (High Priority)

| Task | Status | Effort |
|------|--------|--------|
| UUIDv7 function | ✅ Done | - |
| gen_random_uuid | ✅ Done | - |
| GENERATED ALWAYS AS (STORED) | ✅ Done | - |
| Variable-length cancel keys | Planned | Low |

### Phase 2: PostgreSQL 18 Advanced (Medium Priority)

| Task | Status | Effort |
|------|--------|--------|
| VIRTUAL generated columns | ✅ Done | - |
| OLD/NEW in RETURNING | ✅ Done | - |
| MERGE with RETURNING | Planned | Medium |

### Phase 3: PostgreSQL 18 Temporal (Lower Priority)

| Task | Status | Effort |
|------|--------|--------|
| WITHOUT OVERLAPS constraint | Planned | High |
| Temporal foreign keys | Planned | High |
| Range type improvements | Planned | Medium |

---

## 7. Testing

### Compatibility Tests

```bash
# Run PostgreSQL compatibility tests
cargo test -p orbit-server -- postgres

# Run specific PG18 feature tests
cargo test -p orbit-server -- uuid
cargo test -p orbit-server -- generated_column
```

### Test Coverage

| Feature | Unit Tests | Integration Tests |
|---------|------------|-------------------|
| UUIDv7 | ✅ | ✅ |
| gen_random_uuid | ✅ | ✅ |
| Wire protocol | ✅ | ✅ |
| GENERATED columns (parsing) | ✅ | ❌ |
| GENERATED columns (STORED exec) | ✅ | ❌ |
| GENERATED columns (VIRTUAL exec) | ✅ | ❌ |
| OLD/NEW in RETURNING | ✅ | ❌ |
| Temporal constraints | ❌ | ❌ |

---

## Version History

| Date | Changes |
|------|---------|
| 2025-12-07 | Implemented VIRTUAL generated columns (compute on SELECT) |
| 2025-12-07 | Implemented OLD/NEW table references in UPDATE/DELETE RETURNING |
| 2025-12-07 | Implemented STORED generated column execution (INSERT, UPDATE) |
| 2025-12-07 | Added unit tests for GENERATED columns and UUID functions |
| 2025-12-07 | Added GENERATED ALWAYS AS parsing (STORED and VIRTUAL) |
| 2025-12-07 | Added UUIDv7, gen_random_uuid, uuid_nil, uuid_max functions |
| 2025-12-07 | Initial PostgreSQL 18 compatibility status document |
