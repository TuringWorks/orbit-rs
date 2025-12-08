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

Every AST node begins with a `NodeTag` identifying its type:

```c
typedef enum NodeTag {
    T_Invalid = 0,
    
    // Primitive nodes
    T_Alias,
    T_RangeVar,
    T_TableFunc,
    T_Var,
    T_Const,
    T_Param,
    ...
    
    // Statement nodes
    T_SelectStmt,
    T_InsertStmt,
    T_UpdateStmt,
    T_DeleteStmt,
    T_CreateStmt,
    T_AlterTableStmt,
    ...
    
    // Expression nodes
    T_A_Expr,
    T_ColumnRef,
    T_ParamRef,
    T_A_Const,
    T_FuncCall,
    T_A_Star,
    T_A_Indices,
    T_A_Indirection,
    T_SubLink,
    T_CaseExpr,
    T_CaseWhen,
    T_CoalesceExpr,
    T_NullTest,
    T_BoolExpr,
    ...
    
    // List types
    T_List,
    T_IntList,
    T_OidList,
    ...
} NodeTag;
```

### Core Statement Nodes

#### SelectStmt

```c
typedef struct SelectStmt {
    NodeTag     type;
    List       *distinctClause;    // DISTINCT ON expressions
    IntoClause *intoClause;        // SELECT INTO target
    List       *targetList;        // ResTarget list
    List       *fromClause;        // FROM clause
    Node       *whereClause;       // WHERE clause
    List       *groupClause;       // GROUP BY clause
    bool        groupDistinct;     // GROUP BY DISTINCT
    Node       *havingClause;      // HAVING clause
    List       *windowClause;      // WINDOW definitions
    List       *valuesLists;       // VALUES lists
    List       *sortClause;        // ORDER BY clause
    Node       *limitOffset;       // OFFSET expression
    Node       *limitCount;        // LIMIT expression
    LimitOption limitOption;       // LIMIT option
    List       *lockingClause;     // FOR UPDATE/SHARE
    WithClause *withClause;        // WITH clause
    SetOperation op;               // UNION/INTERSECT/EXCEPT
    bool        all;               // ALL modifier
    struct SelectStmt *larg;       // Left operand
    struct SelectStmt *rarg;       // Right operand
} SelectStmt;
```

#### InsertStmt

```c
typedef struct InsertStmt {
    NodeTag     type;
    RangeVar   *relation;          // Target table
    List       *cols;              // Column list
    Node       *selectStmt;        // SELECT or VALUES
    OnConflictClause *onConflictClause;  // ON CONFLICT
    List       *returningList;     // RETURNING clause
    WithClause *withClause;        // WITH clause
    OverridingKind override;       // OVERRIDING
} InsertStmt;
```

#### UpdateStmt

```c
typedef struct UpdateStmt {
    NodeTag     type;
    RangeVar   *relation;          // Target table
    List       *targetList;        // SET assignments
    Node       *whereClause;       // WHERE clause
    List       *fromClause;        // FROM clause
    List       *returningList;     // RETURNING clause
    WithClause *withClause;        // WITH clause
} UpdateStmt;
```

#### DeleteStmt

```c
typedef struct DeleteStmt {
    NodeTag     type;
    RangeVar   *relation;          // Target table
    List       *usingClause;       // USING clause
    Node       *whereClause;       // WHERE clause
    List       *returningList;     // RETURNING clause
    WithClause *withClause;        // WITH clause
} DeleteStmt;
```

### Expression Nodes

#### ColumnRef (Column Reference)

```c
typedef struct ColumnRef {
    NodeTag     type;
    List       *fields;            // Field names (String nodes)
    int         location;          // Token location
} ColumnRef;
```

#### A_Const (Constant Value)

```c
typedef struct A_Const {
    NodeTag     type;
    union ValUnion {
        Integer ival;
        Float   fval;
        Boolean boolval;
        String  sval;
        BitString bsval;
    } val;
    bool        isnull;            // NULL constant
    int         location;          // Token location
} A_Const;
```

#### FuncCall (Function Call)

