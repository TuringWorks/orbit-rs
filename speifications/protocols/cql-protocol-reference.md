# Cassandra Query Language (CQL) Protocol & Syntax Reference

A comprehensive technical reference for implementing CQL clients, parsers, and drivers in Rust.

---

## Table of Contents

1. [CQL Syntax Overview](#1-cql-syntax-overview)
2. [Complete CQL Keywords](#2-complete-cql-keywords)
3. [Abstract Syntax Tree (AST)](#3-abstract-syntax-tree-ast)
4. [Parser Implementation](#4-parser-implementation)
5. [CQL Native Protocol (Wire Protocol)](#5-cql-native-protocol-wire-protocol)
6. [Connection Management](#6-connection-management)
7. [Grammar Reference (BNF)](#7-grammar-reference-bnf)
8. [Useful Implementation Patterns](#8-useful-implementation-patterns)
9. [ScyllaDB Extensions](#9-scylladb-extensions)
10. [Error Handling Best Practices](#10-error-handling-best-practices)

---

## 1. CQL Syntax Overview

CQL is a SQL-like query language designed for Cassandra's distributed architecture. It's case-insensitive for keywords but case-sensitive for identifiers (unless quoted).

### 1.1 Lexical Structure

**Identifiers:**

```text
identifier     ::= unquoted_identifier | quoted_identifier
unquoted_identifier ::= [a-zA-Z][a-zA-Z0-9_]*
quoted_identifier   ::= '"' (~["\r\n] | '""')* '"'
```

**Literals:**

```text
string_literal  ::= '\'' (~['\r\n] | '\'\'')* '\''
integer_literal ::= '-'? [0-9]+
float_literal   ::= '-'? [0-9]+ ('.' [0-9]*)? ([eE] [+-]? [0-9]+)?
uuid_literal    ::= hex{8} '-' hex{4} '-' hex{4} '-' hex{4} '-' hex{12}
blob_literal    ::= '0' [xX] [0-9a-fA-F]+
boolean_literal ::= 'true' | 'false'
null_literal    ::= 'null'
```

**Comments:**

```text
single_line_comment ::= '--' ~[\r\n]* 
                      | '//' ~[\r\n]*
multi_line_comment  ::= '/*' .*? '*/'
```

---

## 2. Complete CQL Keywords

### 2.1 Reserved Keywords

```rust
pub const RESERVED_KEYWORDS: &[&str] = &[
    "ADD", "AGGREGATE", "ALL", "ALLOW", "ALTER", "AND", "ANY", "APPLY",
    "AS", "ASC", "ASCII", "AUTHORIZE", "BATCH", "BEGIN", "BIGINT", "BLOB",
    "BOOLEAN", "BY", "CALLED", "CAST", "CLUSTERING", "COLUMNFAMILY", "COMPACT",
    "CONTAINS", "COUNT", "COUNTER", "CREATE", "CUSTOM", "DATE", "DECIMAL",
    "DEFAULT", "DELETE", "DESC", "DESCRIBE", "DISTINCT", "DOUBLE", "DROP",
    "DURATION", "ENTRIES", "EXECUTE", "EXISTS", "FILTERING", "FINALFUNC",
    "FLOAT", "FROM", "FROZEN", "FULL", "FUNCTION", "FUNCTIONS", "GRANT",
    "GROUP", "IF", "IN", "INDEX", "INET", "INFINITY", "INITCOND", "INPUT",
    "INSERT", "INT", "INTO", "IS", "JSON", "KEY", "KEYSPACE", "KEYSPACES",
    "LANGUAGE", "LIKE", "LIMIT", "LIST", "LOGIN", "MAP", "MATERIALIZED",
    "MBEAN", "MBEANS", "MODIFY", "NAN", "NOLOGIN", "NORECURSIVE", "NOSUPERUSER",
    "NOT", "NULL", "OF", "ON", "OPTIONS", "OR", "ORDER", "PARTITION",
    "PASSWORD", "PER", "PERMISSION", "PERMISSIONS", "PRIMARY", "RENAME",
    "REPLACE", "RETURNS", "REVOKE", "ROLE", "ROLES", "SCHEMA", "SELECT",
    "SET", "SFUNC", "SMALLINT", "STATIC", "STORAGE", "STYPE", "SUPERUSER",
    "TABLE", "TEXT", "TIME", "TIMESTAMP", "TIMEUUID", "TINYINT", "TO",
    "TOKEN", "TRIGGER", "TRUNCATE", "TTL", "TUPLE", "TYPE", "UNLOGGED",
    "UPDATE", "USE", "USER", "USERS", "USING", "UUID", "VALUES", "VARCHAR",
    "VARINT", "VIEW", "WHERE", "WITH", "WRITETIME"
];
```

### 2.2 Data Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CqlType {
    // Native types
    Ascii,
    Bigint,
    Blob,
    Boolean,
    Counter,
    Date,
    Decimal,
    Double,
    Duration,
    Float,
    Inet,
    Int,
    Smallint,
    Text,
    Time,
    Timestamp,
    Timeuuid,
    Tinyint,
    Uuid,
    Varchar,
    Varint,
    
    // Collection types
    List(Box<CqlType>),
    Set(Box<CqlType>),
    Map(Box<CqlType>, Box<CqlType>),
    
    // Other complex types
    Tuple(Vec<CqlType>),
    Frozen(Box<CqlType>),
    UserDefined {
        keyspace: Option<String>,
        name: String,
    },
}
```

---

## 3. Abstract Syntax Tree (AST)

### 3.1 Top-Level Statement Enum

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // Data Manipulation
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    Batch(BatchStatement),
    
    // Schema Definition
    CreateKeyspace(CreateKeyspaceStatement),
    AlterKeyspace(AlterKeyspaceStatement),
    DropKeyspace(DropKeyspaceStatement),
    CreateTable(CreateTableStatement),
    AlterTable(AlterTableStatement),
    DropTable(DropTableStatement),
    Truncate(TruncateStatement),
    
    // Index
    CreateIndex(CreateIndexStatement),
    DropIndex(DropIndexStatement),
    
    // Materialized Views
    CreateMaterializedView(CreateMaterializedViewStatement),
    AlterMaterializedView(AlterMaterializedViewStatement),
    DropMaterializedView(DropMaterializedViewStatement),
    
    // User-Defined Types
    CreateType(CreateTypeStatement),
    AlterType(AlterTypeStatement),
    DropType(DropTypeStatement),
    
    // Functions and Aggregates
    CreateFunction(CreateFunctionStatement),
    DropFunction(DropFunctionStatement),
    CreateAggregate(CreateAggregateStatement),
    DropAggregate(DropAggregateStatement),
    
    // Security
    CreateRole(CreateRoleStatement),
    AlterRole(AlterRoleStatement),
    DropRole(DropRoleStatement),
    Grant(GrantStatement),
    Revoke(RevokeStatement),
    ListRoles(ListRolesStatement),
    ListPermissions(ListPermissionsStatement),
    
    // Other
    Use(UseStatement),
}
```

### 3.2 SELECT Statement AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub distinct: bool,
    pub json: bool,
    pub selectors: Vec<Selector>,
    pub from: TableName,
    pub where_clause: Option<WhereClause>,
    pub group_by: Option<Vec<ColumnName>>,
    pub order_by: Option<Vec<OrderingClause>>,
    pub per_partition_limit: Option<i32>,
    pub limit: Option<i32>,
    pub allow_filtering: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Selector {
    All,                                    // *
    Column(ColumnName),                     // column_name
    ColumnAs(ColumnName, Identifier),       // column_name AS alias
    Function(FunctionCall),                 // func(args)
    FunctionAs(FunctionCall, Identifier),   // func(args) AS alias
    Term(Term),                             // literal value
    Cast(Box<Selector>, CqlType),           // CAST(selector AS type)
    Count,                                  // COUNT(*)
    Writetime(ColumnName),                  // WRITETIME(column)
    Ttl(ColumnName),                        // TTL(column)
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhereClause {
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Relation {
    Simple {
        column: ColumnName,
        operator: RelationOperator,
        term: Term,
    },
    Token {
        columns: Vec<ColumnName>,
        operator: RelationOperator,
        term: Term,
    },
    In {
        column: ColumnName,
        terms: Vec<Term>,
    },
    InMarker {
        column: ColumnName,
        marker: BindMarker,
    },
    Contains {
        column: ColumnName,
        term: Term,
    },
    ContainsKey {
        column: ColumnName,
        term: Term,
    },
    Like {
        column: ColumnName,
        pattern: Term,
    },
    IsNot {
        column: ColumnName,
        // Always NULL for IS NOT NULL
    },
    MultiColumn {
        columns: Vec<ColumnName>,
        operator: RelationOperator,
        terms: Vec<Term>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationOperator {
    Equal,          // =
    NotEqual,       // != or <>
    LessThan,       // <
    LessEqual,      // <=
    GreaterThan,    // >
    GreaterEqual,   // >=
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderingClause {
    pub column: ColumnName,
    pub direction: OrderDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderDirection {
    Asc,
    Desc,
}
```

### 3.3 INSERT Statement AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStatement {
    pub table: TableName,
    pub insert_type: InsertType,
    pub if_not_exists: bool,
    pub using: Option<UpdateParameters>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertType {
    Values {
        columns: Vec<ColumnName>,
        values: Vec<Term>,
    },
    Json {
        json_value: Term,
        default_clause: Option<JsonDefault>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonDefault {
    Null,
    Unset,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateParameters {
    pub timestamp: Option<Term>,
    pub ttl: Option<Term>,
}
```

### 3.4 UPDATE Statement AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatement {
    pub table: TableName,
    pub using: Option<UpdateParameters>,
    pub assignments: Vec<Assignment>,
    pub where_clause: WhereClause,
    pub conditions: Option<Vec<Condition>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Assignment {
    Simple {
        column: ColumnName,
        term: Term,
    },
    Addition {
        column: ColumnName,
        term: Term,
    },
    Subtraction {
        column: ColumnName,
        term: Term,
    },
    ListPrepend {
        column: ColumnName,
        term: Term,
    },
    ListAppend {
        column: ColumnName,
        term: Term,
    },
    MapPut {
        column: ColumnName,
        key: Term,
        value: Term,
    },
    SetElement {
        column: ColumnName,
        index: Term,
        value: Term,
    },
    UdtField {
        column: ColumnName,
        field: Identifier,
        value: Term,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Condition {
    Exists,
    NotExists,
    Column {
        column: ColumnName,
        operator: RelationOperator,
        term: Term,
    },
    ColumnIn {
        column: ColumnName,
        terms: Vec<Term>,
    },
}
```

### 3.5 DELETE Statement AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStatement {
    pub selectors: Vec<DeleteSelector>,
    pub table: TableName,
    pub using: Option<UpdateParameters>,
    pub where_clause: WhereClause,
    pub conditions: Option<Vec<Condition>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeleteSelector {
    All,
    Column(ColumnName),
    Element {
        column: ColumnName,
        element: Term,
    },
    Field {
        column: ColumnName,
        field: Identifier,
    },
}
```

### 3.6 BATCH Statement AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BatchStatement {
    pub batch_type: BatchType,
    pub using: Option<UpdateParameters>,
    pub statements: Vec<BatchableStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BatchType {
    Logged,
    Unlogged,
    Counter,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BatchableStatement {
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
}
```

### 3.7 Schema Statements AST

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CreateKeyspaceStatement {
    pub if_not_exists: bool,
    pub name: Identifier,
    pub options: KeyspaceOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyspaceOptions {
    pub replication: ReplicationStrategy,
    pub durable_writes: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplicationStrategy {
    SimpleStrategy {
        replication_factor: i32,
    },
    NetworkTopologyStrategy {
        datacenters: HashMap<String, i32>,
    },
    Custom {
        class: String,
        options: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStatement {
    pub if_not_exists: bool,
    pub table: TableName,
    pub columns: Vec<ColumnDefinition>,
    pub primary_key: PrimaryKey,
    pub options: TableOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDefinition {
    pub name: Identifier,
    pub data_type: CqlType,
    pub is_static: bool,
    pub is_primary_key: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryKey {
    pub partition_key: Vec<Identifier>,
    pub clustering_columns: Vec<Identifier>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableOptions {
    pub bloom_filter_fp_chance: Option<f64>,
    pub caching: Option<CachingOptions>,
    pub comment: Option<String>,
    pub compaction: Option<CompactionOptions>,
    pub compression: Option<CompressionOptions>,
    pub crc_check_chance: Option<f64>,
    pub default_time_to_live: Option<i32>,
    pub gc_grace_seconds: Option<i32>,
    pub max_index_interval: Option<i32>,
    pub memtable_flush_period_in_ms: Option<i32>,
    pub min_index_interval: Option<i32>,
    pub speculative_retry: Option<String>,
    pub clustering_order: Option<Vec<(Identifier, OrderDirection)>>,
    pub compact_storage: bool,
}
```

### 3.8 Terms (Values/Expressions)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Term {
    // Literals
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Uuid(uuid::Uuid),
    Blob(Vec<u8>),
    Null,
    
    // Bind markers
    PositionalBind,              // ?
    NamedBind(Identifier),       // :name
    
    // Collections
    List(Vec<Term>),
    Set(Vec<Term>),
    Map(Vec<(Term, Term)>),
    Tuple(Vec<Term>),
    Udt(Vec<(Identifier, Term)>),
    
    // Function call
    Function(FunctionCall),
    
    // Arithmetic
    Addition(Box<Term>, Box<Term>),
    Subtraction(Box<Term>, Box<Term>),
    Negation(Box<Term>),
    
    // Type cast
    Cast(Box<Term>, CqlType),
    
    // Column reference (for conditions)
    ColumnRef(ColumnName),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionCall {
    pub keyspace: Option<Identifier>,
    pub name: Identifier,
    pub args: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableName {
    pub keyspace: Option<Identifier>,
    pub table: Identifier,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnName {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Identifier(pub String);

#[derive(Debug, Clone, PartialEq)]
pub enum BindMarker {
    Positional,
    Named(String),
}
```

---

## 4. Parser Implementation

### 4.1 Lexer (Tokenizer)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords (all reserved keywords become tokens)
    Select, From, Where, And, Or, Not, In, Is, As, 
    Create, Alter, Drop, Insert, Update, Delete,
    Keyspace, Table, Index, Type, Function, Aggregate,
    Primary, Key, Clustering, Static, Frozen,
    If, Exists, Null, True, False,
    Using, Timestamp, Ttl, Limit, Order, By, Asc, Desc,
    Allow, Filtering, Contains, Like, Token as TokenKw,
    Batch, Begin, Apply, Logged, Unlogged, Counter,
    Set, List, Map, Tuple,
    Grant, Revoke, Role, User, Permission,
    With, Options,
    Json, Default, Unset,
    Distinct, Group, Per, Partition,
    Materialized, View,
    Cast,
    // ... all other keywords
    
    // Identifiers and literals
    Identifier(String),
    QuotedIdentifier(String),
    StringLiteral(String),
    IntegerLiteral(i64),
    FloatLiteral(f64),
    UuidLiteral(uuid::Uuid),
    BlobLiteral(Vec<u8>),
    
    // Operators
    Eq,           // =
    Ne,           // != or <>
    Lt,           // <
    Le,           // <=
    Gt,           // >
    Ge,           // >=
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Percent,      // %
    
    // Punctuation
    LParen,       // (
    RParen,       // )
    LBracket,     // [
    RBracket,     // ]
    LBrace,       // {
    RBrace,       // }
    Comma,        // ,
    Semicolon,    // ;
    Colon,        // :
    Dot,          // .
    Question,     // ?
    
    // Special
    Eof,
    Error(String),
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            line: 1,
            column: 1,
        }
    }
    
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();
        
        if self.pos >= self.input.len() {
            return Token::Eof;
        }
        
        let ch = self.current_char();
        
        match ch {
            // Single character tokens
            '(' => { self.advance(); Token::LParen }
            ')' => { self.advance(); Token::RParen }
            '[' => { self.advance(); Token::LBracket }
            ']' => { self.advance(); Token::RBracket }
            '{' => { self.advance(); Token::LBrace }
            '}' => { self.advance(); Token::RBrace }
            ',' => { self.advance(); Token::Comma }
            ';' => { self.advance(); Token::Semicolon }
            ':' => { self.advance(); Token::Colon }
            '.' => { self.advance(); Token::Dot }
            '?' => { self.advance(); Token::Question }
            '*' => { self.advance(); Token::Star }
            '+' => { self.advance(); Token::Plus }
            '-' => { self.advance(); Token::Minus }
            '/' => { self.advance(); Token::Slash }
            '%' => { self.advance(); Token::Percent }
            
            // Multi-character operators
            '=' => { self.advance(); Token::Eq }
            '<' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Token::Le
                } else if self.current_char() == '>' {
                    self.advance();
                    Token::Ne
                } else {
                    Token::Lt
                }
            }
            '>' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Token::Ge
                } else {
                    Token::Gt
                }
            }
            '!' => {
                self.advance();
                if self.current_char() == '=' {
                    self.advance();
                    Token::Ne
                } else {
                    Token::Error("Expected '=' after '!'".into())
                }
            }
            
            // String literal
            '\'' => self.read_string_literal(),
            
            // Quoted identifier
            '"' => self.read_quoted_identifier(),
            
            // Number or blob
            '0'..='9' => self.read_number_or_blob(),
            
            // Identifier or keyword
            'a'..='z' | 'A'..='Z' | '_' => self.read_identifier_or_keyword(),
            
            _ => {
                self.advance();
                Token::Error(format!("Unexpected character: {}", ch))
            }
        }
    }
    
    fn read_string_literal(&mut self) -> Token {
        self.advance(); // consume opening '
        let mut value = String::new();
        
        loop {
            if self.pos >= self.input.len() {
                return Token::Error("Unterminated string literal".into());
            }
            
            let ch = self.current_char();
            if ch == '\'' {
                self.advance();
                // Check for escaped quote ''
                if self.current_char() == '\'' {
                    value.push('\'');
                    self.advance();
                } else {
                    break;
                }
            } else {
                value.push(ch);
                self.advance();
            }
        }
        
        Token::StringLiteral(value)
    }
    
    fn read_identifier_or_keyword(&mut self) -> Token {
        let start = self.pos;
        while self.pos < self.input.len() {
            let ch = self.current_char();
            if ch.is_alphanumeric() || ch == '_' {
                self.advance();
            } else {
                break;
            }
        }
        
        let ident = &self.input[start..self.pos];
        let upper = ident.to_uppercase();
        
        // Match against keywords (case-insensitive)
        match upper.as_str() {
            "SELECT" => Token::Select,
            "FROM" => Token::From,
            "WHERE" => Token::Where,
            "AND" => Token::And,
            "OR" => Token::Or,
            "NOT" => Token::Not,
            "IN" => Token::In,
            "IS" => Token::Is,
            "AS" => Token::As,
            "CREATE" => Token::Create,
            "ALTER" => Token::Alter,
            "DROP" => Token::Drop,
            "INSERT" => Token::Insert,
            "UPDATE" => Token::Update,
            "DELETE" => Token::Delete,
            "INTO" => Token::Into,
            "VALUES" => Token::Values,
            "SET" => Token::Set,
            "KEYSPACE" => Token::Keyspace,
            "TABLE" => Token::Table,
            "INDEX" => Token::Index,
            "PRIMARY" => Token::Primary,
            "KEY" => Token::Key,
            "NULL" => Token::Null,
            "TRUE" => Token::True,
            "FALSE" => Token::False,
            "USING" => Token::Using,
            "TIMESTAMP" => Token::Timestamp,
            "TTL" => Token::Ttl,
            "LIMIT" => Token::Limit,
            "ORDER" => Token::Order,
            "BY" => Token::By,
            "ASC" => Token::Asc,
            "DESC" => Token::Desc,
            "ALLOW" => Token::Allow,
            "FILTERING" => Token::Filtering,
            "IF" => Token::If,
            "EXISTS" => Token::Exists,
            "BATCH" => Token::Batch,
            "BEGIN" => Token::Begin,
            "APPLY" => Token::Apply,
            "WITH" => Token::With,
            "JSON" => Token::Json,
            "DISTINCT" => Token::Distinct,
            "GROUP" => Token::Group,
            "TOKEN" => Token::TokenKw,
            "CONTAINS" => Token::Contains,
            "LIKE" => Token::Like,
            // ... more keywords
            _ => Token::Identifier(ident.to_string()),
        }
    }
    
    // ... other helper methods
}
```

### 4.2 Parser Structure

```rust
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current_token: Token,
    peek_token: Token,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let mut lexer = Lexer::new(input);
        let current_token = lexer.next_token();
        let peek_token = lexer.next_token();
        
        Self {
            lexer,
            current_token,
            peek_token,
        }
    }
    
    fn advance(&mut self) {
        self.current_token = std::mem::replace(
            &mut self.peek_token, 
            self.lexer.next_token()
        );
    }
    
    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        if std::mem::discriminant(&self.current_token) == 
           std::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: format!("{:?}", expected),
                found: format!("{:?}", self.current_token),
            })
        }
    }
    
    pub fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.current_token {
            Token::Select => self.parse_select(),
            Token::Insert => self.parse_insert(),
            Token::Update => self.parse_update(),
            Token::Delete => self.parse_delete(),
            Token::Create => self.parse_create(),
            Token::Alter => self.parse_alter(),
            Token::Drop => self.parse_drop(),
            Token::Batch | Token::Begin => self.parse_batch(),
            Token::Use => self.parse_use(),
            Token::Grant => self.parse_grant(),
            Token::Revoke => self.parse_revoke(),
            Token::Truncate => self.parse_truncate(),
            _ => Err(ParseError::UnexpectedToken {
                expected: "statement".into(),
                found: format!("{:?}", self.current_token),
            }),
        }
    }
    
    fn parse_select(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Select)?;
        
        // DISTINCT
        let distinct = if self.current_token == Token::Distinct {
            self.advance();
            true
        } else {
            false
        };
        
        // JSON
        let json = if self.current_token == Token::Json {
            self.advance();
            true
        } else {
            false
        };
        
        // Selectors
        let selectors = self.parse_selectors()?;
        
        // FROM
        self.expect(Token::From)?;
        let from = self.parse_table_name()?;
        
        // WHERE (optional)
        let where_clause = if self.current_token == Token::Where {
            self.advance();
            Some(self.parse_where_clause()?)
        } else {
            None
        };
        
        // GROUP BY (optional)
        let group_by = if self.current_token == Token::Group {
            self.advance();
            self.expect(Token::By)?;
            Some(self.parse_column_list()?)
        } else {
            None
        };
        
        // ORDER BY (optional)
        let order_by = if self.current_token == Token::Order {
            self.advance();
            self.expect(Token::By)?;
            Some(self.parse_ordering_clauses()?)
        } else {
            None
        };
        
        // PER PARTITION LIMIT (optional)
        let per_partition_limit = if self.current_token == Token::Per {
            self.advance();
            self.expect(Token::Partition)?;
            self.expect(Token::Limit)?;
            Some(self.parse_integer()?)
        } else {
            None
        };
        
        // LIMIT (optional)
        let limit = if self.current_token == Token::Limit {
            self.advance();
            Some(self.parse_integer()?)
        } else {
            None
        };
        
        // ALLOW FILTERING (optional)
        let allow_filtering = if self.current_token == Token::Allow {
            self.advance();
            self.expect(Token::Filtering)?;
            true
        } else {
            false
        };
        
        Ok(Statement::Select(SelectStatement {
            distinct,
            json,
            selectors,
            from,
            where_clause,
            group_by,
            order_by,
            per_partition_limit,
            limit,
            allow_filtering,
        }))
    }
    
    fn parse_insert(&mut self) -> Result<Statement, ParseError> {
        self.expect(Token::Insert)?;
        self.expect(Token::Into)?;
        
        let table = self.parse_table_name()?;
        
        let insert_type = if self.current_token == Token::Json {
            self.advance();
            let json_value = self.parse_term()?;
            let default_clause = if self.current_token == Token::Default {
                self.advance();
                if self.current_token == Token::Null {
                    self.advance();
                    Some(JsonDefault::Null)
                } else if self.current_token == Token::Unset {
                    self.advance();
                    Some(JsonDefault::Unset)
                } else {
                    return Err(ParseError::ExpectedToken("NULL or UNSET".into()));
                }
            } else {
                None
            };
            InsertType::Json { json_value, default_clause }
        } else {
            self.expect(Token::LParen)?;
            let columns = self.parse_identifier_list()?;
            self.expect(Token::RParen)?;
            self.expect(Token::Values)?;
            self.expect(Token::LParen)?;
            let values = self.parse_term_list()?;
            self.expect(Token::RParen)?;
            InsertType::Values { columns, values }
        };
        
        let if_not_exists = self.parse_if_not_exists()?;
        let using = self.parse_using_clause()?;
        
        Ok(Statement::Insert(InsertStatement {
            table,
            insert_type,
            if_not_exists,
            using,
        }))
    }
    
    fn parse_where_clause(&mut self) -> Result<WhereClause, ParseError> {
        let mut relations = vec![self.parse_relation()?];
        
        while self.current_token == Token::And {
            self.advance();
            relations.push(self.parse_relation()?);
        }
        
        Ok(WhereClause { relations })
    }
    
    fn parse_relation(&mut self) -> Result<Relation, ParseError> {
        // Handle TOKEN(...) specially
        if self.current_token == Token::TokenKw {
            self.advance();
            self.expect(Token::LParen)?;
            let columns = self.parse_column_list()?;
            self.expect(Token::RParen)?;
            let operator = self.parse_relation_operator()?;
            let term = self.parse_term()?;
            return Ok(Relation::Token { columns, operator, term });
        }
        
        // Regular column relation
        let column = self.parse_column_name()?;
        
        // Check for special operators
        if self.current_token == Token::In {
            self.advance();
            self.expect(Token::LParen)?;
            if self.current_token == Token::Question {
                self.advance();
                self.expect(Token::RParen)?;
                return Ok(Relation::InMarker { 
                    column, 
                    marker: BindMarker::Positional 
                });
            }
            let terms = self.parse_term_list()?;
            self.expect(Token::RParen)?;
            return Ok(Relation::In { column, terms });
        }
        
        if self.current_token == Token::Contains {
            self.advance();
            if self.current_token == Token::Key {
                self.advance();
                let term = self.parse_term()?;
                return Ok(Relation::ContainsKey { column, term });
            }
            let term = self.parse_term()?;
            return Ok(Relation::Contains { column, term });
        }
        
        if self.current_token == Token::Like {
            self.advance();
            let pattern = self.parse_term()?;
            return Ok(Relation::Like { column, pattern });
        }
        
        if self.current_token == Token::Is {
            self.advance();
            self.expect(Token::Not)?;
            self.expect(Token::Null)?;
            return Ok(Relation::IsNot { column });
        }
        
        // Standard comparison
        let operator = self.parse_relation_operator()?;
        let term = self.parse_term()?;
        
        Ok(Relation::Simple { column, operator, term })
    }
    
    fn parse_term(&mut self) -> Result<Term, ParseError> {
        match &self.current_token {
            Token::StringLiteral(s) => {
                let value = s.clone();
                self.advance();
                Ok(Term::String(value))
            }
            Token::IntegerLiteral(i) => {
                let value = *i;
                self.advance();
                Ok(Term::Integer(value))
            }
            Token::FloatLiteral(f) => {
                let value = *f;
                self.advance();
                Ok(Term::Float(value))
            }
            Token::True => {
                self.advance();
                Ok(Term::Boolean(true))
            }
            Token::False => {
                self.advance();
                Ok(Term::Boolean(false))
            }
            Token::Null => {
                self.advance();
                Ok(Term::Null)
            }
            Token::Question => {
                self.advance();
                Ok(Term::PositionalBind)
            }
            Token::Colon => {
                self.advance();
                if let Token::Identifier(name) = &self.current_token {
                    let name = name.clone();
                    self.advance();
                    Ok(Term::NamedBind(Identifier(name)))
                } else {
                    Err(ParseError::ExpectedToken("identifier".into()))
                }
            }
            Token::LBracket => self.parse_list_literal(),
            Token::LBrace => self.parse_set_or_map_literal(),
            Token::LParen => self.parse_tuple_literal(),
            Token::UuidLiteral(u) => {
                let value = *u;
                self.advance();
                Ok(Term::Uuid(value))
            }
            Token::BlobLiteral(b) => {
                let value = b.clone();
                self.advance();
                Ok(Term::Blob(value))
            }
            Token::Identifier(_) | Token::QuotedIdentifier(_) => {
                // Could be a function call or column reference
                self.parse_function_or_identifier()
            }
            Token::Minus => {
                self.advance();
                let term = self.parse_term()?;
                Ok(Term::Negation(Box::new(term)))
            }
            Token::Cast => self.parse_cast(),
            _ => Err(ParseError::UnexpectedToken {
                expected: "term".into(),
                found: format!("{:?}", self.current_token),
            }),
        }
    }
    
    // ... many more parsing methods
}

#[derive(Debug, Clone)]
pub enum ParseError {
    UnexpectedToken { expected: String, found: String },
    ExpectedToken(String),
    UnterminatedString,
    InvalidNumber(String),
    InvalidUuid(String),
    SyntaxError(String),
}
```

---

## 5. CQL Native Protocol (Wire Protocol)

The CQL binary protocol is used for client-server communication over TCP. Current versions are v4 and v5.

### 5.1 Frame Structure

```text
 0         8        16        24        32
 +---------+---------+---------+---------+
 | version |  flags  |      stream       |
 +---------+---------+---------+---------+
 |  opcode |        length               |
 +---------+---------+---------+---------+
 |               body ...                |
 +---------------------------------------+
```

```rust
#[derive(Debug, Clone)]
pub struct Frame {
    pub version: ProtocolVersion,
    pub flags: FrameFlags,
    pub stream: i16,
    pub opcode: Opcode,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum ProtocolVersion {
    V3 = 0x03,
    V4 = 0x04,
    V5 = 0x05,
    // Response versions have high bit set
    ResponseV3 = 0x83,
    ResponseV4 = 0x84,
    ResponseV5 = 0x85,
}

bitflags! {
    pub struct FrameFlags: u8 {
        const COMPRESSION = 0x01;
        const TRACING = 0x02;
        const CUSTOM_PAYLOAD = 0x04;
        const WARNING = 0x08;
        const USE_BETA = 0x10;  // v5 only
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Opcode {
    // Requests
    Startup = 0x01,
    Options = 0x05,
    Query = 0x07,
    Prepare = 0x09,
    Execute = 0x0A,
    Register = 0x0B,
    Batch = 0x0D,
    AuthResponse = 0x0F,
    
    // Responses
    Error = 0x00,
    Ready = 0x02,
    Authenticate = 0x03,
    Supported = 0x06,
    Result = 0x08,
    Event = 0x0C,
    AuthChallenge = 0x0E,
    AuthSuccess = 0x10,
}
```

### 5.2 Data Type Encoding

```rust
/// Primitive type encodings in the wire protocol
pub trait CqlEncode {
    fn encode(&self, buf: &mut Vec<u8>);
}

pub trait CqlDecode: Sized {
    fn decode(buf: &mut &[u8]) -> Result<Self, DecodeError>;
}

// [int] - 4 byte signed integer (big-endian)
impl CqlEncode for i32 {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

impl CqlDecode for i32 {
    fn decode(buf: &mut &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < 4 {
            return Err(DecodeError::InsufficientData);
        }
        let bytes: [u8; 4] = buf[..4].try_into().unwrap();
        *buf = &buf[4..];
        Ok(i32::from_be_bytes(bytes))
    }
}

// [long] - 8 byte signed integer (big-endian)
impl CqlEncode for i64 {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

// [short] - 2 byte unsigned integer (big-endian)
impl CqlEncode for u16 {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.to_be_bytes());
    }
}

impl CqlDecode for u16 {
    fn decode(buf: &mut &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < 2 {
            return Err(DecodeError::InsufficientData);
        }
        let bytes: [u8; 2] = buf[..2].try_into().unwrap();
        *buf = &buf[2..];
        Ok(u16::from_be_bytes(bytes))
    }
}

// [string] - [short] n followed by n bytes UTF-8
#[derive(Debug, Clone)]
pub struct CqlString(pub String);

impl CqlEncode for CqlString {
    fn encode(&self, buf: &mut Vec<u8>) {
        let bytes = self.0.as_bytes();
        (bytes.len() as u16).encode(buf);
        buf.extend_from_slice(bytes);
    }
}

impl CqlDecode for CqlString {
    fn decode(buf: &mut &[u8]) -> Result<Self, DecodeError> {
        let len = u16::decode(buf)? as usize;
        if buf.len() < len {
            return Err(DecodeError::InsufficientData);
        }
        let s = std::str::from_utf8(&buf[..len])
            .map_err(|_| DecodeError::InvalidUtf8)?;
        *buf = &buf[len..];
        Ok(CqlString(s.to_string()))
    }
}

// [long string] - [int] n followed by n bytes UTF-8
#[derive(Debug, Clone)]
pub struct CqlLongString(pub String);

impl CqlEncode for CqlLongString {
    fn encode(&self, buf: &mut Vec<u8>) {
        let bytes = self.0.as_bytes();
        (bytes.len() as i32).encode(buf);
        buf.extend_from_slice(bytes);
    }
}

// [bytes] - [int] n followed by n bytes (n can be -1 for null)
#[derive(Debug, Clone)]
pub struct CqlBytes(pub Option<Vec<u8>>);

impl CqlEncode for CqlBytes {
    fn encode(&self, buf: &mut Vec<u8>) {
        match &self.0 {
            Some(bytes) => {
                (bytes.len() as i32).encode(buf);
                buf.extend_from_slice(bytes);
            }
            None => {
                (-1i32).encode(buf);
            }
        }
    }
}

impl CqlDecode for CqlBytes {
    fn decode(buf: &mut &[u8]) -> Result<Self, DecodeError> {
        let len = i32::decode(buf)?;
        if len < 0 {
            return Ok(CqlBytes(None));
        }
        let len = len as usize;
        if buf.len() < len {
            return Err(DecodeError::InsufficientData);
        }
        let bytes = buf[..len].to_vec();
        *buf = &buf[len..];
        Ok(CqlBytes(Some(bytes)))
    }
}

// [short bytes] - [short] n followed by n bytes
#[derive(Debug, Clone)]
pub struct CqlShortBytes(pub Vec<u8>);

// [uuid] - 16 bytes
impl CqlEncode for uuid::Uuid {
    fn encode(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.as_bytes());
    }
}

impl CqlDecode for uuid::Uuid {
    fn decode(buf: &mut &[u8]) -> Result<Self, DecodeError> {
        if buf.len() < 16 {
            return Err(DecodeError::InsufficientData);
        }
        let bytes: [u8; 16] = buf[..16].try_into().unwrap();
        *buf = &buf[16..];
        Ok(uuid::Uuid::from_bytes(bytes))
    }
}

// [string list] - [short] n followed by n [string]
#[derive(Debug, Clone)]
pub struct CqlStringList(pub Vec<String>);

impl CqlEncode for CqlStringList {
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.0.len() as u16).encode(buf);
        for s in &self.0 {
            CqlString(s.clone()).encode(buf);
        }
    }
}

// [string map] - [short] n followed by n pairs of [string]
#[derive(Debug, Clone)]
pub struct CqlStringMap(pub HashMap<String, String>);

impl CqlEncode for CqlStringMap {
    fn encode(&self, buf: &mut Vec<u8>) {
        (self.0.len() as u16).encode(buf);
        for (k, v) in &self.0 {
            CqlString(k.clone()).encode(buf);
            CqlString(v.clone()).encode(buf);
        }
    }
}

// [string multimap] - [short] n followed by n pairs of [string] and [string list]
#[derive(Debug, Clone)]
pub struct CqlStringMultiMap(pub HashMap<String, Vec<String>>);

// [inet] - [byte] n (4 or 16) followed by n bytes for IP, then [int] port
#[derive(Debug, Clone)]
pub struct CqlInet {
    pub addr: std::net::IpAddr,
    pub port: i32,
}

impl CqlEncode for CqlInet {
    fn encode(&self, buf: &mut Vec<u8>) {
        match self.addr {
            std::net::IpAddr::V4(ip) => {
                buf.push(4);
                buf.extend_from_slice(&ip.octets());
            }
            std::net::IpAddr::V6(ip) => {
                buf.push(16);
                buf.extend_from_slice(&ip.octets());
            }
        }
        self.port.encode(buf);
    }
}

// Consistency level encoding
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u16)]
pub enum Consistency {
    Any = 0x0000,
    One = 0x0001,
    Two = 0x0002,
    Three = 0x0003,
    Quorum = 0x0004,
    All = 0x0005,
    LocalQuorum = 0x0006,
    EachQuorum = 0x0007,
    Serial = 0x0008,
    LocalSerial = 0x0009,
    LocalOne = 0x000A,
}

impl CqlEncode for Consistency {
    fn encode(&self, buf: &mut Vec<u8>) {
        (*self as u16).encode(buf);
    }
}
```

### 5.3 Request Messages

```rust
// STARTUP message
#[derive(Debug, Clone)]
pub struct StartupMessage {
    pub options: HashMap<String, String>,
}

impl StartupMessage {
    pub fn new() -> Self {
        let mut options = HashMap::new();
        options.insert("CQL_VERSION".to_string(), "3.0.0".to_string());
        Self { options }
    }
    
    pub fn with_compression(mut self, compression: &str) -> Self {
        self.options.insert("COMPRESSION".to_string(), compression.to_string());
        self
    }
    
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        CqlStringMap(self.options.clone()).encode(&mut buf);
        buf
    }
}

// QUERY message
#[derive(Debug, Clone)]
pub struct QueryMessage {
    pub query: String,
    pub parameters: QueryParameters,
}

#[derive(Debug, Clone, Default)]
pub struct QueryParameters {
    pub consistency: Consistency,
    pub values: Option<Vec<CqlValue>>,
    pub skip_metadata: bool,
    pub page_size: Option<i32>,
    pub paging_state: Option<Vec<u8>>,
    pub serial_consistency: Option<Consistency>,
    pub timestamp: Option<i64>,
    pub keyspace: Option<String>,      // v5+
    pub now_in_seconds: Option<i32>,   // v5+
}

bitflags! {
    pub struct QueryFlags: u8 {
        const VALUES = 0x01;
        const SKIP_METADATA = 0x02;
        const PAGE_SIZE = 0x04;
        const PAGING_STATE = 0x08;
        const SERIAL_CONSISTENCY = 0x10;
        const TIMESTAMP = 0x20;
        const NAMES_FOR_VALUES = 0x40;
        const KEYSPACE = 0x80;         // v5+
    }
}

impl QueryMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // Query string
        CqlLongString(self.query.clone()).encode(&mut buf);
        
        // Consistency
        self.parameters.consistency.encode(&mut buf);
        
        // Flags
        let mut flags = QueryFlags::empty();
        if self.parameters.values.is_some() {
            flags |= QueryFlags::VALUES;
        }
        if self.parameters.skip_metadata {
            flags |= QueryFlags::SKIP_METADATA;
        }
        if self.parameters.page_size.is_some() {
            flags |= QueryFlags::PAGE_SIZE;
        }
        if self.parameters.paging_state.is_some() {
            flags |= QueryFlags::PAGING_STATE;
        }
        if self.parameters.serial_consistency.is_some() {
            flags |= QueryFlags::SERIAL_CONSISTENCY;
        }
        if self.parameters.timestamp.is_some() {
            flags |= QueryFlags::TIMESTAMP;
        }
        buf.push(flags.bits());
        
        // Values
        if let Some(values) = &self.parameters.values {
            (values.len() as u16).encode(&mut buf);
            for value in values {
                value.encode(&mut buf);
            }
        }
        
        // Page size
        if let Some(page_size) = self.parameters.page_size {
            page_size.encode(&mut buf);
        }
        
        // Paging state
        if let Some(paging_state) = &self.parameters.paging_state {
            CqlBytes(Some(paging_state.clone())).encode(&mut buf);
        }
        
        // Serial consistency
        if let Some(serial) = self.parameters.serial_consistency {
            serial.encode(&mut buf);
        }
        
        // Timestamp
        if let Some(ts) = self.parameters.timestamp {
            ts.encode(&mut buf);
        }
        
        buf
    }
}

// PREPARE message
#[derive(Debug, Clone)]
pub struct PrepareMessage {
    pub query: String,
    pub keyspace: Option<String>,  // v5+
}

impl PrepareMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        CqlLongString(self.query.clone()).encode(&mut buf);
        // v5: flags and keyspace would follow
        buf
    }
}

// EXECUTE message
#[derive(Debug, Clone)]
pub struct ExecuteMessage {
    pub prepared_id: Vec<u8>,
    pub result_metadata_id: Option<Vec<u8>>,  // v5+
    pub parameters: QueryParameters,
}

impl ExecuteMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // Prepared statement ID
        CqlShortBytes(self.prepared_id.clone()).encode(&mut buf);
        
        // v5: result metadata ID
        // if let Some(id) = &self.result_metadata_id {
        //     CqlShortBytes(id.clone()).encode(&mut buf);
        // }
        
        // Same as query parameters
        self.parameters.consistency.encode(&mut buf);
        // ... flags and values as in QueryMessage
        
        buf
    }
}

// BATCH message
#[derive(Debug, Clone)]
pub struct BatchMessage {
    pub batch_type: BatchType,
    pub queries: Vec<BatchQuery>,
    pub consistency: Consistency,
    pub serial_consistency: Option<Consistency>,
    pub timestamp: Option<i64>,
    pub keyspace: Option<String>,  // v5+
}

#[derive(Debug, Clone)]
pub enum BatchQuery {
    Simple {
        query: String,
        values: Vec<CqlValue>,
    },
    Prepared {
        prepared_id: Vec<u8>,
        values: Vec<CqlValue>,
    },
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum BatchType {
    Logged = 0,
    Unlogged = 1,
    Counter = 2,
}

impl BatchMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        
        // Batch type
        buf.push(self.batch_type as u8);
        
        // Number of queries
        (self.queries.len() as u16).encode(&mut buf);
        
        // Each query
        for query in &self.queries {
            match query {
                BatchQuery::Simple { query, values } => {
                    buf.push(0); // kind = 0 for query string
                    CqlLongString(query.clone()).encode(&mut buf);
                    (values.len() as u16).encode(&mut buf);
                    for value in values {
                        value.encode(&mut buf);
                    }
                }
                BatchQuery::Prepared { prepared_id, values } => {
                    buf.push(1); // kind = 1 for prepared ID
                    CqlShortBytes(prepared_id.clone()).encode(&mut buf);
                    (values.len() as u16).encode(&mut buf);
                    for value in values {
                        value.encode(&mut buf);
                    }
                }
            }
        }
        
        // Consistency
        self.consistency.encode(&mut buf);
        
        // Flags and optional fields
        let mut flags: u8 = 0;
        if self.serial_consistency.is_some() {
            flags |= 0x10;
        }
        if self.timestamp.is_some() {
            flags |= 0x20;
        }
        buf.push(flags);
        
        if let Some(serial) = self.serial_consistency {
            serial.encode(&mut buf);
        }
        if let Some(ts) = self.timestamp {
            ts.encode(&mut buf);
        }
        
        buf
    }
}

// REGISTER message (for event subscription)
#[derive(Debug, Clone)]
pub struct RegisterMessage {
    pub event_types: Vec<String>,
}

impl RegisterMessage {
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        CqlStringList(self.event_types.clone()).encode(&mut buf);
        buf
    }
}
```

### 5.4 Response Messages

```rust
// ERROR response
#[derive(Debug, Clone)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    pub message: String,
    pub additional: ErrorAdditional,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(i32)]
pub enum ErrorCode {
    ServerError = 0x0000,
    ProtocolError = 0x000A,
    AuthenticationError = 0x0100,
    Unavailable = 0x1000,
    Overloaded = 0x1001,
    IsBootstrapping = 0x1002,
    TruncateError = 0x1003,
    WriteTimeout = 0x1100,
    ReadTimeout = 0x1200,
    ReadFailure = 0x1300,
    FunctionFailure = 0x1400,
    WriteFailure = 0x1500,
    SyntaxError = 0x2000,
    Unauthorized = 0x2100,
    Invalid = 0x2200,
    ConfigError = 0x2300,
    AlreadyExists = 0x2400,
    Unprepared = 0x2500,
}

#[derive(Debug, Clone)]
pub enum ErrorAdditional {
    None,
    Unavailable {
        consistency: Consistency,
        required: i32,
        alive: i32,
    },
    WriteTimeout {
        consistency: Consistency,
        received: i32,
        block_for: i32,
        write_type: String,
    },
    ReadTimeout {
        consistency: Consistency,
        received: i32,
        block_for: i32,
        data_present: bool,
    },
    ReadFailure {
        consistency: Consistency,
        received: i32,
        block_for: i32,
        num_failures: i32,
        data_present: bool,
    },
    WriteFailure {
        consistency: Consistency,
        received: i32,
        block_for: i32,
        num_failures: i32,
        write_type: String,
    },
    AlreadyExists {
        keyspace: String,
        table: String,
    },
    Unprepared {
        statement_id: Vec<u8>,
    },
    FunctionFailure {
        keyspace: String,
        function: String,
        arg_types: Vec<String>,
    },
}

// RESULT response
#[derive(Debug, Clone)]
pub enum ResultResponse {
    Void,
    Rows(RowsResult),
    SetKeyspace(String),
    Prepared(PreparedResult),
    SchemaChange(SchemaChangeEvent),
}

#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum ResultKind {
    Void = 0x0001,
    Rows = 0x0002,
    SetKeyspace = 0x0003,
    Prepared = 0x0004,
    SchemaChange = 0x0005,
}

#[derive(Debug, Clone)]
pub struct RowsResult {
    pub metadata: RowsMetadata,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone)]
pub struct RowsMetadata {
    pub flags: RowsFlags,
    pub columns_count: i32,
    pub paging_state: Option<Vec<u8>>,
    pub new_metadata_id: Option<Vec<u8>>,  // v5+
    pub global_table_spec: Option<TableSpec>,
    pub column_specs: Vec<ColumnSpec>,
}

bitflags! {
    pub struct RowsFlags: i32 {
        const GLOBAL_TABLES_SPEC = 0x0001;
        const HAS_MORE_PAGES = 0x0002;
        const NO_METADATA = 0x0004;
        const METADATA_CHANGED = 0x0008;  // v5+
    }
}

#[derive(Debug, Clone)]
pub struct TableSpec {
    pub keyspace: String,
    pub table: String,
}

#[derive(Debug, Clone)]
pub struct ColumnSpec {
    pub table_spec: Option<TableSpec>,
    pub name: String,
    pub data_type: DataTypeSpec,
}

#[derive(Debug, Clone)]
pub enum DataTypeSpec {
    Custom(String),
    Ascii,
    Bigint,
    Blob,
    Boolean,
    Counter,
    Decimal,
    Double,
    Float,
    Int,
    Timestamp,
    Uuid,
    Varchar,
    Varint,
    Timeuuid,
    Inet,
    Date,
    Time,
    Smallint,
    Tinyint,
    Duration,
    List(Box<DataTypeSpec>),
    Map(Box<DataTypeSpec>, Box<DataTypeSpec>),
    Set(Box<DataTypeSpec>),
    Udt {
        keyspace: String,
        name: String,
        fields: Vec<(String, DataTypeSpec)>,
    },
    Tuple(Vec<DataTypeSpec>),
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum DataTypeId {
    Custom = 0x0000,
    Ascii = 0x0001,
    Bigint = 0x0002,
    Blob = 0x0003,
    Boolean = 0x0004,
    Counter = 0x0005,
    Decimal = 0x0006,
    Double = 0x0007,
    Float = 0x0008,
    Int = 0x0009,
    Timestamp = 0x000B,
    Uuid = 0x000C,
    Varchar = 0x000D,
    Varint = 0x000E,
    Timeuuid = 0x000F,
    Inet = 0x0010,
    Date = 0x0011,
    Time = 0x0012,
    Smallint = 0x0013,
    Tinyint = 0x0014,
    Duration = 0x0015,
    List = 0x0020,
    Map = 0x0021,
    Set = 0x0022,
    Udt = 0x0030,
    Tuple = 0x0031,
}

#[derive(Debug, Clone)]
pub struct Row {
    pub columns: Vec<Option<CqlValue>>,
}

#[derive(Debug, Clone)]
pub struct PreparedResult {
    pub id: Vec<u8>,
    pub result_metadata_id: Option<Vec<u8>>,  // v5+
    pub prepared_metadata: PreparedMetadata,
    pub result_metadata: RowsMetadata,
}

#[derive(Debug, Clone)]
pub struct PreparedMetadata {
    pub flags: PreparedFlags,
    pub columns_count: i32,
    pub pk_count: i32,
    pub pk_indexes: Vec<u16>,
    pub global_table_spec: Option<TableSpec>,
    pub column_specs: Vec<ColumnSpec>,
}

bitflags! {
    pub struct PreparedFlags: i32 {
        const GLOBAL_TABLES_SPEC = 0x0001;
    }
}

// Schema change event
#[derive(Debug, Clone)]
pub struct SchemaChangeEvent {
    pub change_type: SchemaChangeType,
    pub target: SchemaChangeTarget,
}

#[derive(Debug, Clone, Copy)]
pub enum SchemaChangeType {
    Created,
    Updated,
    Dropped,
}

#[derive(Debug, Clone)]
pub enum SchemaChangeTarget {
    Keyspace(String),
    Table { keyspace: String, table: String },
    Type { keyspace: String, name: String },
    Function { keyspace: String, name: String, arg_types: Vec<String> },
    Aggregate { keyspace: String, name: String, arg_types: Vec<String> },
}

// EVENT response
#[derive(Debug, Clone)]
pub enum EventResponse {
    TopologyChange {
        change_type: TopologyChangeType,
        address: CqlInet,
    },
    StatusChange {
        change_type: StatusChangeType,
        address: CqlInet,
    },
    SchemaChange(SchemaChangeEvent),
}

#[derive(Debug, Clone, Copy)]
pub enum TopologyChangeType {
    NewNode,
    RemovedNode,
}

#[derive(Debug, Clone, Copy)]
pub enum StatusChangeType {
    Up,
    Down,
}
```

### 5.5 CQL Values (Runtime Values)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum CqlValue {
    Ascii(String),
    Bigint(i64),
    Blob(Vec<u8>),
    Boolean(bool),
    Counter(i64),
    Date(u32),      // days since epoch (unsigned)
    Decimal(BigDecimal),
    Double(f64),
    Duration(CqlDuration),
    Float(f32),
    Inet(std::net::IpAddr),
    Int(i32),
    Smallint(i16),
    Text(String),
    Time(i64),      // nanoseconds since midnight
    Timestamp(i64), // milliseconds since epoch
    Timeuuid(uuid::Uuid),
    Tinyint(i8),
    Uuid(uuid::Uuid),
    Varint(BigInt),
    
    // Collections
    List(Vec<CqlValue>),
    Set(Vec<CqlValue>),
    Map(Vec<(CqlValue, CqlValue)>),
    
    // User-defined type
    Udt(HashMap<String, CqlValue>),
    
    // Tuple
    Tuple(Vec<Option<CqlValue>>),
    
    // Special
    Null,
    Unset,  // For bound values only
}

#[derive(Debug, Clone, PartialEq)]
pub struct CqlDuration {
    pub months: i32,
    pub days: i32,
    pub nanoseconds: i64,
}

impl CqlValue {
    pub fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            CqlValue::Null => {
                (-1i32).encode(buf);
            }
            CqlValue::Unset => {
                (-2i32).encode(buf);
            }
            CqlValue::Ascii(s) | CqlValue::Text(s) => {
                let bytes = s.as_bytes();
                (bytes.len() as i32).encode(buf);
                buf.extend_from_slice(bytes);
            }
            CqlValue::Bigint(v) | CqlValue::Counter(v) | 
            CqlValue::Time(v) | CqlValue::Timestamp(v) => {
                (8i32).encode(buf);
                v.encode(buf);
            }
            CqlValue::Int(v) => {
                (4i32).encode(buf);
                v.encode(buf);
            }
            CqlValue::Smallint(v) => {
                (2i32).encode(buf);
                buf.extend_from_slice(&v.to_be_bytes());
            }
            CqlValue::Tinyint(v) => {
                (1i32).encode(buf);
                buf.push(*v as u8);
            }
            CqlValue::Boolean(v) => {
                (1i32).encode(buf);
                buf.push(if *v { 1 } else { 0 });
            }
            CqlValue::Float(v) => {
                (4i32).encode(buf);
                buf.extend_from_slice(&v.to_be_bytes());
            }
            CqlValue::Double(v) => {
                (8i32).encode(buf);
                buf.extend_from_slice(&v.to_be_bytes());
            }
            CqlValue::Uuid(v) | CqlValue::Timeuuid(v) => {
                (16i32).encode(buf);
                v.encode(buf);
            }
            CqlValue::Blob(bytes) => {
                (bytes.len() as i32).encode(buf);
                buf.extend_from_slice(bytes);
            }
            CqlValue::Inet(addr) => {
                match addr {
                    std::net::IpAddr::V4(ip) => {
                        (4i32).encode(buf);
                        buf.extend_from_slice(&ip.octets());
                    }
                    std::net::IpAddr::V6(ip) => {
                        (16i32).encode(buf);
                        buf.extend_from_slice(&ip.octets());
                    }
                }
            }
            CqlValue::Date(days) => {
                (4i32).encode(buf);
                buf.extend_from_slice(&days.to_be_bytes());
            }
            CqlValue::List(items) | CqlValue::Set(items) => {
                let mut inner = Vec::new();
                (items.len() as i32).encode(&mut inner);
                for item in items {
                    item.encode(&mut inner);
                }
                (inner.len() as i32).encode(buf);
                buf.extend(inner);
            }
            CqlValue::Map(pairs) => {
                let mut inner = Vec::new();
                (pairs.len() as i32).encode(&mut inner);
                for (k, v) in pairs {
                    k.encode(&mut inner);
                    v.encode(&mut inner);
                }
                (inner.len() as i32).encode(buf);
                buf.extend(inner);
            }
            CqlValue::Tuple(elements) => {
                let mut inner = Vec::new();
                for elem in elements {
                    match elem {
                        Some(v) => v.encode(&mut inner),
                        None => (-1i32).encode(&mut inner),
                    }
                }
                (inner.len() as i32).encode(buf);
                buf.extend(inner);
            }
            CqlValue::Udt(fields) => {
                // UDT encoding requires field order from schema
                // This is a simplified version
                let mut inner = Vec::new();
                for (_, v) in fields {
                    v.encode(&mut inner);
                }
                (inner.len() as i32).encode(buf);
                buf.extend(inner);
            }
            CqlValue::Decimal(_) | CqlValue::Varint(_) | 
            CqlValue::Duration(_) => {
                // These require special encoding
                todo!("Implement decimal/varint/duration encoding")
            }
        }
    }
    
    pub fn decode(
        buf: &mut &[u8], 
        type_spec: &DataTypeSpec
    ) -> Result<Option<Self>, DecodeError> {
        let len = i32::decode(buf)?;
        if len < 0 {
            return Ok(None);
        }
        let len = len as usize;
        if buf.len() < len {
            return Err(DecodeError::InsufficientData);
        }
        
        let data = &buf[..len];
        *buf = &buf[len..];
        
        let value = match type_spec {
            DataTypeSpec::Ascii | DataTypeSpec::Varchar => {
                let s = std::str::from_utf8(data)
                    .map_err(|_| DecodeError::InvalidUtf8)?;
                CqlValue::Text(s.to_string())
            }
            DataTypeSpec::Bigint => {
                let bytes: [u8; 8] = data.try_into()
                    .map_err(|_| DecodeError::InvalidData)?;
                CqlValue::Bigint(i64::from_be_bytes(bytes))
            }
            DataTypeSpec::Int => {
                let bytes: [u8; 4] = data.try_into()
                    .map_err(|_| DecodeError::InvalidData)?;
                CqlValue::Int(i32::from_be_bytes(bytes))
            }
            DataTypeSpec::Boolean => {
                CqlValue::Boolean(data[0] != 0)
            }
            DataTypeSpec::Blob => {
                CqlValue::Blob(data.to_vec())
            }
            DataTypeSpec::Uuid | DataTypeSpec::Timeuuid => {
                let bytes: [u8; 16] = data.try_into()
                    .map_err(|_| DecodeError::InvalidData)?;
                CqlValue::Uuid(uuid::Uuid::from_bytes(bytes))
            }
            DataTypeSpec::List(inner) => {
                let mut data = data;
                let count = i32::decode(&mut data)? as usize;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    if let Some(v) = CqlValue::decode(&mut data, inner)? {
                        items.push(v);
                    }
                }
                CqlValue::List(items)
            }
            DataTypeSpec::Map(key_type, val_type) => {
                let mut data = data;
                let count = i32::decode(&mut data)? as usize;
                let mut pairs = Vec::with_capacity(count);
                for _ in 0..count {
                    let k = CqlValue::decode(&mut data, key_type)?
                        .ok_or(DecodeError::InvalidData)?;
                    let v = CqlValue::decode(&mut data, val_type)?
                        .ok_or(DecodeError::InvalidData)?;
                    pairs.push((k, v));
                }
                CqlValue::Map(pairs)
            }
            // ... other types
            _ => {
                CqlValue::Blob(data.to_vec()) // Fallback
            }
        };
        
        Ok(Some(value))
    }
}
```

### 5.6 Frame Encoding/Decoding

```rust
pub struct FrameCodec {
    version: ProtocolVersion,
}

impl FrameCodec {
    pub fn new(version: ProtocolVersion) -> Self {
        Self { version }
    }
    
    pub fn encode_frame(&self, frame: &Frame) -> Vec<u8> {
        let mut buf = Vec::with_capacity(9 + frame.body.len());
        
        // Header
        buf.push(frame.version as u8);
        buf.push(frame.flags.bits());
        buf.extend_from_slice(&frame.stream.to_be_bytes());
        buf.push(frame.opcode as u8);
        (frame.body.len() as i32).encode(&mut buf);
        
        // Body
        buf.extend_from_slice(&frame.body);
        
        buf
    }
    
    pub fn decode_frame(&self, buf: &[u8]) -> Result<(Frame, usize), DecodeError> {
        if buf.len() < 9 {
            return Err(DecodeError::InsufficientData);
        }
        
        let version = buf[0];
        let flags = FrameFlags::from_bits_truncate(buf[1]);
        let stream = i16::from_be_bytes([buf[2], buf[3]]);
        let opcode = buf[4];
        let length = i32::from_be_bytes([buf[5], buf[6], buf[7], buf[8]]) as usize;
        
        if buf.len() < 9 + length {
            return Err(DecodeError::InsufficientData);
        }
        
        let body = buf[9..9 + length].to_vec();
        
        let frame = Frame {
            version: unsafe { std::mem::transmute(version) },
            flags,
            stream,
            opcode: unsafe { std::mem::transmute(opcode) },
            body,
        };
        
        Ok((frame, 9 + length))
    }
    
    pub fn decode_response(&self, frame: &Frame) -> Result<Response, DecodeError> {
        let mut body = frame.body.as_slice();
        
        match frame.opcode {
            Opcode::Error => {
                let code = i32::decode(&mut body)?;
                let message = CqlString::decode(&mut body)?.0;
                let additional = self.decode_error_additional(
                    ErrorCode::try_from(code).unwrap_or(ErrorCode::ServerError),
                    &mut body
                )?;
                Ok(Response::Error(ErrorResponse { 
                    code: ErrorCode::try_from(code).unwrap_or(ErrorCode::ServerError),
                    message, 
                    additional 
                }))
            }
            Opcode::Ready => Ok(Response::Ready),
            Opcode::Result => self.decode_result(&mut body),
            Opcode::Authenticate => {
                let authenticator = CqlString::decode(&mut body)?.0;
                Ok(Response::Authenticate(authenticator))
            }
            Opcode::Supported => {
                let options = CqlStringMultiMap::decode(&mut body)?.0;
                Ok(Response::Supported(options))
            }
            Opcode::Event => self.decode_event(&mut body),
            _ => Err(DecodeError::UnknownOpcode(frame.opcode as u8)),
        }
    }
    
    fn decode_result(&self, buf: &mut &[u8]) -> Result<Response, DecodeError> {
        let kind = i32::decode(buf)?;
        
        match kind {
            0x0001 => Ok(Response::Result(ResultResponse::Void)),
            0x0002 => {
                let metadata = self.decode_rows_metadata(buf)?;
                let rows_count = i32::decode(buf)? as usize;
                let mut rows = Vec::with_capacity(rows_count);
                
                for _ in 0..rows_count {
                    let mut columns = Vec::with_capacity(metadata.columns_count as usize);
                    for col_spec in &metadata.column_specs {
                        let value = CqlValue::decode(buf, &col_spec.data_type)?;
                        columns.push(value);
                    }
                    rows.push(Row { columns });
                }
                
                Ok(Response::Result(ResultResponse::Rows(RowsResult { 
                    metadata, 
                    rows 
                })))
            }
            0x0003 => {
                let keyspace = CqlString::decode(buf)?.0;
                Ok(Response::Result(ResultResponse::SetKeyspace(keyspace)))
            }
            0x0004 => {
                let id = CqlShortBytes::decode(buf)?.0;
                let prepared_metadata = self.decode_prepared_metadata(buf)?;
                let result_metadata = self.decode_rows_metadata(buf)?;
                Ok(Response::Result(ResultResponse::Prepared(PreparedResult {
                    id,
                    result_metadata_id: None,
                    prepared_metadata,
                    result_metadata,
                })))
            }
            0x0005 => {
                let event = self.decode_schema_change(buf)?;
                Ok(Response::Result(ResultResponse::SchemaChange(event)))
            }
            _ => Err(DecodeError::UnknownResultKind(kind)),
        }
    }
    
    fn decode_rows_metadata(&self, buf: &mut &[u8]) -> Result<RowsMetadata, DecodeError> {
        let flags = RowsFlags::from_bits_truncate(i32::decode(buf)?);
        let columns_count = i32::decode(buf)?;
        
        let paging_state = if flags.contains(RowsFlags::HAS_MORE_PAGES) {
            Some(CqlBytes::decode(buf)?.0.unwrap_or_default())
        } else {
            None
        };
        
        // Skip metadata if NO_METADATA flag
        if flags.contains(RowsFlags::NO_METADATA) {
            return Ok(RowsMetadata {
                flags,
                columns_count,
                paging_state,
                new_metadata_id: None,
                global_table_spec: None,
                column_specs: vec![],
            });
        }
        
        let global_table_spec = if flags.contains(RowsFlags::GLOBAL_TABLES_SPEC) {
            Some(TableSpec {
                keyspace: CqlString::decode(buf)?.0,
                table: CqlString::decode(buf)?.0,
            })
        } else {
            None
        };
        
        let mut column_specs = Vec::with_capacity(columns_count as usize);
        for _ in 0..columns_count {
            let table_spec = if global_table_spec.is_none() {
                Some(TableSpec {
                    keyspace: CqlString::decode(buf)?.0,
                    table: CqlString::decode(buf)?.0,
                })
            } else {
                None
            };
            let name = CqlString::decode(buf)?.0;
            let data_type = self.decode_type_spec(buf)?;
            column_specs.push(ColumnSpec { table_spec, name, data_type });
        }
        
        Ok(RowsMetadata {
            flags,
            columns_count,
            paging_state,
            new_metadata_id: None,
            global_table_spec,
            column_specs,
        })
    }
    
    fn decode_type_spec(&self, buf: &mut &[u8]) -> Result<DataTypeSpec, DecodeError> {
        let id = u16::decode(buf)?;
        
        match id {
            0x0000 => Ok(DataTypeSpec::Custom(CqlString::decode(buf)?.0)),
            0x0001 => Ok(DataTypeSpec::Ascii),
            0x0002 => Ok(DataTypeSpec::Bigint),
            0x0003 => Ok(DataTypeSpec::Blob),
            0x0004 => Ok(DataTypeSpec::Boolean),
            0x0005 => Ok(DataTypeSpec::Counter),
            0x0006 => Ok(DataTypeSpec::Decimal),
            0x0007 => Ok(DataTypeSpec::Double),
            0x0008 => Ok(DataTypeSpec::Float),
            0x0009 => Ok(DataTypeSpec::Int),
            0x000B => Ok(DataTypeSpec::Timestamp),
            0x000C => Ok(DataTypeSpec::Uuid),
            0x000D => Ok(DataTypeSpec::Varchar),
            0x000E => Ok(DataTypeSpec::Varint),
            0x000F => Ok(DataTypeSpec::Timeuuid),
            0x0010 => Ok(DataTypeSpec::Inet),
            0x0011 => Ok(DataTypeSpec::Date),
            0x0012 => Ok(DataTypeSpec::Time),
            0x0013 => Ok(DataTypeSpec::Smallint),
            0x0014 => Ok(DataTypeSpec::Tinyint),
            0x0015 => Ok(DataTypeSpec::Duration),
            0x0020 => {
                let inner = self.decode_type_spec(buf)?;
                Ok(DataTypeSpec::List(Box::new(inner)))
            }
            0x0021 => {
                let key = self.decode_type_spec(buf)?;
                let value = self.decode_type_spec(buf)?;
                Ok(DataTypeSpec::Map(Box::new(key), Box::new(value)))
            }
            0x0022 => {
                let inner = self.decode_type_spec(buf)?;
                Ok(DataTypeSpec::Set(Box::new(inner)))
            }
            0x0030 => {
                let keyspace = CqlString::decode(buf)?.0;
                let name = CqlString::decode(buf)?.0;
                let field_count = u16::decode(buf)? as usize;
                let mut fields = Vec::with_capacity(field_count);
                for _ in 0..field_count {
                    let field_name = CqlString::decode(buf)?.0;
                    let field_type = self.decode_type_spec(buf)?;
                    fields.push((field_name, field_type));
                }
                Ok(DataTypeSpec::Udt { keyspace, name, fields })
            }
            0x0031 => {
                let count = u16::decode(buf)? as usize;
                let mut types = Vec::with_capacity(count);
                for _ in 0..count {
                    types.push(self.decode_type_spec(buf)?);
                }
                Ok(DataTypeSpec::Tuple(types))
            }
            _ => Err(DecodeError::UnknownTypeId(id)),
        }
    }
    
    // ... other decode methods
}

#[derive(Debug)]
pub enum DecodeError {
    InsufficientData,
    InvalidUtf8,
    InvalidData,
    UnknownOpcode(u8),
    UnknownResultKind(i32),
    UnknownTypeId(u16),
}

pub enum Response {
    Error(ErrorResponse),
    Ready,
    Authenticate(String),
    Supported(HashMap<String, Vec<String>>),
    Result(ResultResponse),
    Event(EventResponse),
}
```

---

## 6. Connection Management

### 6.1 Connection State Machine

```rust
pub enum ConnectionState {
    Disconnected,
    Connecting,
    WaitingForStartup,
    Authenticating,
    Ready,
    Closing,
    Closed,
}

pub struct Connection {
    state: ConnectionState,
    stream: Option<TcpStream>,
    version: ProtocolVersion,
    codec: FrameCodec,
    stream_ids: StreamIdAllocator,
    pending_requests: HashMap<i16, oneshot::Sender<Response>>,
    compression: Option<Compression>,
}

pub struct StreamIdAllocator {
    next_id: AtomicI16,
    max_id: i16,
}

impl StreamIdAllocator {
    pub fn new(version: ProtocolVersion) -> Self {
        let max_id = match version {
            ProtocolVersion::V3 => 127,
            _ => 32767,
        };
        Self {
            next_id: AtomicI16::new(0),
            max_id,
        }
    }
    
    pub fn allocate(&self) -> Option<i16> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if id > self.max_id {
            self.next_id.store(0, Ordering::SeqCst);
            None
        } else {
            Some(id)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Compression {
    Lz4,
    Snappy,
}

impl Connection {
    pub async fn connect(addr: &str) -> Result<Self, ConnectionError> {
        let stream = TcpStream::connect(addr).await?;
        
        let mut conn = Self {
            state: ConnectionState::Connecting,
            stream: Some(stream),
            version: ProtocolVersion::V4,
            codec: FrameCodec::new(ProtocolVersion::V4),
            stream_ids: StreamIdAllocator::new(ProtocolVersion::V4),
            pending_requests: HashMap::new(),
            compression: None,
        };
        
        conn.startup().await?;
        Ok(conn)
    }
    
    async fn startup(&mut self) -> Result<(), ConnectionError> {
        let startup = StartupMessage::new();
        let frame = Frame {
            version: self.version,
            flags: FrameFlags::empty(),
            stream: 0,
            opcode: Opcode::Startup,
            body: startup.encode(),
        };
        
        self.send_frame(&frame).await?;
        let response = self.receive_frame().await?;
        
        match self.codec.decode_response(&response)? {
            Response::Ready => {
                self.state = ConnectionState::Ready;
                Ok(())
            }
            Response::Authenticate(authenticator) => {
                self.state = ConnectionState::Authenticating;
                self.authenticate(&authenticator).await
            }
            Response::Error(err) => {
                Err(ConnectionError::ServerError(err))
            }
            _ => Err(ConnectionError::UnexpectedResponse),
        }
    }
    
    pub async fn query(
        &mut self,
        query: &str,
        values: Vec<CqlValue>,
        consistency: Consistency,
    ) -> Result<ResultResponse, QueryError> {
        let stream_id = self.stream_ids.allocate()
            .ok_or(QueryError::NoStreamAvailable)?;
        
        let msg = QueryMessage {
            query: query.to_string(),
            parameters: QueryParameters {
                consistency,
                values: if values.is_empty() { None } else { Some(values) },
                ..Default::default()
            },
        };
        
        let frame = Frame {
            version: self.version,
            flags: FrameFlags::empty(),
            stream: stream_id,
            opcode: Opcode::Query,
            body: msg.encode(),
        };
        
        self.send_frame(&frame).await?;
        let response = self.receive_frame().await?;
        
        match self.codec.decode_response(&response)? {
            Response::Result(result) => Ok(result),
            Response::Error(err) => Err(QueryError::ServerError(err)),
            _ => Err(QueryError::UnexpectedResponse),
        }
    }
    
    pub async fn prepare(&mut self, query: &str) -> Result<PreparedStatement, QueryError> {
        let stream_id = self.stream_ids.allocate()
            .ok_or(QueryError::NoStreamAvailable)?;
        
        let msg = PrepareMessage {
            query: query.to_string(),
            keyspace: None,
        };
        
        let frame = Frame {
            version: self.version,
            flags: FrameFlags::empty(),
            stream: stream_id,
            opcode: Opcode::Prepare,
            body: msg.encode(),
        };
        
        self.send_frame(&frame).await?;
        let response = self.receive_frame().await?;
        
        match self.codec.decode_response(&response)? {
            Response::Result(ResultResponse::Prepared(prepared)) => {
                Ok(PreparedStatement {
                    id: prepared.id,
                    query: query.to_string(),
                    metadata: prepared.prepared_metadata,
                    result_metadata: prepared.result_metadata,
                })
            }
            Response::Error(err) => Err(QueryError::ServerError(err)),
            _ => Err(QueryError::UnexpectedResponse),
        }
    }
    
    pub async fn execute(
        &mut self,
        prepared: &PreparedStatement,
        values: Vec<CqlValue>,
        consistency: Consistency,
    ) -> Result<ResultResponse, QueryError> {
        let stream_id = self.stream_ids.allocate()
            .ok_or(QueryError::NoStreamAvailable)?;
        
        let msg = ExecuteMessage {
            prepared_id: prepared.id.clone(),
            result_metadata_id: None,
            parameters: QueryParameters {
                consistency,
                values: if values.is_empty() { None } else { Some(values) },
                skip_metadata: true, // We have metadata from prepare
                ..Default::default()
            },
        };
        
        let frame = Frame {
            version: self.version,
            flags: FrameFlags::empty(),
            stream: stream_id,
            opcode: Opcode::Execute,
            body: msg.encode(),
        };
        
        self.send_frame(&frame).await?;
        let response = self.receive_frame().await?;
        
        match self.codec.decode_response(&response)? {
            Response::Result(result) => Ok(result),
            Response::Error(err) => {
                // Handle UNPREPARED error - need to re-prepare
                if err.code == ErrorCode::Unprepared {
                    return Err(QueryError::Unprepared);
                }
                Err(QueryError::ServerError(err))
            }
            _ => Err(QueryError::UnexpectedResponse),
        }
    }
    
    async fn send_frame(&mut self, frame: &Frame) -> Result<(), ConnectionError> {
        let bytes = self.codec.encode_frame(frame);
        if let Some(stream) = &mut self.stream {
            stream.write_all(&bytes).await?;
            Ok(())
        } else {
            Err(ConnectionError::NotConnected)
        }
    }
    
    async fn receive_frame(&mut self) -> Result<Frame, ConnectionError> {
        if let Some(stream) = &mut self.stream {
            let mut header = [0u8; 9];
            stream.read_exact(&mut header).await?;
            
            let length = i32::from_be_bytes([
                header[5], header[6], header[7], header[8]
            ]) as usize;
            
            let mut body = vec![0u8; length];
            stream.read_exact(&mut body).await?;
            
            let mut full = header.to_vec();
            full.extend(body);
            
            let (frame, _) = self.codec.decode_frame(&full)?;
            Ok(frame)
        } else {
            Err(ConnectionError::NotConnected)
        }
    }
}

pub struct PreparedStatement {
    pub id: Vec<u8>,
    pub query: String,
    pub metadata: PreparedMetadata,
    pub result_metadata: RowsMetadata,
}
```

---

## 7. Grammar Reference (BNF)

```bnf
<statement> ::= <select_statement>
              | <insert_statement>
              | <update_statement>
              | <delete_statement>
              | <batch_statement>
              | <create_keyspace_statement>
              | <create_table_statement>
              | <create_index_statement>
              | <drop_statement>
              | <alter_statement>
              | <grant_statement>
              | <revoke_statement>
              | <use_statement>
              | <truncate_statement>

<select_statement> ::= 'SELECT' ['DISTINCT'] ['JSON'] <selectors>
                       'FROM' <table_name>
                       ['WHERE' <where_clause>]
                       ['GROUP' 'BY' <column_list>]
                       ['ORDER' 'BY' <ordering_clause>]
                       ['PER' 'PARTITION' 'LIMIT' <integer>]
                       ['LIMIT' <integer>]
                       ['ALLOW' 'FILTERING']

<selectors> ::= '*'
              | <selector> (',' <selector>)*

<selector> ::= <column_name> ['AS' <identifier>]
             | <function_call> ['AS' <identifier>]
             | 'COUNT' '(' '*' ')'
             | 'WRITETIME' '(' <column_name> ')'
             | 'TTL' '(' <column_name> ')'
             | 'CAST' '(' <selector> 'AS' <cql_type> ')'

<where_clause> ::= <relation> ('AND' <relation>)*

<relation> ::= <column_name> <relation_op> <term>
             | <column_name> 'IN' '(' <term_list> ')'
             | <column_name> 'IN' '(' '?' ')'
             | <column_name> 'CONTAINS' ['KEY'] <term>
             | <column_name> 'LIKE' <term>
             | <column_name> 'IS' 'NOT' 'NULL'
             | 'TOKEN' '(' <column_list> ')' <relation_op> <term>
             | '(' <column_list> ')' <relation_op> '(' <term_list> ')'

<relation_op> ::= '=' | '!=' | '<>' | '<' | '<=' | '>' | '>='

<insert_statement> ::= 'INSERT' 'INTO' <table_name>
                       ('(' <column_list> ')' 'VALUES' '(' <term_list> ')'
                        | 'JSON' <string_literal> ['DEFAULT' ('NULL' | 'UNSET')])
                       ['IF' 'NOT' 'EXISTS']
                       ['USING' <update_parameter> ('AND' <update_parameter>)*]

<update_statement> ::= 'UPDATE' <table_name>
                       ['USING' <update_parameter> ('AND' <update_parameter>)*]
                       'SET' <assignment> (',' <assignment>)*
                       'WHERE' <where_clause>
                       ['IF' <conditions>]

<assignment> ::= <column_name> '=' <term>
              | <column_name> '=' <column_name> '+' <term>
              | <column_name> '=' <column_name> '-' <term>
              | <column_name> '=' <term> '+' <column_name>
              | <column_name> '[' <term> ']' '=' <term>
              | <column_name> '.' <identifier> '=' <term>

<delete_statement> ::= 'DELETE' [<delete_selections>]
                       'FROM' <table_name>
                       ['USING' <update_parameter> ('AND' <update_parameter>)*]
                       'WHERE' <where_clause>
                       ['IF' <conditions>]

<delete_selections> ::= <delete_selection> (',' <delete_selection>)*

<delete_selection> ::= <column_name>
                     | <column_name> '[' <term> ']'
                     | <column_name> '.' <identifier>

<batch_statement> ::= 'BEGIN' ['UNLOGGED' | 'COUNTER'] 'BATCH'
                      ['USING' <update_parameter> ('AND' <update_parameter>)*]
                      (<insert_statement> | <update_statement> | <delete_statement>) ';'*
                      'APPLY' 'BATCH'

<update_parameter> ::= 'TIMESTAMP' <integer>
                     | 'TTL' <integer>

<create_keyspace_statement> ::= 'CREATE' 'KEYSPACE' ['IF' 'NOT' 'EXISTS'] <identifier>
                                'WITH' <keyspace_options>

<keyspace_options> ::= 'REPLICATION' '=' <map_literal>
                       ['AND' 'DURABLE_WRITES' '=' <boolean>]

<create_table_statement> ::= 'CREATE' ('TABLE' | 'COLUMNFAMILY')
                             ['IF' 'NOT' 'EXISTS'] <table_name>
                             '(' <column_definition> (',' <column_definition>)*
                                 [',' 'PRIMARY' 'KEY' '(' <primary_key> ')']
                             ')'
                             ['WITH' <table_options>]

<column_definition> ::= <identifier> <cql_type> ['STATIC'] ['PRIMARY' 'KEY']

<primary_key> ::= <partition_key> [',' <column_name>]*

<partition_key> ::= <column_name>
                  | '(' <column_name> (',' <column_name>)* ')'

<table_options> ::= <table_option> ('AND' <table_option>)*

<table_option> ::= <identifier> '=' <term>
                 | 'CLUSTERING' 'ORDER' 'BY' '(' <ordering_clause> ')'
                 | 'COMPACT' 'STORAGE'

<cql_type> ::= <native_type>
             | <collection_type>
             | <tuple_type>
             | 'FROZEN' '<' <cql_type> '>'
             | <user_defined_type>

<native_type> ::= 'ASCII' | 'BIGINT' | 'BLOB' | 'BOOLEAN' | 'COUNTER'
               | 'DATE' | 'DECIMAL' | 'DOUBLE' | 'DURATION' | 'FLOAT'
               | 'INET' | 'INT' | 'SMALLINT' | 'TEXT' | 'TIME'
               | 'TIMESTAMP' | 'TIMEUUID' | 'TINYINT' | 'UUID'
               | 'VARCHAR' | 'VARINT'

<collection_type> ::= 'LIST' '<' <cql_type> '>'
                    | 'SET' '<' <cql_type> '>'
                    | 'MAP' '<' <cql_type> ',' <cql_type> '>'

<tuple_type> ::= 'TUPLE' '<' <cql_type> (',' <cql_type>)* '>'

<term> ::= <literal>
         | <bind_marker>
         | <function_call>
         | <type_hint>
         | <collection_literal>
         | <term> <arithmetic_op> <term>
         | '-' <term>

<literal> ::= <string_literal>
            | <integer_literal>
            | <float_literal>
            | <boolean_literal>
            | <uuid_literal>
            | <blob_literal>
            | 'NULL'
            | 'NAN'
            | 'INFINITY'

<bind_marker> ::= '?'
                | ':' <identifier>

<function_call> ::= [<keyspace_name> '.'] <identifier> '(' [<term> (',' <term>)*] ')'

<collection_literal> ::= <list_literal>
                       | <set_literal>
                       | <map_literal>
                       | <tuple_literal>
                       | <udt_literal>

<list_literal> ::= '[' [<term> (',' <term>)*] ']'

<set_literal> ::= '{' [<term> (',' <term>)*] '}'

<map_literal> ::= '{' [<term> ':' <term> (',' <term> ':' <term>)*] '}'

<tuple_literal> ::= '(' <term> (',' <term>)* ')'

<udt_literal> ::= '{' <identifier> ':' <term> (',' <identifier> ':' <term>)* '}'

<table_name> ::= [<keyspace_name> '.'] <identifier>

<column_name> ::= <identifier>

<identifier> ::= <unquoted_identifier>
               | <quoted_identifier>

<unquoted_identifier> ::= [a-zA-Z] [a-zA-Z0-9_]*

<quoted_identifier> ::= '"' (~["\r\n] | '""')* '"'
```

---

## 8. Useful Implementation Patterns

### 8.1 Query Builder Pattern

```rust
pub struct SelectBuilder {
    distinct: bool,
    json: bool,
    selectors: Vec<Selector>,
    table: Option<TableName>,
    where_clause: Vec<Relation>,
    order_by: Vec<OrderingClause>,
    limit: Option<i32>,
    allow_filtering: bool,
}

impl SelectBuilder {
    pub fn new() -> Self {
        Self {
            distinct: false,
            json: false,
            selectors: vec![],
            table: None,
            where_clause: vec![],
            order_by: vec![],
            limit: None,
            allow_filtering: false,
        }
    }
    
    pub fn column(mut self, name: &str) -> Self {
        self.selectors.push(Selector::Column(ColumnName {
            name: Identifier(name.to_string()),
        }));
        self
    }
    
    pub fn all(mut self) -> Self {
        self.selectors.push(Selector::All);
        self
    }
    
    pub fn from(mut self, keyspace: Option<&str>, table: &str) -> Self {
        self.table = Some(TableName {
            keyspace: keyspace.map(|k| Identifier(k.to_string())),
            table: Identifier(table.to_string()),
        });
        self
    }
    
    pub fn where_eq(mut self, column: &str, value: Term) -> Self {
        self.where_clause.push(Relation::Simple {
            column: ColumnName { name: Identifier(column.to_string()) },
            operator: RelationOperator::Equal,
            term: value,
        });
        self
    }
    
    pub fn where_in(mut self, column: &str, values: Vec<Term>) -> Self {
        self.where_clause.push(Relation::In {
            column: ColumnName { name: Identifier(column.to_string()) },
            terms: values,
        });
        self
    }
    
    pub fn order_by(mut self, column: &str, direction: OrderDirection) -> Self {
        self.order_by.push(OrderingClause {
            column: ColumnName { name: Identifier(column.to_string()) },
            direction,
        });
        self
    }
    
    pub fn limit(mut self, n: i32) -> Self {
        self.limit = Some(n);
        self
    }
    
    pub fn allow_filtering(mut self) -> Self {
        self.allow_filtering = true;
        self
    }
    
    pub fn build(self) -> Result<SelectStatement, BuildError> {
        Ok(SelectStatement {
            distinct: self.distinct,
            json: self.json,
            selectors: if self.selectors.is_empty() {
                vec![Selector::All]
            } else {
                self.selectors
            },
            from: self.table.ok_or(BuildError::MissingTable)?,
            where_clause: if self.where_clause.is_empty() {
                None
            } else {
                Some(WhereClause { relations: self.where_clause })
            },
            group_by: None,
            order_by: if self.order_by.is_empty() {
                None
            } else {
                Some(self.order_by)
            },
            per_partition_limit: None,
            limit: self.limit,
            allow_filtering: self.allow_filtering,
        })
    }
    
    pub fn to_cql(&self) -> String {
        // Generate CQL string from builder
        let mut cql = String::from("SELECT ");
        
        if self.distinct {
            cql.push_str("DISTINCT ");
        }
        if self.json {
            cql.push_str("JSON ");
        }
        
        // Selectors
        if self.selectors.is_empty() {
            cql.push('*');
        } else {
            let selectors: Vec<String> = self.selectors.iter()
                .map(|s| format_selector(s))
                .collect();
            cql.push_str(&selectors.join(", "));
        }
        
        // FROM
        if let Some(table) = &self.table {
            cql.push_str(" FROM ");
            if let Some(ks) = &table.keyspace {
                cql.push_str(&ks.0);
                cql.push('.');
            }
            cql.push_str(&table.table.0);
        }
        
        // WHERE
        if !self.where_clause.is_empty() {
            cql.push_str(" WHERE ");
            let relations: Vec<String> = self.where_clause.iter()
                .map(|r| format_relation(r))
                .collect();
            cql.push_str(&relations.join(" AND "));
        }
        
        // ORDER BY
        if !self.order_by.is_empty() {
            cql.push_str(" ORDER BY ");
            let orders: Vec<String> = self.order_by.iter()
                .map(|o| format!(
                    "{} {}",
                    o.column.name.0,
                    match o.direction {
                        OrderDirection::Asc => "ASC",
                        OrderDirection::Desc => "DESC",
                    }
                ))
                .collect();
            cql.push_str(&orders.join(", "));
        }
        
        // LIMIT
        if let Some(limit) = self.limit {
            cql.push_str(&format!(" LIMIT {}", limit));
        }
        
        // ALLOW FILTERING
        if self.allow_filtering {
            cql.push_str(" ALLOW FILTERING");
        }
        
        cql
    }
}

fn format_selector(s: &Selector) -> String {
    match s {
        Selector::All => "*".to_string(),
        Selector::Column(c) => c.name.0.clone(),
        Selector::ColumnAs(c, alias) => format!("{} AS {}", c.name.0, alias.0),
        // ... other cases
        _ => unimplemented!(),
    }
}

fn format_relation(r: &Relation) -> String {
    match r {
        Relation::Simple { column, operator, term } => {
            format!("{} {} {}", column.name.0, format_op(operator), format_term(term))
        }
        Relation::In { column, terms } => {
            let values: Vec<String> = terms.iter().map(format_term).collect();
            format!("{} IN ({})", column.name.0, values.join(", "))
        }
        // ... other cases
        _ => unimplemented!(),
    }
}

fn format_op(op: &RelationOperator) -> &'static str {
    match op {
        RelationOperator::Equal => "=",
        RelationOperator::NotEqual => "!=",
        RelationOperator::LessThan => "<",
        RelationOperator::LessEqual => "<=",
        RelationOperator::GreaterThan => ">",
        RelationOperator::GreaterEqual => ">=",
    }
}

fn format_term(t: &Term) -> String {
    match t {
        Term::String(s) => format!("'{}'", s.replace("'", "''")),
        Term::Integer(i) => i.to_string(),
        Term::Float(f) => f.to_string(),
        Term::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
        Term::Null => "null".to_string(),
        Term::PositionalBind => "?".to_string(),
        Term::NamedBind(id) => format!(":{}", id.0),
        Term::Uuid(u) => u.to_string(),
        // ... other cases
        _ => unimplemented!(),
    }
}
```

### 8.2 Type-Safe Value Extraction

```rust
pub trait FromCqlValue: Sized {
    fn from_cql(value: &CqlValue) -> Result<Self, TypeError>;
}

impl FromCqlValue for i32 {
    fn from_cql(value: &CqlValue) -> Result<Self, TypeError> {
        match value {
            CqlValue::Int(v) => Ok(*v),
            _ => Err(TypeError::TypeMismatch {
                expected: "int",
                found: value.type_name(),
            }),
        }
    }
}

impl FromCqlValue for i64 {
    fn from_cql(value: &CqlValue) -> Result<Self, TypeError> {
        match value {
            CqlValue::Bigint(v) => Ok(*v),
            CqlValue::Counter(v) => Ok(*v),
            CqlValue::Timestamp(v) => Ok(*v),
            CqlValue::Time(v) => Ok(*v),
            _ => Err(TypeError::TypeMismatch {
                expected: "bigint",
                found: value.type_name(),
            }),
        }
    }
}

impl FromCqlValue for String {
    fn from_cql(value: &CqlValue) -> Result<Self, TypeError> {
        match value {
            CqlValue::Text(v) => Ok(v.clone()),
            CqlValue::Ascii(v) => Ok(v.clone()),
            _ => Err(TypeError::TypeMismatch {
                expected: "text",
                found: value.type_name(),
            }),
        }
    }
}

impl<T: FromCqlValue> FromCqlValue for Option<T> {
    fn from_cql(value: &CqlValue) -> Result<Self, TypeError> {
        match value {
            CqlValue::Null => Ok(None),
            _ => Ok(Some(T::from_cql(value)?)),
        }
    }
}

impl<T: FromCqlValue> FromCqlValue for Vec<T> {
    fn from_cql(value: &CqlValue) -> Result<Self, TypeError> {
        match value {
            CqlValue::List(items) => {
                items.iter().map(T::from_cql).collect()
            }
            CqlValue::Set(items) => {
                items.iter().map(T::from_cql).collect()
            }
            _ => Err(TypeError::TypeMismatch {
                expected: "list or set",
                found: value.type_name(),
            }),
        }
    }
}

// Row value extraction
impl Row {
    pub fn get<T: FromCqlValue>(&self, index: usize) -> Result<T, RowError> {
        let value = self.columns.get(index)
            .ok_or(RowError::IndexOutOfBounds)?;
        
        match value {
            Some(v) => T::from_cql(v).map_err(RowError::TypeError),
            None => T::from_cql(&CqlValue::Null).map_err(RowError::TypeError),
        }
    }
}
```

---

## 9. ScyllaDB Extensions

ScyllaDB implements CQL with some extensions:

### 9.1 Additional Features

```rust
// ScyllaDB-specific options
pub struct ScyllaTableOptions {
    // Standard options
    pub bloom_filter_fp_chance: Option<f64>,
    // ... standard options
    
    // ScyllaDB extensions
    pub cdc: Option<CdcOptions>,
    pub per_partition_rate_limit: Option<PerPartitionRateLimit>,
}

#[derive(Debug, Clone)]
pub struct CdcOptions {
    pub enabled: bool,
    pub preimage: bool,
    pub postimage: bool,
    pub ttl: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct PerPartitionRateLimit {
    pub max_writes_per_second: Option<i32>,
    pub max_reads_per_second: Option<i32>,
}

// ScyllaDB BYPASS CACHE hint
#[derive(Debug, Clone)]
pub struct SelectStatementScylla {
    pub base: SelectStatement,
    pub bypass_cache: bool,
}

// USING TIMEOUT extension
#[derive(Debug, Clone)]
pub struct UpdateParametersScylla {
    pub timestamp: Option<Term>,
    pub ttl: Option<Term>,
    pub timeout: Option<Duration>,  // ScyllaDB extension
}
```

### 9.2 Shard-Aware Routing

```rust
pub struct ShardAwareConnection {
    shard_id: u16,
    shard_count: u16,
    connection: Connection,
}

impl ShardAwareConnection {
    pub fn compute_shard(
        partition_key: &[u8],
        shard_count: u16,
    ) -> u16 {
        // ScyllaDB uses murmur3 hash
        let token = murmur3_hash(partition_key);
        // Map token to shard
        ((token as u64 * shard_count as u64) >> 64) as u16
    }
}

fn murmur3_hash(key: &[u8]) -> i64 {
    // Cassandra/ScyllaDB murmur3 implementation
    const C1: i64 = -0x783C846EEEBDAC2B_i64; // 0x87c37b91114253d5
    const C2: i64 = 0x4cf5ad432745937f;
    
    let mut h1: i64 = 0;
    let mut h2: i64 = 0;
    
    let chunks = key.chunks_exact(16);
    let remainder = chunks.remainder();
    
    for chunk in chunks {
        let mut k1 = i64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let mut k2 = i64::from_le_bytes(chunk[8..16].try_into().unwrap());
        
        k1 = k1.wrapping_mul(C1);
        k1 = k1.rotate_left(31);
        k1 = k1.wrapping_mul(C2);
        h1 ^= k1;
        
        h1 = h1.rotate_left(27);
        h1 = h1.wrapping_add(h2);
        h1 = h1.wrapping_mul(5).wrapping_add(0x52dce729);
        
        k2 = k2.wrapping_mul(C2);
        k2 = k2.rotate_left(33);
        k2 = k2.wrapping_mul(C1);
        h2 ^= k2;
        
        h2 = h2.rotate_left(31);
        h2 = h2.wrapping_add(h1);
        h2 = h2.wrapping_mul(5).wrapping_add(0x38495ab5);
    }
    
    // Handle remainder
    // ... (remainder processing)
    
    // Finalization
    h1 ^= key.len() as i64;
    h2 ^= key.len() as i64;
    
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    
    h1 = fmix64(h1);
    h2 = fmix64(h2);
    
    h1.wrapping_add(h2)
}

fn fmix64(mut k: i64) -> i64 {
    k ^= k >> 33;
    k = k.wrapping_mul(-0x0ae502812aa7333_i64); // 0xff51afd7ed558ccd
    k ^= k >> 33;
    k = k.wrapping_mul(-0x4b6d12be76b66b51_i64); // 0xc4ceb9fe1a85ec53
    k ^= k >> 33;
    k
}
```

---

## 10. Error Handling Best Practices

```rust
#[derive(Debug, thiserror::Error)]
pub enum CqlError {
    #[error("Connection error: {0}")]
    Connection(#[from] ConnectionError),
    
    #[error("Query error: {0}")]
    Query(#[from] QueryError),
    
    #[error("Parse error: {0}")]
    Parse(#[from] ParseError),
    
    #[error("Type error: {0}")]
    Type(#[from] TypeError),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
}

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Server error: {code:?} - {message}")]
    ServerError {
        code: ErrorCode,
        message: String,
    },
    
    #[error("Query unprepared - needs re-preparation")]
    Unprepared,
    
    #[error("No stream ID available")]
    NoStreamAvailable,
    
    #[error("Timeout waiting for response")]
    Timeout,
    
    #[error("Unexpected response type")]
    UnexpectedResponse,
}

// Retry policy
pub trait RetryPolicy: Send + Sync {
    fn on_read_timeout(
        &self,
        query: &str,
        consistency: Consistency,
        received: i32,
        required: i32,
        data_retrieved: bool,
        retry_count: usize,
    ) -> RetryDecision;
    
    fn on_write_timeout(
        &self,
        query: &str,
        consistency: Consistency,
        write_type: &str,
        received: i32,
        required: i32,
        retry_count: usize,
    ) -> RetryDecision;
    
    fn on_unavailable(
        &self,
        query: &str,
        consistency: Consistency,
        required: i32,
        alive: i32,
        retry_count: usize,
    ) -> RetryDecision;
}

pub enum RetryDecision {
    Retry(Option<Consistency>),
    RetrySameHost(Option<Consistency>),
    DontRetry,
}

pub struct DefaultRetryPolicy {
    max_retries: usize,
}

impl RetryPolicy for DefaultRetryPolicy {
    fn on_read_timeout(
        &self,
        _query: &str,
        _consistency: Consistency,
        received: i32,
        required: i32,
        data_retrieved: bool,
        retry_count: usize,
    ) -> RetryDecision {
        if retry_count >= self.max_retries {
            return RetryDecision::DontRetry;
        }
        
        // Retry if we have enough responses but no data
        if received >= required && !data_retrieved {
            RetryDecision::RetrySameHost(None)
        } else {
            RetryDecision::DontRetry
        }
    }
    
    fn on_write_timeout(
        &self,
        _query: &str,
        _consistency: Consistency,
        write_type: &str,
        _received: i32,
        _required: i32,
        retry_count: usize,
    ) -> RetryDecision {
        if retry_count >= self.max_retries {
            return RetryDecision::DontRetry;
        }
        
        // Only retry batch log writes
        if write_type == "BATCH_LOG" {
            RetryDecision::RetrySameHost(None)
        } else {
            RetryDecision::DontRetry
        }
    }
    
    fn on_unavailable(
        &self,
        _query: &str,
        _consistency: Consistency,
        _required: i32,
        _alive: i32,
        retry_count: usize,
    ) -> RetryDecision {
        if retry_count >= self.max_retries {
            RetryDecision::DontRetry
        } else {
            RetryDecision::Retry(None)
        }
    }
}
```

---

## Additional Resources

- **Apache Cassandra CQL Documentation**: https://cassandra.apache.org/doc/latest/cql/
- **ScyllaDB CQL Reference**: https://docs.scylladb.com/stable/cql/
- **CQL Native Protocol Specification**: https://github.com/apache/cassandra/blob/trunk/doc/native_protocol_v4.spec
- **Existing Rust Drivers**:
  - `scylla` crate: https://github.com/scylladb/scylla-rust-driver
  - `cdrs-tokio` crate: https://github.com/krojew/cdrs-tokio

---

*This reference enables implementation of CQL parsers, AST structures, wire protocol encoders/decoders, and connection management in Rust.*
