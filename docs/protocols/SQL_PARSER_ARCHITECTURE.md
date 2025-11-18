---
layout: default
title: SQL Parser Architecture Design
category: protocols
---

## SQL Parser Architecture Design

## Overview

This document outlines the comprehensive SQL parser architecture for Orbit-RS that will provide full ANSI SQL compliance while maintaining compatibility with existing vector operations and actor-based storage.

## Current State Analysis

The current `query_engine.rs` implementation provides:

- Basic SQL parsing for SELECT, INSERT, UPDATE, DELETE
- Simple WHERE clause support with single conditions
- Vector query detection and routing to VectorQueryEngine
- Actor-based storage simulation

### Limitations

- No support for JOINs, subqueries, or complex expressions
- Limited WHERE clause parsing (single condition only)
- No DDL (CREATE, ALTER, DROP) support
- No DCL (GRANT, REVOKE) or TCL (BEGIN, COMMIT) support
- No aggregate functions, window functions, or CTEs
- Hardcoded to "actors" table only
- Basic string-based parsing prone to errors

## New Architecture Design

### 1. Modular Parser Architecture

```text
src/postgres_wire/sql/
├── mod.rs                    // Public API and parser factory
├── lexer.rs                  // SQL tokenization
├── ast.rs                    // Abstract Syntax Tree definitions
├── parser/
│   ├── mod.rs               // Parser orchestration
│   ├── ddl.rs               // Data Definition Language
│   ├── dml.rs               // Data Manipulation Language
│   ├── dcl.rs               // Data Control Language
│   ├── tcl.rs               // Transaction Control Language
│   ├── expressions.rs       // Expression parsing
│   ├── functions.rs         // Function calls and operators
│   └── utilities.rs         // Common parsing utilities
├── analyzer/
│   ├── mod.rs               // Semantic analysis
│   ├── type_checker.rs      // Type validation
│   ├── schema_validator.rs  // Schema existence validation
│   └── optimizer.rs         // Basic query optimization
├── executor/
│   ├── mod.rs               // Execution engine
│   ├── ddl_executor.rs      // DDL execution
│   ├── dml_executor.rs      // DML execution
│   ├── dcl_executor.rs      // DCL execution
│   ├── tcl_executor.rs      // TCL execution
│   └── expression_evaluator.rs // Expression evaluation
└── types/
    ├── mod.rs               // SQL type system
    ├── data_types.rs        // SQL data types
    └── value.rs             // Runtime values
```

### 2. Abstract Syntax Tree (AST) Design

The AST will represent all SQL constructs in a structured format:

```rust
// Core AST node types
pub enum Statement {
    // Data Definition Language
    CreateTable(CreateTableStatement),
    CreateIndex(CreateIndexStatement),
    CreateView(CreateViewStatement),
    CreateSchema(CreateSchemaStatement),
    AlterTable(AlterTableStatement),
    DropTable(DropTableStatement),
    DropIndex(DropIndexStatement),
    
    // Data Manipulation Language
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    
    // Data Control Language
    Grant(GrantStatement),
    Revoke(RevokeStatement),
    
    // Transaction Control Language
    Begin(BeginStatement),
    Commit(CommitStatement),
    Rollback(RollbackStatement),
    Savepoint(SavepointStatement),
    
    // Utility statements
    Explain(ExplainStatement),
    Show(ShowStatement),
    Use(UseStatement),
    
    // Vector extensions (preserved)
    CreateExtension(CreateExtensionStatement),
}

// Enhanced SELECT statement support
pub struct SelectStatement {
    pub select_list: Vec<SelectItem>,
    pub from_clause: Option<FromClause>,
    pub where_clause: Option<Expression>,
    pub group_by: Option<Vec<Expression>>,
    pub having: Option<Expression>,
    pub order_by: Option<Vec<OrderByItem>>,
    pub limit: Option<LimitClause>,
    pub offset: Option<u64>,
    pub distinct: bool,
    pub for_update: bool,
}

// Complex FROM clause support
pub enum FromClause {
    Table {
        name: TableName,
        alias: Option<String>,
    },
    Join {
        left: Box<FromClause>,
        join_type: JoinType,
        right: Box<FromClause>,
        condition: JoinCondition,
    },
    Subquery {
        query: Box<SelectStatement>,
        alias: String,
    },
    Values {
        values: Vec<Vec<Expression>>,
        alias: Option<String>,
    },
}

// Expression system for complex queries
pub enum Expression {
    Column(ColumnRef),
    Literal(LiteralValue),
    Function(FunctionCall),
    Binary {
        left: Box<Expression>,
        operator: BinaryOperator,
        right: Box<Expression>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    Case(CaseExpression),
    Subquery(Box<SelectStatement>),
    In {
        expr: Box<Expression>,
        list: Vec<Expression>,
        negated: bool,
    },
    Between {
        expr: Box<Expression>,
        low: Box<Expression>,
        high: Box<Expression>,
        negated: bool,
    },
    Exists(Box<SelectStatement>),
    // Vector extensions
    VectorSimilarity {
        left: Box<Expression>,
        operator: VectorOperator,
        right: Box<Expression>,
    },
}
```