```c
typedef struct FuncCall {
    NodeTag     type;
    List       *funcname;          // Qualified function name
    List       *args;              // Arguments
    List       *agg_order;         // ORDER BY (aggregates)
    Node       *agg_filter;        // FILTER clause
    struct WindowDef *over;        // OVER clause
    bool        agg_within_group;  // WITHIN GROUP
    bool        agg_star;          // COUNT(*)
    bool        agg_distinct;      // DISTINCT
    bool        func_variadic;     // VARIADIC argument
    CoercionForm funcformat;       // How to display
    int         location;          // Token location
} FuncCall;
```

#### A_Expr (Generic Expression)

```c
typedef struct A_Expr {
    NodeTag     type;
    A_Expr_Kind kind;              // Expression type
    List       *name;              // Operator name
    Node       *lexpr;             // Left operand
    Node       *rexpr;             // Right operand
    int         location;          // Token location
} A_Expr;

typedef enum A_Expr_Kind {
    AEXPR_OP,                      // Normal operator
    AEXPR_OP_ANY,                  // scalar op ANY (array)
    AEXPR_OP_ALL,                  // scalar op ALL (array)
    AEXPR_DISTINCT,                // IS DISTINCT FROM
    AEXPR_NOT_DISTINCT,            // IS NOT DISTINCT FROM
    AEXPR_NULLIF,                  // NULLIF
    AEXPR_IN,                      // IN
    AEXPR_LIKE,                    // [NOT] LIKE
    AEXPR_ILIKE,                   // [NOT] ILIKE
    AEXPR_SIMILAR,                 // [NOT] SIMILAR TO
    AEXPR_BETWEEN,                 // BETWEEN
    AEXPR_NOT_BETWEEN,             // NOT BETWEEN
    AEXPR_BETWEEN_SYM,             // BETWEEN SYMMETRIC
    AEXPR_NOT_BETWEEN_SYM          // NOT BETWEEN SYMMETRIC
} A_Expr_Kind;
```

#### BoolExpr (Boolean Expression)

```c
typedef struct BoolExpr {
    NodeTag     type;
    BoolExprType boolop;           // AND, OR, NOT
    List       *args;              // Argument list
    int         location;          // Token location
} BoolExpr;

typedef enum BoolExprType {
    AND_EXPR,
    OR_EXPR,
    NOT_EXPR
} BoolExprType;
```

### Utility Nodes

#### RangeVar (Table Reference)

```c
typedef struct RangeVar {
    NodeTag     type;
    char       *catalogname;       // Catalog (database)
    char       *schemaname;        // Schema
    char       *relname;           // Table name
    bool        inh;               // Inheritance (usually true)
    char        relpersistence;    // Persistence type
    Alias      *alias;             // Table alias
    int         location;          // Token location
} RangeVar;
```

#### ResTarget (Result Column)

```c
typedef struct ResTarget {
    NodeTag     type;
    char       *name;              // Column alias
    List       *indirection;       // Subscripts/field selection
    Node       *val;               // Value expression
    int         location;          // Token location
} ResTarget;
```

---

## Data Types and OIDs

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

```python
# Pseudo-code for parameter binding
def bind_parameters(statement_name, portal_name, params, param_formats, result_formats):
    msg = bytearray()
    msg.append(ord('B'))  # Bind message type
    
    # Length placeholder
    length_pos = len(msg)
    msg.extend(b'\x00\x00\x00\x00')
    
    # Portal name (empty for unnamed)
    msg.extend(portal_name.encode() + b'\x00')
    
    # Statement name (empty for unnamed)
    msg.extend(statement_name.encode() + b'\x00')
    
    # Parameter format codes
    msg.extend(struct.pack('>H', len(param_formats)))
    for fmt in param_formats:
        msg.extend(struct.pack('>H', fmt))
    
    # Parameter values
    msg.extend(struct.pack('>H', len(params)))
    for param in params:
        if param is None:
            msg.extend(struct.pack('>i', -1))
        else:
            data = encode_param(param)
            msg.extend(struct.pack('>i', len(data)))
            msg.extend(data)
    
    # Result format codes
    msg.extend(struct.pack('>H', len(result_formats)))
    for fmt in result_formats:
        msg.extend(struct.pack('>H', fmt))
    
    # Fill in length
    length = len(msg) - 1
    struct.pack_into('>I', msg, length_pos, length)
    
    return bytes(msg)
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

## Appendix: Sample Implementation Patterns

### Connection Startup Sequence

```python
def connect(host, port, user, database):
    sock = socket.create_connection((host, port))
    
    # Send startup message
    params = {
        'user': user,
        'database': database,
        'client_encoding': 'UTF8'
    }
    startup = build_startup_message(params)
    sock.sendall(startup)
    
    # Handle authentication
    while True:
        msg_type, data = recv_message(sock)
        
        if msg_type == ord('R'):  # Authentication
            auth_type = struct.unpack('>I', data[:4])[0]
            
            if auth_type == 0:  # AuthenticationOk
                break
            elif auth_type == 3:  # CleartextPassword
                send_password(sock, password)
            elif auth_type == 5:  # MD5Password
                salt = data[4:8]
                send_md5_password(sock, user, password, salt)
            elif auth_type == 10:  # SASL
                handle_sasl(sock, data[4:])
        
        elif msg_type == ord('E'):  # Error
            raise PostgresError(parse_error(data))
    
    # Read backend parameters and key data
    params = {}
    backend_pid = None
    backend_key = None
    
    while True:
        msg_type, data = recv_message(sock)
        
        if msg_type == ord('S'):  # ParameterStatus
            name, value = parse_parameter_status(data)
            params[name] = value
        
        elif msg_type == ord('K'):  # BackendKeyData
            backend_pid = struct.unpack('>I', data[:4])[0]
            backend_key = data[4:]
        
        elif msg_type == ord('Z'):  # ReadyForQuery
            return Connection(sock, params, backend_pid, backend_key)
