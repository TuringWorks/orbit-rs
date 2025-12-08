# PostgreSQL 18 Protocol & SQL Syntax Reference

A comprehensive reference for LLM coding tools (like Claude Code) to create and maintain PostgreSQL-related code, including wire protocol implementation, SQL parsing, and client library development.

---

## Table of Contents

1. [Overview](#overview)
2. [Wire Protocol (v3.2)](#wire-protocol-v32)
3. [Message Formats](#message-formats)
4. [SQL Keywords](#sql-keywords)
5. [Parser Architecture](#parser-architecture)
6. [Abstract Syntax Tree (AST)](#abstract-syntax-tree-ast)
7. [Data Types and OIDs](#data-types-and-oids)
8. [Extended Query Protocol](#extended-query-protocol)
9. [Implementation Libraries](#implementation-libraries)
10. [PostgreSQL 18 New Features](#postgresql-18-new-features)

---

## Overview

PostgreSQL 18 was released on September 25, 2025, introducing protocol version 3.2. This document provides the technical specifications needed to implement PostgreSQL clients, parsers, and tools.

### Key Version Information

| Component | Version |
|-----------|---------|
| PostgreSQL | 18.1 (current as of Nov 2025) |
| Protocol | 3.2 |
| Default Port | 5432 |
| Supported Formats | Text (0), Binary (1) |

---

## Wire Protocol (v3.2)

PostgreSQL uses a message-based protocol for communication between frontends (clients) and backends (servers). The protocol operates over TCP/IP or Unix-domain sockets.

### Protocol Phases

1. **Startup Phase**: Connection establishment and authentication
2. **Normal Operation**: Query execution and results
3. **Termination**: Clean connection closure

### Message Structure

```text
┌─────────────┬──────────────┬─────────────────┐
│ Message Type│ Length (4B)  │ Message Body    │
│ (1 byte)    │ includes self│ (variable)      │
└─────────────┴──────────────┴─────────────────┘
```

**Note**: The startup message has no initial message-type byte (for historical reasons).

### Byte Ordering

All multi-byte integers use **network byte order (big-endian)**.

### Protocol Version Encoding

```text
Major version: Upper 16 bits
Minor version: Lower 16 bits

Protocol 3.2 = 0x00030002 = 196610
Protocol 3.0 = 0x00030000 = 196608
```

---

## Message Formats

### Frontend (Client) Messages

| Type Byte | Message | Description |
|-----------|---------|-------------|
| (none) | StartupMessage | Initial connection request |
| `p` | PasswordMessage | Password authentication response |
| `Q` | Query | Simple query |
| `P` | Parse | Extended query: parse statement |
| `B` | Bind | Extended query: bind parameters |
| `E` | Execute | Extended query: execute portal |
| `D` | Describe | Describe statement/portal |
| `C` | Close | Close statement/portal |
| `S` | Sync | Synchronization point |
| `H` | Flush | Force backend to send results |
| `X` | Terminate | Connection termination |
| `d` | CopyData | COPY data |
| `c` | CopyDone | COPY completion |
| `f` | CopyFail | COPY failure |
| `F` | FunctionCall | Function call (legacy) |

### Backend (Server) Messages

| Type Byte | Message | Description |
|-----------|---------|-------------|
| `R` | Authentication* | Various authentication messages |
| `K` | BackendKeyData | Cancellation key data |
| `Z` | ReadyForQuery | Ready for new query |
| `T` | RowDescription | Column metadata |
| `D` | DataRow | Result row data |
| `C` | CommandComplete | Command completion |
| `E` | ErrorResponse | Error message |
| `N` | NoticeResponse | Warning/notice |
| `S` | ParameterStatus | Runtime parameter value |
| `1` | ParseComplete | Parse completed |
| `2` | BindComplete | Bind completed |
| `3` | CloseComplete | Close completed |
| `n` | NoData | No data returned |
| `t` | ParameterDescription | Parameter types |
| `I` | EmptyQueryResponse | Empty query string |
| `A` | NotificationResponse | NOTIFY message |
| `G` | CopyInResponse | Start COPY IN |
| `H` | CopyOutResponse | Start COPY OUT |
| `W` | CopyBothResponse | Streaming replication |
| `v` | NegotiateProtocolVersion | Protocol negotiation |
| `s` | PortalSuspended | Execute row limit reached |

### Detailed Message Specifications

#### StartupMessage (Frontend)

```text
Int32       Length (including self)
Int32       Protocol version (196610 for 3.2)
String      Parameter name (e.g., "user")
String      Parameter value
...         (more name/value pairs)
Byte1       '\0' (terminator)
```

**Required Parameters**:

- `user`: Database user name

**Optional Parameters**:

- `database`: Database name (defaults to user name)
- `options`: Command-line arguments for backend
- `replication`: Streaming replication mode (`true`, `false`, `database`)
- `_pq_.*`: Reserved for protocol extensions

#### Query (Frontend) - Simple Query Protocol

```text
Byte1       'Q'
Int32       Length
String      SQL query string (null-terminated)
```

#### Parse (Frontend) - Extended Query Protocol

```text
Byte1       'P'
Int32       Length
String      Prepared statement name (empty = unnamed)
String      Query string
Int16       Number of parameter type OIDs
Int32[n]    Parameter type OIDs (0 = unspecified)
```

#### Bind (Frontend)

```text
Byte1       'B'
Int32       Length
String      Destination portal name (empty = unnamed)
String      Source prepared statement name
Int16       Number of parameter format codes (C)
Int16[C]    Parameter format codes (0=text, 1=binary)
Int16       Number of parameter values
For each parameter:
  Int32     Value length (-1 for NULL)
  Byte[n]   Value data
Int16       Number of result format codes (R)
Int16[R]    Result format codes
```

#### Execute (Frontend)

```text
Byte1       'E'
Int32       Length
String      Portal name (empty = unnamed)
Int32       Maximum rows (0 = no limit)
```

#### Describe (Frontend)

```text
Byte1       'D'
Int32       Length
Byte1       'S' (statement) or 'P' (portal)
String      Name (empty = unnamed)
```

#### Sync (Frontend)

```text
Byte1       'S'
Int32       4 (length)
```

#### RowDescription (Backend)

```text
Byte1       'T'
Int32       Length
Int16       Number of fields
For each field:
  String    Field name
  Int32     Table OID (0 if not a table column)
  Int16     Column attribute number (0 if not a column)
  Int32     Data type OID
  Int16     Data type size (negative = variable)
  Int32     Type modifier
  Int16     Format code (0=text, 1=binary)
```

#### DataRow (Backend)

```text
Byte1       'D'
Int32       Length
Int16       Number of columns
For each column:
  Int32     Value length (-1 for NULL)
  Byte[n]   Value data
```

#### ErrorResponse / NoticeResponse (Backend)

```text
Byte1       'E' or 'N'
Int32       Length
Repeated:
  Byte1     Field type code
  String    Field value
Byte1       '\0' (terminator)
```

**Error Field Codes**:

| Code | Meaning |
|------|---------|
| `S` | Severity (ERROR, FATAL, PANIC, WARNING, NOTICE, DEBUG, INFO, LOG) |
| `V` | Severity (non-localized) |
| `C` | SQLSTATE code |
| `M` | Message text |
| `D` | Detail |
| `H` | Hint |
| `P` | Position (character offset) |
| `p` | Internal position |
| `q` | Internal query |
| `W` | Where (context) |
| `s` | Schema name |
| `t` | Table name |
| `c` | Column name |
| `d` | Data type name |
| `n` | Constraint name |
| `F` | File name |
| `L` | Line number |
| `R` | Routine name |

#### ReadyForQuery (Backend)

```text
Byte1       'Z'
Int32       5 (length)
Byte1       Transaction status:
            'I' = idle (not in transaction)
            'T' = in transaction block
            'E' = in failed transaction block
```

#### BackendKeyData (Backend)

```text
Byte1       'K'
Int32       Length (12 for protocol 3.0, variable for 3.2)
Int32       Process ID
Byte[n]     Secret key (4-256 bytes, typically 32 in PG18)
```

**Note**: In protocol 3.2, the secret key can be 4-256 bytes (was fixed 4 bytes in 3.0).

#### CancelRequest

```text
Int32       16 (length)
Int32       80877102 (cancel request code: 1234 << 16 | 5678)
Int32       Process ID
Int32       Secret key (first 4 bytes)
```

#### SSLRequest

```text
Int32       8 (length)
Int32       80877103 (SSL request code: 1234 << 16 | 5679)
```

Server responds with single byte: `S` (SSL supported) or `N` (not supported).

#### GSSENCRequest (Protocol 3.2)

```text
Int32       8 (length)
Int32       80877104 (GSSENC request code: 1234 << 16 | 5680)
```

---

## SQL Keywords

PostgreSQL 18 has several keyword categories:

### Keyword Classifications

| Classification | Description |
|---------------|-------------|
| **Reserved** | Cannot be used as identifiers (except as column labels) |
| **Non-reserved** | Can be used as table/column names |
| **Cannot be function/type** | Non-reserved but restricted for functions/types |
| **Requires AS** | Need explicit AS for column aliases |

### Reserved Keywords (PostgreSQL 18)

```text
ALL             ANALYSE         ANALYZE         AND
ANY             ARRAY           AS              ASC
ASYMMETRIC      AUTHORIZATION   BINARY          BOTH
CASE            CAST            CHECK           COLLATE
COLLATION       COLUMN          CONCURRENTLY    CONSTRAINT
CREATE          CROSS           CURRENT_CATALOG CURRENT_DATE
CURRENT_ROLE    CURRENT_SCHEMA  CURRENT_TIME    CURRENT_TIMESTAMP
CURRENT_USER    DEFAULT         DEFERRABLE      DESC
DISTINCT        DO              ELSE            END
EXCEPT          FALSE           FETCH           FOR
FOREIGN         FREEZE          FROM            FULL
GRANT           GROUP           HAVING          ILIKE
IN              INITIALLY       INNER           INTERSECT
INTO            IS              ISNULL          JOIN
LATERAL         LEADING         LEFT            LIKE
LIMIT           LOCALTIME       LOCALTIMESTAMP  NATURAL
NOT             NOTNULL         NULL            OFFSET
ON              ONLY            OR              ORDER
OUTER           OVERLAPS        PLACING         PRIMARY
REFERENCES      RETURNING       RIGHT           SELECT
SESSION_USER    SIMILAR         SOME            SYMMETRIC
SYSTEM_USER     TABLE           TABLESAMPLE     THEN
TO              TRAILING        TRUE            UNION
UNIQUE          USER            USING           VARIADIC
VERBOSE         WHEN            WHERE           WINDOW
WITH
```

### Non-Reserved Keywords (Common)

```text
ABORT           ABSOLUTE        ACCESS          ACTION
ADD             ADMIN           AFTER           AGGREGATE
ALSO            ALTER           ALWAYS          ASSERTION
ASSIGNMENT      AT              ATOMIC          ATTACH
ATTRIBUTE       BACKWARD        BEFORE          BEGIN
BY              CACHE           CALL            CALLED
CASCADE         CASCADED        CATALOG         CHAIN
CHARACTERISTICS CHECKPOINT      CLASS           CLOSE
CLUSTER         COALESCE        COLLATION       COLUMN
COLUMNS         COMMENT         COMMENTS        COMMIT
COMMITTED       COMPRESSION     CONFIGURATION   CONFLICT
CONNECTION      CONSTRAINT      CONSTRAINTS     CONTENT
CONTINUE        CONVERSION      COPY            COST
CSV             CUBE            CURRENT         CURSOR
CYCLE           DATA            DATABASE        DAY
DEALLOCATE      DECLARE         DEFAULTS        DEFERRED
DEFINER         DELETE          DELIMITER       DELIMITERS
DEPENDS         DEPTH           DETACH          DICTIONARY
DISABLE         DISCARD         DOCUMENT        DOMAIN
DOUBLE          DROP            EACH            ENABLE
ENCODING        ENCRYPTED       END             ENUM
ESCAPE          EVENT           EXCEPT          EXCLUDE
EXCLUDING       EXCLUSIVE       EXECUTE         EXISTS
EXPLAIN         EXPRESSION      EXTENSION       EXTERNAL
EXTRACT         FALSE           FAMILY          FILTER
FINALIZE        FIRST           FLOAT           FOLLOWING
FORCE           FORWARD         FUNCTION        FUNCTIONS
GENERATED       GLOBAL          GRANTED         GREATEST
GROUPING        GROUPS          HANDLER         HEADER
HOLD            HOUR            IDENTITY        IF
IMMEDIATE       IMMUTABLE       IMPLICIT        IMPORT
IN              INCLUDE         INCLUDING       INCREMENT
INDEX           INDEXES         INHERIT         INHERITS
INLINE          INPUT           INSENSITIVE     INSERT
INSTEAD         INVOKER         ISOLATION       JSON
KEY             LABEL           LANGUAGE        LARGE
LAST            LATERAL         LEAKPROOF       LEAST
LEVEL           LISTEN          LOAD            LOCAL
LOCATION        LOCK            LOCKED          LOGGED
MAPPING         MATCH           MATCHED         MATERIALIZED
MAXVALUE        MERGE           METHOD          MINUTE
MINVALUE        MODE            MONTH           MOVE
NAME            NAMES           NATIONAL        NEW
NEXT            NFC             NFD             NFKC
NFKD            NO              NONE            NORMALIZED
NOTHING         NOTIFY          NOWAIT          NULL
NULLIF          NULLS           OBJECT          OBJECTS
OF              OFF             OIDS            OLD
OMIT            OPERATOR        OPTION          OPTIONS
ORDINALITY      OTHERS          OUT             OVER
OVERLAY         OVERRIDING      OWNED           OWNER
PARALLEL        PARAMETER       PARSER          PARTIAL
PARTITION       PASSING         PASSWORD        PERIOD
PLANS           POLICY          POSITION        PRECEDING
PREPARE         PREPARED        PRESERVE        PRIMARY
PRIOR           PRIVILEGES      PROCEDURAL      PROCEDURE
PROCEDURES      PROGRAM         PUBLICATION     QUOTE
QUOTES          RANGE           READ            REASSIGN
RECHECK         RECURSIVE       REF             REFERENCES
REFERENCING     REFRESH         REINDEX         RELATIVE
RELEASE         RENAME          REPEATABLE      REPLACE
REPLICA         RESET           RESPECT         RESTART
RESTORE         RESTRICT        RETURN          RETURNS
REVOKE          ROLE            ROLLBACK        ROLLUP
ROUTINE         ROUTINES        ROW             ROWS
RULE            SAVEPOINT       SCALAR          SCHEMA
SCHEMAS         SCROLL          SEARCH          SECOND
SECTION         SECURITY        SELECT          SEQUENCE
SEQUENCES       SERIALIZABLE    SERVER          SESSION
SET             SETOF           SETS            SHARE
SHOW            SIMPLE          SKIP            SNAPSHOT
SOURCE          SQL             STABLE          STANDALONE
START           STATEMENT       STATISTICS      STDIN
STDOUT          STORAGE         STORED          STRICT
STRIP           SUBSCRIPTION    SUBSTRING       SUPPORT
SYMMETRIC       SYSID           SYSTEM          TABLES
TABLESPACE      TARGET          TEMP            TEMPLATE
TEMPORARY       TEXT            THEN            TIES
TIME            TIMESTAMP       TO              TRAILING
TRANSACTION     TRANSFORM       TRANSFORMS      TREAT
TRIGGER         TRIM            TRUE            TRUNCATE
TRUSTED         TYPE            TYPES           UESCAPE
UNBOUNDED       UNCOMMITTED     UNCONDITIONAL   UNENCRYPTED
UNIQUE          UNKNOWN         UNLISTEN        UNLOGGED
UNMATCHED       UNNAMED         UNTIL           UPDATE
VACUUM          VALID           VALIDATE        VALIDATOR
VALUE           VALUES          VARCHAR         VARIADIC
VARYING         VERSION         VIEW            VIEWS
VIRTUAL         VOLATILE        WHEN            WHITESPACE
WINDOW          WITH            WITHIN          WITHOUT
WORK            WRAPPER         WRITE           XML
XMLATTRIBUTES   XMLCONCAT       XMLELEMENT      XMLEXISTS
XMLFOREST       XMLNAMESPACES   XMLPARSE        XMLPI
XMLROOT         XMLSERIALIZE    XMLTABLE        YEAR
YES             ZONE
```

---

## Parser Architecture

PostgreSQL's parser transforms SQL text into an Abstract Syntax Tree (AST).

### Parser Components

```text
┌─────────────────────────────────────────────────────────────┐
│                     SQL Query Text                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Lexical Analyzer (scan.l)                 │
│           Tokenizes input into keywords, identifiers,       │
│           literals, operators                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Grammar Parser (gram.y)                   │
│           Bison-based parser that applies grammar rules     │
│           and builds parse tree nodes                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Raw Parse Tree                         │
│           List of RawStmt nodes containing statement        │
│           parse trees                                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│               Semantic Analysis (parse_analyze)             │
│           Resolves names, validates types, transforms       │
│           FuncCall to FuncExpr/Aggref, etc.                 │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                       Query Tree                            │
│           Fully analyzed query representation               │
└─────────────────────────────────────────────────────────────┘
```

### Key Source Files

| File | Purpose |
|------|---------|
| `src/backend/parser/scan.l` | Flex lexer definition |
| `src/backend/parser/gram.y` | Bison grammar rules |
| `src/include/nodes/parsenodes.h` | Parse tree node definitions |
| `src/include/nodes/primnodes.h` | Primitive node definitions |
| `src/backend/parser/parse_analyze.c` | Semantic analysis |

### Lexer Token Types

```c
// Keywords
SELECT, FROM, WHERE, INSERT, UPDATE, DELETE, CREATE, DROP, ...

// Identifiers
IDENT           // Regular identifier
UIDENT          // Unicode identifier (U&"...")

// Literals
ICONST          // Integer constant
FCONST          // Float constant
SCONST          // String constant
BCONST          // Binary string constant
XCONST          // Hexadecimal constant
USCONST         // Unicode string constant

// Operators
Op              // Operator
EQUALS_GREATER  // =>
LESS_EQUALS     // <=
GREATER_EQUALS  // >=
NOT_EQUALS      // <> or !=
TYPECAST        // ::
DOT_DOT         // ..
COLON_EQUALS    // :=
```

---

## Abstract Syntax Tree (AST)

### Node Tag Types

Every AST node has a discriminant identifying its type. In Rust, this is typically represented as an enum:

```rust
/// Node type discriminants for PostgreSQL AST
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum NodeTag {
    Invalid = 0,
    
    // Primitive nodes
    Alias,
    RangeVar,
    TableFunc,
    Var,
    Const,
    Param,
    
    // Statement nodes
    SelectStmt,
    InsertStmt,
    UpdateStmt,
    DeleteStmt,
    CreateStmt,
    AlterTableStmt,
    MergeStmt,
    
    // Expression nodes
    AExpr,
    ColumnRef,
    ParamRef,
    AConst,
    FuncCall,
    AStar,
    AIndices,
    AIndirection,
    SubLink,
    CaseExpr,
    CaseWhen,
    CoalesceExpr,
    NullTest,
    BoolExpr,
    
    // List types (in Rust, use Vec<T> instead)
    List,
    IntList,
    OidList,
}

/// A generic AST node that can be any statement or expression
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    // Statements
    SelectStmt(Box<SelectStmt>),
    InsertStmt(Box<InsertStmt>),
    UpdateStmt(Box<UpdateStmt>),
    DeleteStmt(Box<DeleteStmt>),
    MergeStmt(Box<MergeStmt>),
    
    // Expressions
    AExpr(Box<AExpr>),
    ColumnRef(ColumnRef),
    ParamRef(ParamRef),
    AConst(AConst),
    FuncCall(Box<FuncCall>),
    BoolExpr(Box<BoolExpr>),
    SubLink(Box<SubLink>),
    CaseExpr(Box<CaseExpr>),
    NullTest(Box<NullTest>),
    
    // Utility
    RangeVar(RangeVar),
    Alias(Alias),
    ResTarget(Box<ResTarget>),
    SortBy(SortBy),
    WindowDef(Box<WindowDef>),
    
    // Special
    AStar,
    Null,
}
```

### Core Statement Nodes

#### SelectStmt

```rust
/// SET operation types for compound SELECT statements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SetOperation {
    #[default]
    None,
    Union,
    Intersect,
    Except,
}

/// LIMIT options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LimitOption {
    #[default]
    Default,
    Count,
    WithTies,
}

/// SELECT statement
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectStmt {
    /// DISTINCT ON expressions, None for ALL
    pub distinct_clause: Option<Vec<Node>>,
    /// SELECT INTO target
    pub into_clause: Option<IntoClause>,
    /// The target list (list of ResTarget)
    pub target_list: Vec<ResTarget>,
    /// The FROM clause
    pub from_clause: Vec<Node>,
    /// The WHERE clause
    pub where_clause: Option<Box<Node>>,
    /// GROUP BY clause
    pub group_clause: Vec<Node>,
    /// Is GROUP BY DISTINCT?
    pub group_distinct: bool,
    /// HAVING clause
    pub having_clause: Option<Box<Node>>,
    /// WINDOW definitions
    pub window_clause: Vec<WindowDef>,
    /// VALUES lists (for VALUES command)
    pub values_lists: Vec<Vec<Node>>,
    /// ORDER BY clause (list of SortBy)
    pub sort_clause: Vec<SortBy>,
    /// OFFSET expression
    pub limit_offset: Option<Box<Node>>,
    /// LIMIT expression
    pub limit_count: Option<Box<Node>>,
    /// LIMIT option (WITH TIES, etc.)
    pub limit_option: LimitOption,
    /// FOR UPDATE/SHARE clauses
    pub locking_clause: Vec<LockingClause>,
    /// WITH clause
    pub with_clause: Option<WithClause>,
    /// Type of set operation
    pub op: SetOperation,
    /// ALL modifier for set operation
    pub all: bool,
    /// Left operand of set op
    pub larg: Option<Box<SelectStmt>>,
    /// Right operand of set op
    pub rarg: Option<Box<SelectStmt>>,
}
```

#### InsertStmt

```rust
/// OVERRIDING clause type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverridingKind {
    #[default]
    NotSet,
    OverridingUserValue,
    OverridingSystemValue,
}

/// ON CONFLICT action
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnConflictAction {
    Nothing,
    Update,
}

/// ON CONFLICT clause
#[derive(Debug, Clone, PartialEq)]
pub struct OnConflictClause {
    /// Action to take on conflict
    pub action: OnConflictAction,
    /// Inference specification (target columns)
    pub infer: Option<InferClause>,
    /// SET assignments for DO UPDATE
    pub target_list: Vec<ResTarget>,
    /// WHERE clause for DO UPDATE
    pub where_clause: Option<Box<Node>>,
    /// Token location
    pub location: i32,
}

/// INSERT statement
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InsertStmt {
    /// Target relation to insert into
    pub relation: RangeVar,
    /// Target column names (list of ResTarget)
    pub cols: Vec<ResTarget>,
    /// SELECT query or VALUES list
    pub select_stmt: Option<Box<Node>>,
    /// ON CONFLICT clause
    pub on_conflict_clause: Option<OnConflictClause>,
    /// RETURNING clause
    pub returning_list: Vec<ResTarget>,
    /// WITH clause
    pub with_clause: Option<WithClause>,
    /// OVERRIDING clause
    pub override_: OverridingKind,
}
```

#### UpdateStmt

```rust
/// UPDATE statement
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateStmt {
    /// Target relation to update
    pub relation: RangeVar,
    /// SET clause assignments (list of ResTarget)
    pub target_list: Vec<ResTarget>,
    /// WHERE clause
    pub where_clause: Option<Box<Node>>,
    /// FROM clause for joins
    pub from_clause: Vec<Node>,
    /// RETURNING clause
    pub returning_list: Vec<ResTarget>,
    /// WITH clause
    pub with_clause: Option<WithClause>,
}
```

#### DeleteStmt

```rust
/// DELETE statement
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DeleteStmt {
    /// Target relation to delete from
    pub relation: RangeVar,
    /// USING clause for joins
    pub using_clause: Vec<Node>,
    /// WHERE clause
    pub where_clause: Option<Box<Node>>,
    /// RETURNING clause
    pub returning_list: Vec<ResTarget>,
    /// WITH clause
    pub with_clause: Option<WithClause>,
}
```

#### MergeStmt (PostgreSQL 15+)

```rust
/// MERGE WHEN clause type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeMatchKind {
    Matched,
    NotMatchedBySource,
    NotMatchedByTarget,
}

/// MERGE WHEN clause action
#[derive(Debug, Clone, PartialEq)]
pub struct MergeWhenClause {
    /// MATCHED or NOT MATCHED (BY SOURCE/TARGET)
    pub match_kind: MergeMatchKind,
    /// Command type (INSERT, UPDATE, DELETE, DO NOTHING)
    pub command_type: CmdType,
    /// AND condition
    pub condition: Option<Box<Node>>,
    /// SET assignments or INSERT target list
    pub target_list: Vec<ResTarget>,
    /// INSERT VALUES list
    pub values: Vec<Node>,
    /// OVERRIDING clause for INSERT
    pub override_: OverridingKind,
    /// Token location
    pub location: i32,
}

/// MERGE statement
#[derive(Debug, Clone, PartialEq)]
pub struct MergeStmt {
    /// Target relation
    pub relation: RangeVar,
    /// Source relation
    pub source_relation: Box<Node>,
    /// Join condition (ON clause)
    pub join_condition: Box<Node>,
    /// WHEN clauses
    pub merge_when_clauses: Vec<MergeWhenClause>,
    /// RETURNING clause (PostgreSQL 18+)
    pub returning_list: Vec<ResTarget>,
    /// WITH clause
    pub with_clause: Option<WithClause>,
}
```

### Expression Nodes

#### ColumnRef (Column Reference)

```rust
/// Column reference (e.g., "table.column" or just "column")
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnRef {
    /// Field names: ["schema", "table", "column"] or ["column"]
    /// Each element is a String node or A_Star for "*"
    pub fields: Vec<Node>,
    /// Token location in source
    pub location: i32,
}

impl ColumnRef {
    /// Create a simple column reference
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            fields: vec![Node::String(name.into())],
            location: -1,
        }
    }
    
    /// Create a qualified column reference (table.column)
    pub fn qualified(table: impl Into<String>, column: impl Into<String>) -> Self {
        Self {
            fields: vec![
                Node::String(table.into()),
                Node::String(column.into()),
            ],
            location: -1,
        }
    }
}
```

#### AConst (Constant Value)

```rust
/// Constant value in the AST
#[derive(Debug, Clone, PartialEq)]
pub enum AConst {
    /// Integer constant
    Integer(i64),
    /// Float constant (stored as string for precision)
    Float(String),
    /// Boolean constant
    Boolean(bool),
    /// String constant
    String(String),
    /// Bit string constant (e.g., B'1010')
    BitString(String),
    /// NULL constant
    Null,
}

impl AConst {
    pub fn integer(val: i64) -> Self {
        AConst::Integer(val)
    }
    
    pub fn float(val: impl Into<String>) -> Self {
        AConst::Float(val.into())
    }
    
    pub fn string(val: impl Into<String>) -> Self {
        AConst::String(val.into())
    }
    
    pub fn boolean(val: bool) -> Self {
        AConst::Boolean(val)
    }
    
    pub fn null() -> Self {
        AConst::Null
    }
}
```

#### FuncCall (Function Call)

```rust
/// How to display function in output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoercionForm {
    #[default]
    Explicit,
    Implicit,
    CoerceViaIO,
    CoerceViaUnion,
}

/// Function call expression
#[derive(Debug, Clone, PartialEq)]
pub struct FuncCall {
    /// Qualified function name: ["schema", "function"] or ["function"]
    pub funcname: Vec<String>,
    /// Arguments to the function
    pub args: Vec<Node>,
    /// ORDER BY within aggregate
    pub agg_order: Vec<SortBy>,
    /// FILTER clause for aggregates
    pub agg_filter: Option<Box<Node>>,
    /// OVER clause for window functions
    pub over: Option<WindowDef>,
    /// WITHIN GROUP for ordered-set aggregates
    pub agg_within_group: bool,
    /// Is this COUNT(*)?
    pub agg_star: bool,
    /// Has DISTINCT modifier?
    pub agg_distinct: bool,
    /// Last arg is VARIADIC?
    pub func_variadic: bool,
    /// How to display this function call
    pub funcformat: CoercionForm,
    /// Token location
    pub location: i32,
}

impl FuncCall {
    /// Create a simple function call
    pub fn new(name: impl Into<String>, args: Vec<Node>) -> Self {
        Self {
            funcname: vec![name.into()],
            args,
            agg_order: vec![],
            agg_filter: None,
            over: None,
            agg_within_group: false,
            agg_star: false,
            agg_distinct: false,
            func_variadic: false,
            funcformat: CoercionForm::Explicit,
            location: -1,
        }
    }
    
    /// Create COUNT(*)
    pub fn count_star() -> Self {
        Self {
            funcname: vec!["count".into()],
            args: vec![],
            agg_order: vec![],
            agg_filter: None,
            over: None,
            agg_within_group: false,
            agg_star: true,
            agg_distinct: false,
            func_variadic: false,
            funcformat: CoercionForm::Explicit,
            location: -1,
        }
    }
}
```

#### AExpr (Generic Expression)

```rust
/// Expression kind for A_Expr
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AExprKind {
    /// Normal operator expression
    Op,
    /// scalar op ANY (array)
    OpAny,
    /// scalar op ALL (array)  
    OpAll,
    /// IS DISTINCT FROM
    Distinct,
    /// IS NOT DISTINCT FROM
    NotDistinct,
    /// NULLIF(a, b)
    NullIf,
    /// a IN (...)
    In,
    /// [NOT] LIKE
    Like,
    /// [NOT] ILIKE
    ILike,
    /// [NOT] SIMILAR TO
    Similar,
    /// BETWEEN
    Between,
    /// NOT BETWEEN
    NotBetween,
    /// BETWEEN SYMMETRIC
    BetweenSym,
    /// NOT BETWEEN SYMMETRIC
    NotBetweenSym,
}

/// Generic expression node for operators and special syntax
#[derive(Debug, Clone, PartialEq)]
pub struct AExpr {
    /// Type of expression
    pub kind: AExprKind,
    /// Operator name(s), e.g., ["+"] or ["pg_catalog", "="]
    pub name: Vec<String>,
    /// Left operand (None for prefix operators)
    pub lexpr: Option<Box<Node>>,
    /// Right operand (None for postfix operators)
    pub rexpr: Option<Box<Node>>,
    /// Token location
    pub location: i32,
}

impl AExpr {
    /// Create a binary operator expression
    pub fn binary_op(op: impl Into<String>, left: Node, right: Node) -> Self {
        Self {
            kind: AExprKind::Op,
            name: vec![op.into()],
            lexpr: Some(Box::new(left)),
            rexpr: Some(Box::new(right)),
            location: -1,
        }
    }
    
    /// Create an equality expression
    pub fn eq(left: Node, right: Node) -> Self {
        Self::binary_op("=", left, right)
    }
    
    /// Create a LIKE expression
    pub fn like(expr: Node, pattern: Node) -> Self {
        Self {
            kind: AExprKind::Like,
            name: vec!["~~".into()],
            lexpr: Some(Box::new(expr)),
            rexpr: Some(Box::new(pattern)),
            location: -1,
        }
    }
}
```

#### BoolExpr (Boolean Expression)

```rust
/// Boolean expression type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolExprType {
    And,
    Or,
    Not,
}

/// Boolean combination expression (AND, OR, NOT)
#[derive(Debug, Clone, PartialEq)]
pub struct BoolExpr {
    /// Type of boolean operation
    pub boolop: BoolExprType,
    /// Argument expressions
    pub args: Vec<Node>,
    /// Token location
    pub location: i32,
}

impl BoolExpr {
    /// Create an AND expression
    pub fn and(args: Vec<Node>) -> Self {
        Self {
            boolop: BoolExprType::And,
            args,
            location: -1,
        }
    }
    
    /// Create an OR expression  
    pub fn or(args: Vec<Node>) -> Self {
        Self {
            boolop: BoolExprType::Or,
            args,
            location: -1,
        }
    }
    
    /// Create a NOT expression
    pub fn not(arg: Node) -> Self {
        Self {
            boolop: BoolExprType::Not,
            args: vec![arg],
            location: -1,
        }
    }
}
```

#### ParamRef (Parameter Reference)

```rust
/// Parameter reference ($1, $2, etc.)
#[derive(Debug, Clone, PartialEq)]
pub struct ParamRef {
    /// Parameter number (1-indexed)
    pub number: i32,
    /// Token location
    pub location: i32,
}

impl ParamRef {
    pub fn new(number: i32) -> Self {
        Self { number, location: -1 }
    }
}
```

#### SubLink (Subquery Expression)

```rust
/// Subquery link type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubLinkType {
    /// EXISTS(SELECT ...)
    Exists,
    /// (SELECT ...) = ALL/ANY/SOME (SELECT ...)
    All,
    Any,
    /// Scalar subquery (SELECT ... returns single value)
    Expr,
    /// Multi-column comparison
    Multiexpr,
    /// Array subquery: ARRAY(SELECT ...)
    Array,
    /// CTE reference
    Cte,
}

/// Subquery expression
#[derive(Debug, Clone, PartialEq)]
pub struct SubLink {
    /// Type of subquery
    pub sub_link_type: SubLinkType,
    /// ID for CTE references
    pub sub_link_id: i32,
    /// Test expression (left side of comparison)
    pub test_expr: Option<Box<Node>>,
    /// Operator name for comparison
    pub oper_name: Vec<String>,
    /// The subquery itself
    pub subselect: Box<SelectStmt>,
    /// Token location
    pub location: i32,
}
```

#### CaseExpr (CASE Expression)

```rust
/// CASE WHEN ... THEN ... ELSE ... END
#[derive(Debug, Clone, PartialEq)]
pub struct CaseExpr {
    /// Implicit equality test expression (for simple CASE)
    pub arg: Option<Box<Node>>,
    /// List of CaseWhen nodes
    pub args: Vec<CaseWhen>,
    /// ELSE expression
    pub defresult: Option<Box<Node>>,
    /// Token location
    pub location: i32,
}

/// WHEN condition THEN result
#[derive(Debug, Clone, PartialEq)]
pub struct CaseWhen {
    /// WHEN condition
    pub expr: Box<Node>,
    /// THEN result
    pub result: Box<Node>,
    /// Token location
    pub location: i32,
}
```

#### NullTest (NULL Test)

```rust
/// NULL test type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullTestType {
    IsNull,
    IsNotNull,
}

/// IS [NOT] NULL test
#[derive(Debug, Clone, PartialEq)]
pub struct NullTest {
    /// Expression to test
    pub arg: Box<Node>,
    /// IS NULL or IS NOT NULL
    pub nulltesttype: NullTestType,
    /// Was originally UNKNOWN test (internal)
    pub argisrow: bool,
    /// Token location
    pub location: i32,
}
```

### Utility Nodes

#### RangeVar (Table Reference)

```rust
/// Table/relation reference
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RangeVar {
    /// Catalog name (usually None)
    pub catalogname: Option<String>,
    /// Schema name
    pub schemaname: Option<String>,
    /// Table name (required)
    pub relname: String,
    /// Include inheritance children? (usually true)
    pub inh: bool,
    /// Persistence: 'p' permanent, 'u' unlogged, 't' temp
    pub relpersistence: char,
    /// Table alias
    pub alias: Option<Alias>,
    /// Token location
    pub location: i32,
}

impl RangeVar {
    /// Create a simple table reference
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            catalogname: None,
            schemaname: None,
            relname: name.into(),
            inh: true,
            relpersistence: 'p',
            alias: None,
            location: -1,
        }
    }
    
    /// Create a schema-qualified table reference
    pub fn with_schema(schema: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            catalogname: None,
            schemaname: Some(schema.into()),
            relname: name.into(),
            inh: true,
            relpersistence: 'p',
            alias: None,
            location: -1,
        }
    }
    
    /// Add an alias to this reference
    pub fn aliased(mut self, alias: impl Into<String>) -> Self {
        self.alias = Some(Alias::new(alias));
        self
    }
}
```

#### Alias

```rust
/// Table or column alias
#[derive(Debug, Clone, PartialEq)]
pub struct Alias {
    /// Alias name
    pub aliasname: String,
    /// Column aliases (for table alias with column list)
    pub colnames: Vec<String>,
}

impl Alias {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            aliasname: name.into(),
            colnames: vec![],
        }
    }
    
    pub fn with_columns(name: impl Into<String>, columns: Vec<String>) -> Self {
        Self {
            aliasname: name.into(),
            colnames: columns,
        }
    }
}
```

#### ResTarget (Result Column / Assignment)

```rust
/// Target list entry (SELECT column or SET assignment)
#[derive(Debug, Clone, PartialEq)]
pub struct ResTarget {
    /// Column name or alias (None for unnamed)
    pub name: Option<String>,
    /// Subscripts/field selections for assignment
    pub indirection: Vec<Node>,
    /// Value expression
    pub val: Option<Box<Node>>,
    /// Token location
    pub location: i32,
}

impl ResTarget {
    /// Create a SELECT target from an expression
    pub fn expr(val: Node) -> Self {
        Self {
            name: None,
            indirection: vec![],
            val: Some(Box::new(val)),
            location: -1,
        }
    }
    
    /// Create a SELECT target with an alias
    pub fn aliased(val: Node, alias: impl Into<String>) -> Self {
        Self {
            name: Some(alias.into()),
            indirection: vec![],
            val: Some(Box::new(val)),
            location: -1,
        }
    }
    
    /// Create an INSERT column target
    pub fn column(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            indirection: vec![],
            val: None,
            location: -1,
        }
    }
}
```

#### SortBy (ORDER BY Item)

```rust
/// Sort order
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortByDir {
    #[default]
    Default,
    Asc,
    Desc,
    Using, // USING operator
}

/// NULLS positioning
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortByNulls {
    #[default]
    Default,
    First,
    Last,
}

/// ORDER BY clause item
#[derive(Debug, Clone, PartialEq)]
pub struct SortBy {
    /// Expression to sort by
    pub node: Box<Node>,
    /// ASC/DESC/USING
    pub sortby_dir: SortByDir,
    /// NULLS FIRST/LAST
    pub sortby_nulls: SortByNulls,
    /// USING operator (if sortby_dir is Using)
    pub use_op: Vec<String>,
    /// Token location
    pub location: i32,
}

impl SortBy {
    pub fn asc(node: Node) -> Self {
        Self {
            node: Box::new(node),
            sortby_dir: SortByDir::Asc,
            sortby_nulls: SortByNulls::Default,
            use_op: vec![],
            location: -1,
        }
    }
    
    pub fn desc(node: Node) -> Self {
        Self {
            node: Box::new(node),
            sortby_dir: SortByDir::Desc,
            sortby_nulls: SortByNulls::Default,
            use_op: vec![],
            location: -1,
        }
    }
}
```

#### JoinExpr (Join Expression)

```rust
/// Join type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinType {
    Inner,
    Left,
    Full,
    Right,
    Semi,
    Anti,
    /// Implicit join (comma in FROM)
    UniqueOuter,
    UniqueInner,
}

/// JOIN expression
#[derive(Debug, Clone, PartialEq)]
pub struct JoinExpr {
    /// Type of join
    pub jointype: JoinType,
    /// NATURAL join?
    pub is_natural: bool,
    /// Left relation
    pub larg: Box<Node>,
    /// Right relation
    pub rarg: Box<Node>,
    /// USING clause (list of column names)
    pub using_clause: Vec<String>,
    /// Join alias (if any)
    pub join_using_alias: Option<Alias>,
    /// ON clause
    pub quals: Option<Box<Node>>,
    /// Alias for the join
    pub alias: Option<Alias>,
    /// Range table index (filled during analysis)
    pub rtindex: i32,
}
```

#### WithClause (CTE)

```rust
/// Common Table Expression (CTE)
#[derive(Debug, Clone, PartialEq)]
pub struct CommonTableExpr {
    /// CTE name
    pub ctename: String,
    /// Optional column aliases
    pub aliascolnames: Vec<String>,
    /// Materialization hint
    pub ctematerialized: CTEMaterialize,
    /// The CTE query
    pub ctequery: Box<Node>,
    /// SEARCH clause
    pub search_clause: Option<CTESearchClause>,
    /// CYCLE clause  
    pub cycle_clause: Option<CTECycleClause>,
    /// Token location
    pub location: i32,
    /// Is this recursive? (filled during analysis)
    pub cterecursive: bool,
    /// Reference count (filled during analysis)
    pub cterefcount: i32,
    /// Column types (filled during analysis)
    pub ctecolnames: Vec<String>,
    pub ctecoltypes: Vec<Oid>,
    pub ctecoltypmods: Vec<i32>,
    pub ctecolcollations: Vec<Oid>,
}

/// CTE materialization option
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CTEMaterialize {
    #[default]
    Default,
    Always,
    Never,
}

/// WITH clause
#[derive(Debug, Clone, PartialEq)]
pub struct WithClause {
    /// List of CTEs
    pub ctes: Vec<CommonTableExpr>,
    /// Is this WITH RECURSIVE?
    pub recursive: bool,
    /// Token location
    pub location: i32,
}
```

#### WindowDef (Window Definition)

```rust
/// Window frame boundary type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowBoundType {
    UnboundedPreceding,
    ValuePreceding,
    CurrentRow,
    ValueFollowing,
    UnboundedFollowing,
}

/// Window frame edge
#[derive(Debug, Clone, PartialEq)]
pub struct WindowBound {
    pub kind: WindowBoundType,
    pub val: Option<Box<Node>>,
}

/// Frame mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowFrameMode {
    #[default]
    Range,
    Rows,
    Groups,
}

/// Window definition (OVER clause)
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WindowDef {
    /// Window name (for named windows)
    pub name: Option<String>,
    /// Referenced window name
    pub refname: Option<String>,
    /// PARTITION BY expressions
    pub partition_clause: Vec<Node>,
    /// ORDER BY expressions
    pub order_clause: Vec<SortBy>,
    /// Frame mode: ROWS, RANGE, GROUPS
    pub frame_options: WindowFrameMode,
    /// Frame start bound
    pub start_offset: Option<WindowBound>,
    /// Frame end bound
    pub end_offset: Option<WindowBound>,
    /// Token location
    pub location: i32,
}
```

---

## Data Types and OIDs

### Rust Type Definitions

```rust
/// PostgreSQL Object Identifier
pub type Oid = u32;

/// Well-known PostgreSQL type OIDs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum PgType {
    Bool = 16,
    Bytea = 17,
    Char = 18,
    Name = 19,
    Int8 = 20,
    Int2 = 21,
    Int2Vector = 22,
    Int4 = 23,
    RegProc = 24,
    Text = 25,
    Oid = 26,
    Tid = 27,
    Xid = 28,
    Cid = 29,
    OidVector = 30,
    Json = 114,
    Xml = 142,
    Point = 600,
    Lseg = 601,
    Path = 602,
    Box = 603,
    Polygon = 604,
    Line = 628,
    Float4 = 700,
    Float8 = 701,
    Money = 790,
    MacAddr = 829,
    Inet = 869,
    Cidr = 650,
    MacAddr8 = 774,
    BoolArray = 1000,
    ByteaArray = 1001,
    CharArray = 1002,
    NameArray = 1003,
    Int2Array = 1005,
    Int4Array = 1007,
    TextArray = 1009,
    BpCharArray = 1014,
    VarCharArray = 1015,
    Int8Array = 1016,
    Float4Array = 1021,
    Float8Array = 1022,
    OidArray = 1028,
    AclItem = 1033,
    BpChar = 1042,
    VarChar = 1043,
    Date = 1082,
    Time = 1083,
    Timestamp = 1114,
    TimestampTz = 1184,
    Interval = 1186,
    TimeTz = 1266,
    Bit = 1560,
    VarBit = 1562,
    Numeric = 1700,
    Uuid = 2950,
    PgLsn = 3220,
    TsVector = 3614,
    TsQuery = 3615,
    Jsonb = 3802,
    RegNamespace = 4089,
    RegRole = 4096,
    JsonbArray = 3807,
    JsonPath = 4072,
    RegDatabase = 8326,
}

impl PgType {
    /// Get the OID value
    pub const fn oid(self) -> Oid {
        self as Oid
    }
    
    /// Try to get PgType from an OID
    pub fn from_oid(oid: Oid) -> Option<Self> {
        match oid {
            16 => Some(Self::Bool),
            17 => Some(Self::Bytea),
            18 => Some(Self::Char),
            19 => Some(Self::Name),
            20 => Some(Self::Int8),
            21 => Some(Self::Int2),
            23 => Some(Self::Int4),
            25 => Some(Self::Text),
            26 => Some(Self::Oid),
            114 => Some(Self::Json),
            700 => Some(Self::Float4),
            701 => Some(Self::Float8),
            1042 => Some(Self::BpChar),
            1043 => Some(Self::VarChar),
            1082 => Some(Self::Date),
            1083 => Some(Self::Time),
            1114 => Some(Self::Timestamp),
            1184 => Some(Self::TimestampTz),
            1186 => Some(Self::Interval),
            1700 => Some(Self::Numeric),
            2950 => Some(Self::Uuid),
            3802 => Some(Self::Jsonb),
            _ => None,
        }
    }
    
    /// Get type name as it appears in PostgreSQL
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Bool => "boolean",
            Self::Bytea => "bytea",
            Self::Char => "char",
            Self::Name => "name",
            Self::Int8 => "bigint",
            Self::Int2 => "smallint",
            Self::Int4 => "integer",
            Self::Text => "text",
            Self::Float4 => "real",
            Self::Float8 => "double precision",
            Self::Numeric => "numeric",
            Self::VarChar => "character varying",
            Self::BpChar => "character",
            Self::Date => "date",
            Self::Time => "time without time zone",
            Self::Timestamp => "timestamp without time zone",
            Self::TimestampTz => "timestamp with time zone",
            Self::Interval => "interval",
            Self::Uuid => "uuid",
            Self::Json => "json",
            Self::Jsonb => "jsonb",
            _ => "unknown",
        }
    }
}

/// Type category codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCategory {
    Array = b'A' as isize,
    Boolean = b'B' as isize,
    Composite = b'C' as isize,
    DateTime = b'D' as isize,
    Enum = b'E' as isize,
    Geometric = b'G' as isize,
    NetworkAddress = b'I' as isize,
    Numeric = b'N' as isize,
    Pseudo = b'P' as isize,
    Range = b'R' as isize,
    String = b'S' as isize,
    Timespan = b'T' as isize,
    UserDefined = b'U' as isize,
    BitString = b'V' as isize,
    Unknown = b'X' as isize,
    Internal = b'Z' as isize,
}
```

### Built-in Type OIDs

| OID | Type Name | Size | Category |
|-----|-----------|------|----------|
| 16 | bool | 1 | Boolean |
| 17 | bytea | -1 | User-defined |
| 18 | char | 1 | String |
| 19 | name | 64 | String |
| 20 | int8 | 8 | Numeric |
| 21 | int2 | 2 | Numeric |
| 22 | int2vector | -1 | Array |
| 23 | int4 | 4 | Numeric |
| 24 | regproc | 4 | Numeric |
| 25 | text | -1 | String |
| 26 | oid | 4 | Numeric |
| 27 | tid | 6 | User-defined |
| 28 | xid | 4 | User-defined |
| 29 | cid | 4 | User-defined |
| 30 | oidvector | -1 | Array |
| 114 | json | -1 | User-defined |
| 142 | xml | -1 | User-defined |
| 600 | point | 16 | Geometric |
| 601 | lseg | 32 | Geometric |
| 602 | path | -1 | Geometric |
| 603 | box | 32 | Geometric |
| 604 | polygon | -1 | Geometric |
| 628 | line | 24 | Geometric |
| 700 | float4 | 4 | Numeric |
| 701 | float8 | 8 | Numeric |
| 790 | money | 8 | Numeric |
| 829 | macaddr | 6 | User-defined |
| 869 | inet | -1 | Network |
| 650 | cidr | -1 | Network |
| 774 | macaddr8 | 8 | User-defined |
| 1000 | _bool | -1 | Array |
| 1001 | _bytea | -1 | Array |
| 1002 | _char | -1 | Array |
| 1003 | _name | -1 | Array |
| 1005 | _int2 | -1 | Array |
| 1006 | _int2vector | -1 | Array |
| 1007 | _int4 | -1 | Array |
| 1008 | _regproc | -1 | Array |
| 1009 | _text | -1 | Array |
| 1014 | _bpchar | -1 | Array |
| 1015 | _varchar | -1 | Array |
| 1016 | _int8 | -1 | Array |
| 1017 | _point | -1 | Array |
| 1021 | _float4 | -1 | Array |
| 1022 | _float8 | -1 | Array |
| 1028 | _oid | -1 | Array |
| 1033 | aclitem | 12 | User-defined |
| 1042 | bpchar | -1 | String |
| 1043 | varchar | -1 | String |
| 1082 | date | 4 | Date/Time |
| 1083 | time | 8 | Date/Time |
| 1114 | timestamp | 8 | Date/Time |
| 1184 | timestamptz | 8 | Date/Time |
| 1186 | interval | 16 | Timespan |
| 1266 | timetz | 12 | Date/Time |
| 1560 | bit | -1 | Bit String |
| 1562 | varbit | -1 | Bit String |
| 1700 | numeric | -1 | Numeric |
| 2950 | uuid | 16 | User-defined |
| 3220 | pg_lsn | 8 | User-defined |
| 3614 | tsvector | -1 | User-defined |
| 3615 | tsquery | -1 | User-defined |
| 3802 | jsonb | -1 | User-defined |
| 4089 | regnamespace | 4 | Numeric |
| 4096 | regrole | 4 | Numeric |

### Type Categories

| Code | Category |
|------|----------|
| A | Array |
| B | Boolean |
| C | Composite |
| D | Date/Time |
| E | Enum |
| G | Geometric |
| I | Network Address |
| N | Numeric |
| P | Pseudo |
| R | Range |
| S | String |
| T | Timespan |
| U | User-defined |
| V | Bit String |
| X | Unknown |
| Z | Internal |

### OID Alias Types

| Type | OID | References |
|------|-----|------------|
| regproc | 24 | pg_proc.oid |
| regprocedure | 2202 | pg_proc.oid (with args) |
| regoper | 2203 | pg_operator.oid |
| regoperator | 2204 | pg_operator.oid (with args) |
| regclass | 2205 | pg_class.oid |
| regtype | 2206 | pg_type.oid |
| regconfig | 3734 | pg_ts_config.oid |
| regdictionary | 3769 | pg_ts_dict.oid |
| regrole | 4096 | pg_authid.oid |
| regnamespace | 4089 | pg_namespace.oid |
| regcollation | 4191 | pg_collation.oid |
| regdatabase | 8326 | pg_database.oid |

---

## Extended Query Protocol

The Extended Query Protocol separates query processing into distinct phases for better control and performance.

### Protocol Flow

```text
┌──────────┐                              ┌──────────┐
│ Frontend │                              │ Backend  │
└────┬─────┘                              └────┬─────┘
     │                                         │
     │  Parse (P)                              │
     │ ───────────────────────────────────────>│
     │                                         │
     │                        ParseComplete (1)│
     │ <───────────────────────────────────────│
     │                                         │
     │  Bind (B)                               │
     │ ───────────────────────────────────────>│
     │                                         │
     │                         BindComplete (2)│
     │ <───────────────────────────────────────│
     │                                         │
     │  Describe (D) [optional]                │
     │ ───────────────────────────────────────>│
     │                                         │
     │         ParameterDescription (t)        │
     │ <───────────────────────────────────────│
     │                                         │
     │             RowDescription (T)          │
     │ <───────────────────────────────────────│
     │                                         │
     │  Execute (E)                            │
     │ ───────────────────────────────────────>│
     │                                         │
     │                 DataRow (D) [repeated]  │
     │ <───────────────────────────────────────│
     │                                         │
     │              CommandComplete (C)        │
     │ <───────────────────────────────────────│
     │                                         │
     │  Sync (S)                               │
     │ ───────────────────────────────────────>│
     │                                         │
     │              ReadyForQuery (Z)          │
     │ <───────────────────────────────────────│
```

### Named vs Unnamed Statements

| Feature | Unnamed | Named |
|---------|---------|-------|
| Lifetime | Until next Parse/Query | Until Close or session end |
| Optimization | Single use | Multiple uses |
| Destruction | Automatic | Explicit Close required |

### Pipelining

Extended query supports pipelining (sending multiple requests without waiting for responses):

```text
Frontend: Parse → Bind → Execute → Parse → Bind → Execute → Sync
Backend:  ParseComplete → BindComplete → DataRow... → CommandComplete →
          ParseComplete → BindComplete → DataRow... → CommandComplete →
          ReadyForQuery
```

### Parameter Binding Example

```rust
/// Build a Bind message with parameters
pub fn build_bind_message(
    portal_name: &str,
    statement_name: &str,
    params: &[&dyn ToSql],
    param_formats: &[i16],    // 0 = text, 1 = binary
    result_formats: &[i16],   // 0 = text, 1 = binary
) -> Vec<u8> {
    let mut buf = MessageBuffer::new();
    buf.start(b'B');
    
    // Portal name (empty for unnamed)
    buf.write_cstr(portal_name);
    
    // Statement name (empty for unnamed)
    buf.write_cstr(statement_name);
    
    // Parameter format codes
    buf.write_i16(param_formats.len() as i16);
    for &fmt in param_formats {
        buf.write_i16(fmt);
    }
    
    // Parameter values
    buf.write_i16(params.len() as i16);
    for param in params {
        if let Some(bytes) = param.to_sql_bytes() {
            buf.write_i32(bytes.len() as i32);
            buf.write_bytes(&bytes);
        } else {
            buf.write_i32(-1); // NULL
        }
    }
    
    // Result format codes
    buf.write_i16(result_formats.len() as i16);
    for &fmt in result_formats {
        buf.write_i16(fmt);
    }
    
    buf.finish().to_vec()
}

/// Example usage
fn example_prepared_query(conn: &mut Connection) -> io::Result<()> {
    // Query with parameters
    let sql = "SELECT id, name, email FROM users WHERE age > $1 AND status = $2";
    let params: Vec<&dyn ToSql> = vec![&25i32, &"active"];
    
    let results = conn.execute_prepared(sql, &params)?;
    
    for result in results {
        match result {
            QueryResult::Select { columns, rows, tag } => {
                println!("Columns: {:?}", columns.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
                println!("Got {} rows ({})", rows.len(), tag);
                
                for row in rows {
                    for (i, value) in row.values.iter().enumerate() {
                        match value {
                            Some(bytes) => {
                                let s = String::from_utf8_lossy(bytes);
                                println!("  {}: {}", columns.columns[i].name, s);
                            }
                            None => println!("  {}: NULL", columns.columns[i].name),
                        }
                    }
                }
            }
            QueryResult::Command { tag } => {
                println!("Command completed: {}", tag);
            }
            QueryResult::Empty => {
                println!("Empty query");
            }
        }
    }
    
    Ok(())
}
```

---

## Implementation Libraries

### libpg_query

The official PostgreSQL parser extracted as a standalone library.

**Repository**: <https://github.com/pganalyze/libpg_query>

**Versions**: Tracks PostgreSQL versions (17-latest branch for PG17, 18-latest for PG18)

**Features**:

- Parse SQL to JSON or Protocol Buffers
- Deparse AST back to SQL
- Query fingerprinting
- PL/pgSQL parsing

**Language Bindings**:

| Language | Library |
|----------|---------|
| Ruby | pg_query |
| Go | pg_query_go |
| Rust | pg_query.rs |
| Node.js | pgsql-parser, libpg-query |
| Python | pglast |
| PHP | flow-php/pg-query |
| .NET | Npgquery |

**Example (Node.js)**:

```javascript
import { parse } from 'pgsql-parser';

const ast = await parse('SELECT id, name FROM users WHERE active = true');
console.log(JSON.stringify(ast, null, 2));
// Output:
// {
//   "version": 180000,
//   "stmts": [{
//     "stmt": {
//       "SelectStmt": {
//         "targetList": [...],
//         "fromClause": [...],
//         "whereClause": {...}
//       }
//     }
//   }]
// }
```

**Example (Python with pglast)**:

```python
from pglast import parse_sql, prettify

# Parse SQL
result = parse_sql('SELECT * FROM users WHERE id = $1')
print(result)

# Pretty print
sql = prettify('select * from users where id=$1')
print(sql)  # SELECT * FROM users WHERE id = $1
```

### pgwire (Rust)

PostgreSQL wire protocol implementation for building compatible servers.

**Repository**: <https://github.com/sunng87/pgwire>

**Features**:

- Protocol 3.0 and 3.2 support
- Simple and Extended query protocols
- SSL/TLS support
- Streaming replication protocol

**Example**:

```rust
use pgwire::api::auth::noop::NoopStartupHandler;
use pgwire::api::query::SimpleQueryHandler;
use pgwire::api::results::{DataRowEncoder, FieldInfo, QueryResponse};
use pgwire::api::Type;

struct MyQueryHandler;

impl SimpleQueryHandler for MyQueryHandler {
    async fn do_query(&self, query: &str) -> QueryResponse {
        // Handle query
        let fields = vec![
            FieldInfo::new("id".to_string(), None, None, Type::INT4, 0),
            FieldInfo::new("name".to_string(), None, None, Type::VARCHAR, 0),
        ];
        
        let mut encoder = DataRowEncoder::new(fields.len());
        encoder.encode_field(&1i32);
        encoder.encode_field(&"Alice".to_string());
        
        QueryResponse::new(fields, vec![encoder.finish()])
    }
}
```

---

## PostgreSQL 18 New Features

### SQL Syntax Additions

#### UUIDv7 Generation

```sql
-- Generate timestamp-ordered UUID
SELECT uuidv7();

-- Use as primary key
CREATE TABLE orders (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    customer_id INT,
    created_at TIMESTAMP DEFAULT NOW()
);
```

#### Virtual Generated Columns

```sql
-- Virtual columns (computed at query time)
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    price NUMERIC(10,2),
    quantity INT,
    total NUMERIC(10,2) GENERATED ALWAYS AS (price * quantity) VIRTUAL
);
```

#### Temporal Constraints

```sql
-- Temporal PRIMARY KEY
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

#### OLD/NEW in RETURNING

```sql
-- Access OLD values in UPDATE RETURNING
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

#### MERGE Enhancements

```sql
MERGE INTO target_table t
USING source_table s ON t.id = s.id
WHEN MATCHED THEN
    UPDATE SET value = s.value
    RETURNING OLD.value, NEW.value
WHEN NOT MATCHED THEN
    INSERT (id, value) VALUES (s.id, s.value)
    RETURNING *;
```

### Protocol Changes (v3.2)

1. **Variable-length cancellation keys**: Backend key data can now be 4-256 bytes (was fixed 4 bytes)
2. **OAuth authentication support**: New authentication type for OAuth-based connections
3. **Improved pipelining**: Better error handling for pipelined queries

### Performance Features

1. **Asynchronous I/O (AIO)**: New `io_method` setting with `io_uring` support on Linux
2. **Skip scan**: B-tree index optimization for multicolumn indexes
3. **Parallel GIN index builds**: Parallel creation of GIN indexes
4. **Preserved statistics on upgrade**: pg_upgrade retains planner statistics

---

## Quick Reference Tables

### Common SQLSTATE Codes

| Code | Meaning |
|------|---------|
| 00000 | Successful completion |
| 01000 | Warning |
| 02000 | No data |
| 03000 | SQL statement not yet complete |
| 08000 | Connection exception |
| 22000 | Data exception |
| 23000 | Integrity constraint violation |
| 25000 | Invalid transaction state |
| 28000 | Invalid authorization specification |
| 40000 | Transaction rollback |
| 42000 | Syntax error or access rule violation |
| 53000 | Insufficient resources |
| 57000 | Operator intervention |
| 58000 | System error |

### Format Codes

| Code | Format | Description |
|------|--------|-------------|
| 0 | Text | String representation |
| 1 | Binary | Native binary encoding |

### Transaction Status Indicators

| Indicator | Status |
|-----------|--------|
| I | Idle (not in transaction) |
| T | In transaction block |
| E | In failed transaction block |

---

## Appendix: Sample Implementation Patterns (Rust)

### Core Protocol Types

```rust
use std::io::{self, Read, Write};
use std::net::TcpStream;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

/// Frontend (client) message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrontendMessage {
    Bind = b'B',
    Close = b'C',
    Describe = b'D',
    Execute = b'E',
    Flush = b'H',
    Parse = b'P',
    Query = b'Q',
    Sync = b'S',
    Terminate = b'X',
    CopyData = b'd',
    CopyDone = b'c',
    CopyFail = b'f',
    Password = b'p',
    FunctionCall = b'F',
}

/// Backend (server) message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BackendMessage {
    Authentication = b'R',
    BackendKeyData = b'K',
    BindComplete = b'2',
    CloseComplete = b'3',
    CommandComplete = b'C',
    CopyData = b'd',
    CopyDone = b'c',
    CopyInResponse = b'G',
    CopyOutResponse = b'H',
    CopyBothResponse = b'W',
    DataRow = b'D',
    EmptyQueryResponse = b'I',
    ErrorResponse = b'E',
    FunctionCallResponse = b'V',
    NegotiateProtocolVersion = b'v',
    NoData = b'n',
    NoticeResponse = b'N',
    NotificationResponse = b'A',
    ParameterDescription = b't',
    ParameterStatus = b'S',
    ParseComplete = b'1',
    PortalSuspended = b's',
    ReadyForQuery = b'Z',
    RowDescription = b'T',
}

/// Transaction status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Idle,        // 'I'
    Transaction, // 'T'
    Error,       // 'E'
}

impl TryFrom<u8> for TransactionStatus {
    type Error = io::Error;
    
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            b'I' => Ok(Self::Idle),
            b'T' => Ok(Self::Transaction),
            b'E' => Ok(Self::Error),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid transaction status")),
        }
    }
}
```

### Message Building

```rust
/// Buffer for building protocol messages
pub struct MessageBuffer {
    buf: Vec<u8>,
}

impl MessageBuffer {
    pub fn new() -> Self {
        Self { buf: Vec::with_capacity(256) }
    }
    
    /// Start a new message with the given type byte
    pub fn start(&mut self, msg_type: u8) {
        self.buf.clear();
        self.buf.push(msg_type);
        // Reserve space for length (filled in later)
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
    }
    
    /// Start a startup message (no type byte)
    pub fn start_startup(&mut self) {
        self.buf.clear();
        // Reserve space for length
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
    }
    
    pub fn write_i16(&mut self, val: i16) {
        self.buf.write_i16::<BigEndian>(val).unwrap();
    }
    
    pub fn write_i32(&mut self, val: i32) {
        self.buf.write_i32::<BigEndian>(val).unwrap();
    }
    
    pub fn write_cstr(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
    }
    
    pub fn write_bytes(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }
    
    /// Finish the message, filling in the length
    pub fn finish(&mut self) -> &[u8] {
        let len = (self.buf.len() - 1) as i32; // Exclude type byte
        self.buf[1..5].copy_from_slice(&len.to_be_bytes());
        &self.buf
    }
    
    /// Finish startup message (length includes itself)
    pub fn finish_startup(&mut self) -> &[u8] {
        let len = self.buf.len() as i32;
        self.buf[0..4].copy_from_slice(&len.to_be_bytes());
        &self.buf
    }
}
```

### Connection Startup Sequence

```rust
use std::collections::HashMap;

/// PostgreSQL connection
pub struct Connection {
    stream: TcpStream,
    params: HashMap<String, String>,
    backend_pid: i32,
    secret_key: Vec<u8>,
}

/// Authentication result
pub enum AuthResult {
    Ok,
    CleartextPassword,
    Md5Password { salt: [u8; 4] },
    Sasl { mechanisms: Vec<String> },
}

impl Connection {
    pub fn connect(
        host: &str,
        port: u16,
        user: &str,
        database: &str,
        password: Option<&str>,
    ) -> io::Result<Self> {
        let mut stream = TcpStream::connect((host, port))?;
        let mut buf = MessageBuffer::new();
        
        // Build startup message
        buf.start_startup();
        buf.write_i32(196610); // Protocol 3.2
        buf.write_cstr("user");
        buf.write_cstr(user);
        buf.write_cstr("database");
        buf.write_cstr(database);
        buf.write_cstr("client_encoding");
        buf.write_cstr("UTF8");
        buf.buf.push(0); // Terminator
        
        stream.write_all(buf.finish_startup())?;
        
        // Handle authentication
        loop {
            let (msg_type, data) = read_message(&mut stream)?;
            
            match msg_type {
                b'R' => {
                    let auth_type = (&data[..4]).read_i32::<BigEndian>()?;
                    match auth_type {
                        0 => break, // AuthenticationOk
                        3 => {
                            // Cleartext password
                            let pwd = password.ok_or_else(|| {
                                io::Error::new(io::ErrorKind::Other, "password required")
                            })?;
                            send_password(&mut stream, pwd)?;
                        }
                        5 => {
                            // MD5 password
                            let mut salt = [0u8; 4];
                            salt.copy_from_slice(&data[4..8]);
                            let pwd = password.ok_or_else(|| {
                                io::Error::new(io::ErrorKind::Other, "password required")
                            })?;
                            send_md5_password(&mut stream, user, pwd, &salt)?;
                        }
                        10 => {
                            // SASL authentication
                            let mechanisms = parse_sasl_mechanisms(&data[4..])?;
                            handle_sasl(&mut stream, &mechanisms, user, password)?;
                        }
                        _ => return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("unsupported auth type: {}", auth_type)
                        )),
                    }
                }
                b'E' => {
                    let error = parse_error_response(&data)?;
                    return Err(io::Error::new(io::ErrorKind::Other, error.message));
                }
                _ => {}
            }
        }
        
        // Read backend parameters and key data
        let mut params = HashMap::new();
        let mut backend_pid = 0i32;
        let mut secret_key = Vec::new();
        
        loop {
            let (msg_type, data) = read_message(&mut stream)?;
            
            match msg_type {
                b'S' => {
                    // ParameterStatus
                    let (name, value) = parse_parameter_status(&data)?;
                    params.insert(name, value);
                }
                b'K' => {
                    // BackendKeyData
                    backend_pid = (&data[..4]).read_i32::<BigEndian>()?;
                    secret_key = data[4..].to_vec();
                }
                b'Z' => {
                    // ReadyForQuery
                    break;
                }
                b'E' => {
                    let error = parse_error_response(&data)?;
                    return Err(io::Error::new(io::ErrorKind::Other, error.message));
                }
                _ => {}
            }
        }
        
        Ok(Connection {
            stream,
            params,
            backend_pid,
            secret_key,
        })
    }
}

fn read_message(stream: &mut TcpStream) -> io::Result<(u8, Vec<u8>)> {
    let msg_type = stream.read_u8()?;
    let len = stream.read_i32::<BigEndian>()? as usize - 4;
    let mut data = vec![0u8; len];
    stream.read_exact(&mut data)?;
    Ok((msg_type, data))
}

fn send_password(stream: &mut TcpStream, password: &str) -> io::Result<()> {
    let mut buf = MessageBuffer::new();
    buf.start(b'p');
    buf.write_cstr(password);
    stream.write_all(buf.finish())
}

fn send_md5_password(
    stream: &mut TcpStream,
    user: &str,
    password: &str,
    salt: &[u8; 4],
) -> io::Result<()> {
    use md5::{Md5, Digest};
    
    // md5(md5(password + user) + salt)
    let mut hasher = Md5::new();
    hasher.update(password.as_bytes());
    hasher.update(user.as_bytes());
    let inner = format!("{:x}", hasher.finalize());
    
    let mut hasher = Md5::new();
    hasher.update(inner.as_bytes());
    hasher.update(salt);
    let hash = format!("md5{:x}", hasher.finalize());
    
    send_password(stream, &hash)
}
```

### Simple Query Execution

```rust
/// Query result types
#[derive(Debug)]
pub struct RowDescription {
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug)]
pub struct ColumnInfo {
    pub name: String,
    pub table_oid: Oid,
    pub column_id: i16,
    pub type_oid: Oid,
    pub type_size: i16,
    pub type_modifier: i32,
    pub format: i16,
}

#[derive(Debug)]
pub struct DataRow {
    pub values: Vec<Option<Vec<u8>>>,
}

#[derive(Debug)]
pub enum QueryResult {
    Select {
        columns: RowDescription,
        rows: Vec<DataRow>,
        tag: String,
    },
    Command {
        tag: String,
    },
    Empty,
}

impl Connection {
    /// Execute a simple query
    pub fn simple_query(&mut self, sql: &str) -> io::Result<Vec<QueryResult>> {
        let mut buf = MessageBuffer::new();
        buf.start(b'Q');
        buf.write_cstr(sql);
        self.stream.write_all(buf.finish())?;
        
        let mut results = Vec::new();
        let mut columns: Option<RowDescription> = None;
        let mut rows = Vec::new();
        
        loop {
            let (msg_type, data) = read_message(&mut self.stream)?;
            
            match msg_type {
                b'T' => {
                    // RowDescription
                    columns = Some(parse_row_description(&data)?);
                    rows.clear();
                }
                b'D' => {
                    // DataRow
                    rows.push(parse_data_row(&data)?);
                }
                b'C' => {
                    // CommandComplete
                    let tag = parse_command_tag(&data)?;
                    if let Some(cols) = columns.take() {
                        results.push(QueryResult::Select {
                            columns: cols,
                            rows: std::mem::take(&mut rows),
                            tag,
                        });
                    } else {
                        results.push(QueryResult::Command { tag });
                    }
                }
                b'I' => {
                    // EmptyQueryResponse
                    results.push(QueryResult::Empty);
                }
                b'E' => {
                    let error = parse_error_response(&data)?;
                    return Err(io::Error::new(io::ErrorKind::Other, error.message));
                }
                b'Z' => {
                    // ReadyForQuery
                    break;
                }
                _ => {}
            }
        }
        
        Ok(results)
    }
}

fn parse_row_description(data: &[u8]) -> io::Result<RowDescription> {
    let mut cursor = std::io::Cursor::new(data);
    let num_fields = cursor.read_i16::<BigEndian>()?;
    let mut columns = Vec::with_capacity(num_fields as usize);
    
    for _ in 0..num_fields {
        let name = read_cstring(&mut cursor)?;
        let table_oid = cursor.read_u32::<BigEndian>()?;
        let column_id = cursor.read_i16::<BigEndian>()?;
        let type_oid = cursor.read_u32::<BigEndian>()?;
        let type_size = cursor.read_i16::<BigEndian>()?;
        let type_modifier = cursor.read_i32::<BigEndian>()?;
        let format = cursor.read_i16::<BigEndian>()?;
        
        columns.push(ColumnInfo {
            name,
            table_oid,
            column_id,
            type_oid,
            type_size,
            type_modifier,
            format,
        });
    }
    
    Ok(RowDescription { columns })
}

fn parse_data_row(data: &[u8]) -> io::Result<DataRow> {
    let mut cursor = std::io::Cursor::new(data);
    let num_cols = cursor.read_i16::<BigEndian>()?;
    let mut values = Vec::with_capacity(num_cols as usize);
    
    for _ in 0..num_cols {
        let len = cursor.read_i32::<BigEndian>()?;
        if len < 0 {
            values.push(None);
        } else {
            let mut value = vec![0u8; len as usize];
            cursor.read_exact(&mut value)?;
            values.push(Some(value));
        }
    }
    
    Ok(DataRow { values })
}

fn read_cstring<R: Read>(reader: &mut R) -> io::Result<String> {
    let mut bytes = Vec::new();
    loop {
        let b = reader.read_u8()?;
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn parse_command_tag(data: &[u8]) -> io::Result<String> {
    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    String::from_utf8(data[..end].to_vec())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
```

### Extended Query Execution

```rust
impl Connection {
    /// Execute prepared statement with parameters
    pub fn execute_prepared(
        &mut self,
        sql: &str,
        params: &[&dyn ToSql],
    ) -> io::Result<Vec<QueryResult>> {
        let mut buf = MessageBuffer::new();
        
        // Parse message
        buf.start(b'P');
        buf.write_cstr(""); // Unnamed statement
        buf.write_cstr(sql);
        buf.write_i16(0); // No parameter type hints
        self.stream.write_all(buf.finish())?;
        
        // Bind message
        buf.start(b'B');
        buf.write_cstr(""); // Unnamed portal
        buf.write_cstr(""); // Unnamed statement
        buf.write_i16(1); // One format code
        buf.write_i16(0); // Text format for all params
        buf.write_i16(params.len() as i16);
        
        for param in params {
            if let Some(bytes) = param.to_sql_bytes() {
                buf.write_i32(bytes.len() as i32);
                buf.write_bytes(&bytes);
            } else {
                buf.write_i32(-1); // NULL
            }
        }
        
        buf.write_i16(1); // One result format code
        buf.write_i16(0); // Text format for results
        self.stream.write_all(buf.finish())?;
        
        // Describe portal
        buf.start(b'D');
        buf.buf.push(b'P');
        buf.write_cstr("");
        self.stream.write_all(buf.finish())?;
        
        // Execute
        buf.start(b'E');
        buf.write_cstr("");
        buf.write_i32(0); // No row limit
        self.stream.write_all(buf.finish())?;
        
        // Sync
        buf.start(b'S');
        self.stream.write_all(buf.finish())?;
        
        // Read responses
        let mut columns: Option<RowDescription> = None;
        let mut rows = Vec::new();
        let mut results = Vec::new();
        
        loop {
            let (msg_type, data) = read_message(&mut self.stream)?;
            
            match msg_type {
                b'1' => {} // ParseComplete
                b'2' => {} // BindComplete
                b't' => {} // ParameterDescription
                b'T' => {
                    columns = Some(parse_row_description(&data)?);
                }
                b'n' => {
                    columns = Some(RowDescription { columns: vec![] });
                }
                b'D' => {
                    rows.push(parse_data_row(&data)?);
                }
                b'C' => {
                    let tag = parse_command_tag(&data)?;
                    if let Some(cols) = columns.take() {
                        results.push(QueryResult::Select {
                            columns: cols,
                            rows: std::mem::take(&mut rows),
                            tag,
                        });
                    } else {
                        results.push(QueryResult::Command { tag });
                    }
                }
                b'E' => {
                    let error = parse_error_response(&data)?;
                    return Err(io::Error::new(io::ErrorKind::Other, error.message));
                }
                b'Z' => break,
                _ => {}
            }
        }
        
        Ok(results)
    }
}

/// Trait for types that can be converted to SQL parameters
pub trait ToSql {
    fn to_sql_bytes(&self) -> Option<Vec<u8>>;
}

impl ToSql for i32 {
    fn to_sql_bytes(&self) -> Option<Vec<u8>> {
        Some(self.to_string().into_bytes())
    }
}

impl ToSql for i64 {
    fn to_sql_bytes(&self) -> Option<Vec<u8>> {
        Some(self.to_string().into_bytes())
    }
}

impl ToSql for &str {
    fn to_sql_bytes(&self) -> Option<Vec<u8>> {
        Some(self.as_bytes().to_vec())
    }
}

impl ToSql for String {
    fn to_sql_bytes(&self) -> Option<Vec<u8>> {
        Some(self.as_bytes().to_vec())
    }
}

impl<T: ToSql> ToSql for Option<T> {
    fn to_sql_bytes(&self) -> Option<Vec<u8>> {
        self.as_ref().and_then(|v| v.to_sql_bytes())
    }
}
```

### Error Handling

```rust
/// PostgreSQL error/notice response
#[derive(Debug, Clone)]
pub struct PgError {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub position: Option<i32>,
    pub schema: Option<String>,
    pub table: Option<String>,
    pub column: Option<String>,
    pub datatype: Option<String>,
    pub constraint: Option<String>,
}

fn parse_error_response(data: &[u8]) -> io::Result<PgError> {
    let mut error = PgError {
        severity: String::new(),
        code: String::new(),
        message: String::new(),
        detail: None,
        hint: None,
        position: None,
        schema: None,
        table: None,
        column: None,
        datatype: None,
        constraint: None,
    };
    
    let mut cursor = std::io::Cursor::new(data);
    
    loop {
        let field_type = cursor.read_u8()?;
        if field_type == 0 {
            break;
        }
        
        let value = read_cstring(&mut cursor)?;
        
        match field_type {
            b'S' => error.severity = value,
            b'C' => error.code = value,
            b'M' => error.message = value,
            b'D' => error.detail = Some(value),
            b'H' => error.hint = Some(value),
            b'P' => error.position = value.parse().ok(),
            b's' => error.schema = Some(value),
            b't' => error.table = Some(value),
            b'c' => error.column = Some(value),
            b'd' => error.datatype = Some(value),
            b'n' => error.constraint = Some(value),
            _ => {} // Ignore unknown fields
        }
    }
    
    Ok(error)
}

fn parse_parameter_status(data: &[u8]) -> io::Result<(String, String)> {
    let mut cursor = std::io::Cursor::new(data);
    let name = read_cstring(&mut cursor)?;
    let value = read_cstring(&mut cursor)?;
    Ok((name, value))
}

fn parse_sasl_mechanisms(data: &[u8]) -> io::Result<Vec<String>> {
    let mut mechanisms = Vec::new();
    let mut cursor = std::io::Cursor::new(data);
    
    loop {
        let mech = read_cstring(&mut cursor)?;
        if mech.is_empty() {
            break;
        }
        mechanisms.push(mech);
    }
    
    Ok(mechanisms)
}

fn handle_sasl(
    _stream: &mut TcpStream,
    _mechanisms: &[String],
    _user: &str,
    _password: Option<&str>,
) -> io::Result<()> {
    // SCRAM-SHA-256 implementation would go here
    // This is a placeholder for the full implementation
    Err(io::Error::new(io::ErrorKind::Other, "SASL not implemented"))
}
```

### Cargo Dependencies

```toml
[dependencies]
# For protocol implementation
byteorder = "1.5"
md5 = "0.7"

# For SCRAM authentication (optional)
scram = "0.6"

# For AST parsing (optional - use libpg_query bindings)
pg_query = "5.1"  # Rust bindings to libpg_query

# For async support (optional)
tokio = { version = "1", features = ["net", "io-util"] }
```

### Complete AST Builder Example

```rust
/// Build a SELECT statement programmatically
fn build_select_example() -> SelectStmt {
    // SELECT u.id, u.name, COUNT(o.id) as order_count
    // FROM users u
    // LEFT JOIN orders o ON o.user_id = u.id
    // WHERE u.active = true AND u.created_at > $1
    // GROUP BY u.id, u.name
    // HAVING COUNT(o.id) > 0
    // ORDER BY order_count DESC
    // LIMIT 10
    
    SelectStmt {
        target_list: vec![
            // u.id
            ResTarget::expr(Node::ColumnRef(ColumnRef::qualified("u", "id"))),
            // u.name
            ResTarget::expr(Node::ColumnRef(ColumnRef::qualified("u", "name"))),
            // COUNT(o.id) as order_count
            ResTarget::aliased(
                Node::FuncCall(Box::new(FuncCall::new(
                    "count",
                    vec![Node::ColumnRef(ColumnRef::qualified("o", "id"))]
                ))),
                "order_count"
            ),
        ],
        from_clause: vec![
            // FROM users u LEFT JOIN orders o ON o.user_id = u.id
            Node::JoinExpr(Box::new(JoinExpr {
                jointype: JoinType::Left,
                is_natural: false,
                larg: Box::new(Node::RangeVar(RangeVar::new("users").aliased("u"))),
                rarg: Box::new(Node::RangeVar(RangeVar::new("orders").aliased("o"))),
                using_clause: vec![],
                join_using_alias: None,
                quals: Some(Box::new(Node::AExpr(Box::new(AExpr::eq(
                    Node::ColumnRef(ColumnRef::qualified("o", "user_id")),
                    Node::ColumnRef(ColumnRef::qualified("u", "id")),
                ))))),
                alias: None,
                rtindex: 0,
            })),
        ],
        where_clause: Some(Box::new(Node::BoolExpr(Box::new(BoolExpr::and(vec![
            // u.active = true
            Node::AExpr(Box::new(AExpr::eq(
                Node::ColumnRef(ColumnRef::qualified("u", "active")),
                Node::AConst(AConst::boolean(true)),
            ))),
            // u.created_at > $1
            Node::AExpr(Box::new(AExpr::binary_op(
                ">",
                Node::ColumnRef(ColumnRef::qualified("u", "created_at")),
                Node::ParamRef(ParamRef::new(1)),
            ))),
        ]))))),
        group_clause: vec![
            Node::ColumnRef(ColumnRef::qualified("u", "id")),
            Node::ColumnRef(ColumnRef::qualified("u", "name")),
        ],
        having_clause: Some(Box::new(Node::AExpr(Box::new(AExpr::binary_op(
            ">",
            Node::FuncCall(Box::new(FuncCall::new(
                "count",
                vec![Node::ColumnRef(ColumnRef::qualified("o", "id"))]
            ))),
            Node::AConst(AConst::integer(0)),
        ))))),
        sort_clause: vec![
            SortBy::desc(Node::ColumnRef(ColumnRef::new("order_count"))),
        ],
        limit_count: Some(Box::new(Node::AConst(AConst::integer(10)))),
        ..Default::default()
    }
}

/// Build an INSERT statement
fn build_insert_example() -> InsertStmt {
    // INSERT INTO users (name, email, age)
    // VALUES ($1, $2, $3)
    // ON CONFLICT (email) DO UPDATE SET name = EXCLUDED.name
    // RETURNING id, name
    
    InsertStmt {
        relation: RangeVar::new("users"),
        cols: vec![
            ResTarget::column("name"),
            ResTarget::column("email"),
            ResTarget::column("age"),
        ],
        select_stmt: Some(Box::new(Node::SelectStmt(Box::new(SelectStmt {
            values_lists: vec![vec![
                Node::ParamRef(ParamRef::new(1)),
                Node::ParamRef(ParamRef::new(2)),
                Node::ParamRef(ParamRef::new(3)),
            ]],
            ..Default::default()
        })))),
        on_conflict_clause: Some(OnConflictClause {
            action: OnConflictAction::Update,
            infer: Some(InferClause {
                index_elems: vec![IndexElem { name: "email".into(), ..Default::default() }],
                where_clause: None,
                conname: None,
                location: -1,
            }),
            target_list: vec![
                ResTarget {
                    name: Some("name".into()),
                    indirection: vec![],
                    val: Some(Box::new(Node::ColumnRef(ColumnRef::qualified("excluded", "name")))),
                    location: -1,
                },
            ],
            where_clause: None,
            location: -1,
        }),
        returning_list: vec![
            ResTarget::expr(Node::ColumnRef(ColumnRef::new("id"))),
            ResTarget::expr(Node::ColumnRef(ColumnRef::new("name"))),
        ],
        ..Default::default()
    }
}

// Supporting types for the example
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InferClause {
    pub index_elems: Vec<IndexElem>,
    pub where_clause: Option<Box<Node>>,
    pub conname: Option<String>,
    pub location: i32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct IndexElem {
    pub name: String,
    pub expr: Option<Box<Node>>,
    pub indexcolname: Option<String>,
    pub collation: Vec<String>,
    pub opclass: Vec<String>,
    pub opclassopts: Vec<Node>,
    pub ordering: SortByDir,
    pub nulls_ordering: SortByNulls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdType {
    Unknown,
    Select,
    Update,
    Insert,
    Delete,
    Merge,
    Utility,
    Nothing,
}
```

---

*Document Version: 1.1*
*PostgreSQL Version: 18.1*
*Protocol Version: 3.2*
*Primary Language: Rust*
*Last Updated: December 2025*