### 3. Lexical Analysis (Tokenizer)

A robust tokenizer that handles:

- SQL keywords (case-insensitive)
- Identifiers (quoted and unquoted)
- String literals (with proper escaping)
- Numeric literals (integers, decimals, scientific notation)
- Operators and punctuation
- Comments (single-line and multi-line)
- Vector-specific operators (<->, <#>, <=>)

```rust
pub enum Token {
    // Keywords
    Select, From, Where, Insert, Update, Delete, Create, Alter, Drop,
    Join, Inner, Left, Right, Full, Outer, On, Using,
    Group, By, Order, Having, Distinct, Limit, Offset,
    Begin, Commit, Rollback, Savepoint,
    And, Or, Not, In, Between, Like, Is, Null,
    
    // Identifiers and literals
    Identifier(String),
    QuotedIdentifier(String),
    StringLiteral(String),
    NumericLiteral(String),
    
    // Operators
    Equal, NotEqual, LessThan, LessThanOrEqual,
    GreaterThan, GreaterThanOrEqual,
    Plus, Minus, Multiply, Divide, Modulo,
    
    // Vector operators
    VectorDistance,     // <->
    VectorInnerProduct, // <#>
    VectorCosineDistance, // <=>
    
    // Punctuation
    LeftParen, RightParen, Comma, Semicolon, Dot,
    
    // Special
    Eof, Whitespace, Comment,
}
```

### 4. Parser Implementation Strategy

#### Recursive Descent Parser

Use a recursive descent parser with:

- Precedence climbing for expressions
- Look-ahead for disambiguation
- Error recovery and detailed error messages
- Support for PostgreSQL-specific syntax extensions

#### Key Parser Components

1. **Statement Parser**: Top-level SQL statement parsing
2. **Expression Parser**: Complex expression parsing with operator precedence
3. **Type Parser**: SQL data type parsing
4. **Identifier Parser**: Table names, column names, aliases
5. **Function Parser**: SQL functions and aggregates
6. **Subquery Parser**: Nested query support

### 5. Semantic Analysis

#### Schema Integration

- Interface with Orbit's actor-based schema
- Virtual table definitions for actors, vector stores
- Type checking and validation
- Dependency resolution

#### Query Planning

- Basic query optimization
- Join order optimization
- Index usage recommendations
- Vector query optimization

### 6. Execution Engine Integration

#### Actor Storage Mapping

```rust
// Map SQL tables to actor types
pub struct SchemaMapping {
    pub tables: HashMap<String, TableDefinition>,
    pub actor_types: HashMap<String, ActorTypeMapping>,
    pub vector_collections: HashMap<String, VectorCollection>,
}

pub struct TableDefinition {
    pub name: String,
    pub columns: Vec<ColumnDefinition>,
    pub primary_key: Option<Vec<String>>,
    pub indexes: Vec<IndexDefinition>,
    pub table_type: TableType,
}

pub enum TableType {
    ActorTable { actor_type: String },
    VectorTable { collection: String },
    SystemTable,
    VirtualTable,
}
```

#### Query Execution Pipeline

1. **Parse** → AST
2. **Analyze** → Validated AST + Query Plan
3. **Execute** → Interact with OrbitClient/VectorEngine
4. **Format** → PostgreSQL wire protocol response

### 7. ANSI SQL Compliance Features

#### DDL Support

- CREATE/ALTER/DROP TABLE with constraints
- CREATE/DROP INDEX with various types
- CREATE/DROP VIEW and materialized views
- CREATE/DROP SCHEMA
- CREATE/DROP USER and roles

#### Advanced DML Support

- Complex SELECT with multiple JOINs
- Common Table Expressions (WITH clause)
- Window functions (ROW_NUMBER, RANK, etc.)
- Aggregate functions (COUNT, SUM, AVG, etc.)
- Set operations (UNION, INTERSECT, EXCEPT)
- Subqueries in all contexts

#### DCL Support

- GRANT/REVOKE permissions on tables and schemas
- Role-based access control
- Integration with Orbit's security model

#### TCL Support

- BEGIN/COMMIT/ROLLBACK transactions
- SAVEPOINT and nested transactions
- Transaction isolation levels
- Deadlock detection and resolution

### 8. Vector Query Integration

#### Seamless Integration

The new parser will seamlessly integrate vector operations:

- Detect vector data types in DDL
- Parse vector similarity operators in expressions
- Route vector queries to VectorQueryEngine
- Support hybrid SQL + vector queries

#### Enhanced Vector Features

```sql
-- Vector similarity in complex queries
SELECT a.*, similarity_score
FROM articles a,
     LATERAL (SELECT embedding <-> $1 as similarity_score 
              FROM vector_store v 
              WHERE v.article_id = a.id) s
ORDER BY s.similarity_score
LIMIT 10;

-- Vector operations in joins
SELECT a.title, b.title, a.embedding <-> b.embedding as distance
FROM articles a
JOIN articles b ON a.category = b.category
WHERE a.embedding <-> b.embedding < 0.8;
```

### 9. Error Handling and Recovery

#### Comprehensive Error Messages

- Syntax error location with line/column numbers
- Suggestion for common mistakes
- Context-aware error messages
- Recovery strategies for partial parsing

#### Error Types

```rust
pub enum SqlError {
    SyntaxError { message: String, location: Location },
    SemanticError { message: String, context: String },
    TypeMismatch { expected: SqlType, found: SqlType },
    UnknownTable { table: String },
    UnknownColumn { column: String, table: Option<String> },
    PermissionDenied { operation: String, resource: String },
    TransactionError { message: String },
    VectorError { message: String },
}
```

### 10. Performance Considerations

#### Optimization Strategies

- Prepared statement caching
- Query plan caching
- Lazy evaluation of complex expressions
- Parallel execution for independent operations
- Efficient memory management for large result sets

#### Benchmarking Framework

- Performance tests for all SQL constructs
- Comparison with PostgreSQL reference implementation
- Memory usage profiling
- Scalability testing with large datasets

### 11. Testing Strategy

#### Test Categories

1. **Unit Tests**: Parser components, AST nodes, expression evaluation
2. **Integration Tests**: Full SQL statement parsing and execution
3. **Compatibility Tests**: PostgreSQL client compatibility
4. **Performance Tests**: Query execution benchmarks
5. **Regression Tests**: Ensure vector functionality remains intact

#### Test Data

- Comprehensive SQL statement test suite
- Edge cases and error conditions
- Complex real-world queries
- Vector + SQL hybrid queries
- Large dataset performance tests

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)