```

### Simple Query Execution

```python
def simple_query(conn, sql):
    # Send Query message
    msg = b'Q' + struct.pack('>I', len(sql) + 5) + sql.encode() + b'\x00'
    conn.sock.sendall(msg)
    
    results = []
    columns = None
    rows = []
    
    while True:
        msg_type, data = recv_message(conn.sock)
        
        if msg_type == ord('T'):  # RowDescription
            columns = parse_row_description(data)
            rows = []
        
        elif msg_type == ord('D'):  # DataRow
            row = parse_data_row(data, columns)
            rows.append(row)
        
        elif msg_type == ord('C'):  # CommandComplete
            tag = data[:-1].decode()
            if columns:
                results.append(QueryResult(columns, rows, tag))
            else:
                results.append(CommandResult(tag))
            columns = None
            rows = []
        
        elif msg_type == ord('I'):  # EmptyQueryResponse
            results.append(EmptyResult())
        
        elif msg_type == ord('E'):  # ErrorResponse
            raise PostgresError(parse_error(data))
        
        elif msg_type == ord('Z'):  # ReadyForQuery
            return results
```

### Extended Query Execution

```python
def execute_prepared(conn, sql, params):
    # Parse
    parse_msg = build_parse_message('', sql, [])
    conn.sock.sendall(parse_msg)
    
    # Bind
    bind_msg = build_bind_message('', '', params)
    conn.sock.sendall(bind_msg)
    
    # Describe
    describe_msg = b'D\x00\x00\x00\x06P\x00'
    conn.sock.sendall(describe_msg)
    
    # Execute
    execute_msg = b'E\x00\x00\x00\x09\x00\x00\x00\x00\x00'
    conn.sock.sendall(execute_msg)
    
    # Sync
    sync_msg = b'S\x00\x00\x00\x04'
    conn.sock.sendall(sync_msg)
    
    # Receive responses
    columns = None
    rows = []
    
    while True:
        msg_type, data = recv_message(conn.sock)
        
        if msg_type == ord('1'):  # ParseComplete
            pass
        elif msg_type == ord('2'):  # BindComplete
            pass
        elif msg_type == ord('t'):  # ParameterDescription
            pass
        elif msg_type == ord('T'):  # RowDescription
            columns = parse_row_description(data)
        elif msg_type == ord('n'):  # NoData
            columns = []
        elif msg_type == ord('D'):  # DataRow
            rows.append(parse_data_row(data, columns))
        elif msg_type == ord('C'):  # CommandComplete
            pass
        elif msg_type == ord('E'):  # ErrorResponse
            raise PostgresError(parse_error(data))
        elif msg_type == ord('Z'):  # ReadyForQuery
            return QueryResult(columns, rows)
```

---

*Document Version: 1.0*
*PostgreSQL Version: 18.1*
*Protocol Version: 3.2*
*Last Updated: December 2025*