- Lexer implementation
- Basic AST definitions
- Core parser infrastructure
- Simple statement parsing

### Phase 2: DML Enhancement (Weeks 3-4)

- Enhanced SELECT with JOINs
- Complex WHERE clauses
- Subquery support
- Expression evaluation engine

### Phase 3: DDL Implementation (Weeks 5-6)

- CREATE/ALTER/DROP TABLE
- Index management
- Schema operations
- Constraint handling

### Phase 4: Advanced Features (Weeks 7-8)

- Aggregate functions and GROUP BY
- Window functions
- Common Table Expressions
- Set operations

### Phase 5: Control Languages (Weeks 9-10)

- DCL implementation (GRANT/REVOKE)
- TCL implementation (transactions)
- Security integration
- Permission checking

### Phase 6: Integration & Testing (Weeks 11-12)

- Vector query integration
- Comprehensive testing
- Performance optimization
- Documentation

## Migration Strategy

### Backward Compatibility

- Existing vector operations continue to work
- Current basic SQL queries remain functional
- Gradual migration of functionality to new parser
- Feature flags for new capabilities

### Integration Points

- Maintain existing QueryEngine interface
- Extend with new capabilities
- Preserve VectorQueryEngine integration
- Keep PostgreSQL wire protocol unchanged

## Success Metrics

### Compliance

- Support for 95% of common ANSI SQL constructs
- Pass PostgreSQL compatibility test suite
- Support complex real-world queries
- Maintain vector operation performance

### Performance

- Parse 1000+ SQL statements per second
- Execute simple queries within 1ms
- Handle result sets up to 100MB
- Support 1000+ concurrent connections

### Quality

- 95%+ test coverage
- Zero memory leaks
- Detailed error messages
- Comprehensive documentation

This architecture provides a solid foundation for building a comprehensive SQL parser that maintains the excellent vector capabilities while adding full ANSI SQL compliance to Orbit-RS.
