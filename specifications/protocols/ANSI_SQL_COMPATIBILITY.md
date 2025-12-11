# ANSI SQL Compatibility Specification

**Target**: SQL:2023 (ISO/IEC 9075:2023) Standard Compliance
**Reference**: https://www.iso.org/standard/76583.html
**Last Updated**: 2025-12-09
**Current Estimated Coverage**: ~70%

---

## Overview

This document specifies OrbitRS's compliance with the ANSI/ISO SQL standard. The goal is to provide maximum portability across SQL databases by supporting core SQL features defined in the SQL:2023 standard.

## Table of Contents

1. [SQL Standard Features](#sql-standard-features)
2. [Core SQL](#core-sql)
3. [Optional Features](#optional-features)
4. [Implementation Status](#implementation-status)

---

## SQL Standard Features

### Legend
- ✅ **Implemented** - Fully compliant
- 🔶 **Partial** - Partially compliant
- ❌ **Not Implemented** - Not supported
- 🚫 **Not Planned** - Out of scope

---

## Core SQL (Mandatory Features)

### E011: Numeric Data Types

| Feature | Status | Notes |
|---------|--------|-------|
| E011-01: INTEGER | ✅ | Full support |
| E011-02: SMALLINT | ✅ | Full support |
| E011-03: DECIMAL | ✅ | Full support |
| E011-04: NUMERIC | ✅ | Full support |
| E011-05: REAL | ✅ | Full support |
| E011-06: DOUBLE PRECISION | ✅ | Full support |
| E011-07: FLOAT | ✅ | Full support |

### E021: Character String Types

| Feature | Status | Notes |
|---------|--------|-------|
| E021-01: CHARACTER | ✅ | CHAR support |
| E021-02: CHARACTER VARYING | ✅ | VARCHAR support |
| E021-03: CHARACTER literals | ✅ | String literals |
| E021-04: CHARACTER_LENGTH | ✅ | Function support |
| E021-05: OCTET_LENGTH | ✅ | Function support |
| E021-06: SUBSTRING | ✅ | Function support |
| E021-07: CONCATENATION | ✅ | || operator |
| E021-08: UPPER/LOWER | ✅ | Case functions |
| E021-09: TRIM | ✅ | TRIM function |
| E021-10: POSITION | ✅ | Function support |
| E021-11: Comparison predicates | ✅ | =, <>, <, >, <=, >= |
| E021-12: LIKE predicate | ✅ | Pattern matching |

### E031: Identifiers

| Feature | Status | Notes |
|---------|--------|-------|
| E031-01: Delimited identifiers | ✅ | Quoted identifiers |
| E031-02: Lower case identifiers | ✅ | Case-insensitive |
| E031-03: Trailing underscore | ✅ | Allowed |

### E051: Basic Query Specification

| Feature | Status | Notes |
|---------|--------|-------|
| E051-01: SELECT DISTINCT | ✅ | Full support |
| E051-02: GROUP BY | ✅ | Full support |
| E051-04: GROUP BY can contain columns not in SELECT | ✅ | Full support |
| E051-05: SELECT list items can be renamed | ✅ | AS clause |
| E051-06: HAVING clause | ✅ | Full support |
| E051-07: Qualified * in SELECT list | ✅ | table.* |
| E051-08: Correlation names in FROM | ✅ | Table aliases |
| E051-09: Rename columns in FROM | ✅ | Column aliases |

### E061: Basic Predicates and Search Conditions

| Feature | Status | Notes |
|---------|--------|-------|
| E061-01: Comparison predicate | ✅ | =, <>, <, >, <=, >= |
| E061-02: BETWEEN predicate | ✅ | Full support |
| E061-03: IN predicate with list | ✅ | Full support |
| E061-04: LIKE predicate | ✅ | Pattern matching |
| E061-05: LIKE ESCAPE | ✅ | Escape character |
| E061-06: NULL predicate | ✅ | IS NULL, IS NOT NULL |
| E061-07: Quantified comparison | 🔶 | ANY, ALL, SOME |
| E061-08: EXISTS predicate | ✅ | Full support |
| E061-09: Subqueries in comparison | ✅ | Full support |
| E061-11: Subqueries in IN | ✅ | Full support |
| E061-12: Subqueries in quantified comparison | 🔶 | Partial support |
| E061-13: Correlated subqueries | ✅ | Full support |
| E061-14: Search condition | ✅ | AND, OR, NOT |

### E071: Basic Query Expressions

| Feature | Status | Notes |
|---------|--------|-------|
| E071-01: UNION DISTINCT | ✅ | Full support |
| E071-02: UNION ALL | ✅ | Full support |
| E071-03: EXCEPT DISTINCT | ✅ | Full support |
| E071-05: Columns combined via table operators need not have same name | ✅ | Full support |
| E071-06: Table operators in subqueries | ✅ | Full support |

### E081: Basic Privileges

| Feature | Status | Notes |
|---------|--------|-------|
| E081-01: SELECT privilege | 🔶 | Basic support |
| E081-02: DELETE privilege | 🔶 | Basic support |
| E081-03: INSERT privilege | 🔶 | Basic support |
| E081-04: UPDATE privilege | 🔶 | Basic support |
| E081-05: REFERENCES privilege | ❌ | Not implemented |
| E081-06: USAGE privilege | ❌ | Not implemented |
| E081-07: EXECUTE privilege | ❌ | Not implemented |
| E081-08: GRANT | 🔶 | Basic support |
| E081-09: REVOKE | 🔶 | Basic support |
| E081-10: GRANT OPTION | ❌ | Not implemented |

### E091: Set Functions

| Feature | Status | Notes |
|---------|--------|-------|
| E091-01: AVG | ✅ | Full support |
| E091-02: COUNT | ✅ | Full support |
| E091-03: MAX | ✅ | Full support |
| E091-04: MIN | ✅ | Full support |
| E091-05: SUM | ✅ | Full support |
| E091-06: ALL quantifier | ✅ | Full support |
| E091-07: DISTINCT quantifier | ✅ | Full support |

### E101: Basic Data Manipulation

| Feature | Status | Notes |
|---------|--------|-------|
| E101-01: INSERT statement | ✅ | Full support |
| E101-03: Searched UPDATE | ✅ | Full support |
| E101-04: Searched DELETE | ✅ | Full support |

### E111: Single Row SELECT

| Feature | Status | Notes |
|---------|--------|-------|
| E111-01: SELECT INTO | ✅ | Full support |

### E121: Basic Cursor Support

| Feature | Status | Notes |
|---------|--------|-------|
| E121-01: DECLARE CURSOR | ✅ | Full support |
| E121-02: ORDER BY in cursors | ✅ | Supported via SELECT |
| E121-03: Value expressions in ORDER BY | ✅ | Supported via SELECT |
| E121-04: OPEN statement | ❌ | Not implemented (Implicit in DECLARE) |
| E121-06: Positioned UPDATE | ❌ | Not implemented |
| E121-07: Positioned DELETE | ❌ | Not implemented |
| E121-08: CLOSE statement | ✅ | Full support |
| E121-10: FETCH statement | ✅ | Full support |
| E121-17: WITH HOLD cursors | ✅ | Full support |

### E131: Null Value Support

| Feature | Status | Notes |
|---------|--------|-------|
| E131-01: NULL values | ✅ | Full support |
| E131-02: NULL in arithmetic | ✅ | Full support |
| E131-03: NULL in comparison | ✅ | Full support |
| E131-04: NULL in set functions | ✅ | Full support |
| E131-05: NULL in DISTINCT | ✅ | Full support |
| E131-06: NULL in UNIQUE | ✅ | Full support |

### E141: Basic Integrity Constraints

| Feature | Status | Notes |
|---------|--------|-------|
| E141-01: NOT NULL | ✅ | Full support |
| E141-02: UNIQUE | ✅ | Full support |
| E141-03: PRIMARY KEY | ✅ | Full support |
| E141-04: Basic FOREIGN KEY | ✅ | Full support |
| E141-06: CHECK constraint | ✅ | Full support |
| E141-07: Column defaults | ✅ | Full support |
| E141-08: NOT NULL inferred on PRIMARY KEY | ✅ | Full support |
| E141-10: Names in a foreign key can be specified in any order | ✅ | Full support |

### E151: Transaction Support

| Feature | Status | Notes |
|---------|--------|-------|
| E151-01: COMMIT | ✅ | Full support |
| E151-02: ROLLBACK | ✅ | Full support |

### E152: Basic SET TRANSACTION

| Feature | Status | Notes |
|---------|--------|-------|
| E152-01: SET TRANSACTION ISOLATION LEVEL | 🔶 | Basic support |
| E152-02: READ UNCOMMITTED | 🔶 | Basic support |
| E152-03: READ COMMITTED | ✅ | Full support |
| E152-04: REPEATABLE READ | ✅ | Full support |
| E152-05: SERIALIZABLE | ✅ | Full support |

### E153: Updatable Queries with Subqueries

| Feature | Status | Notes |
|---------|--------|-------|
| E153-01: UPDATE with subquery | ✅ | Full support |
| E153-02: DELETE with subquery | ✅ | Full support |

---

## Optional Features

### F031: Basic Schema Manipulation

| Feature | Status | Notes |
|---------|--------|-------|
| F031-01: CREATE TABLE | ✅ | Full support |
| F031-02: CREATE VIEW | ✅ | Full support |
| F031-03: GRANT | 🔶 | Basic support |
| F031-04: ALTER TABLE ADD COLUMN | ✅ | Full support |
| F031-13: DROP TABLE RESTRICT | ✅ | Full support |
| F031-16: DROP VIEW RESTRICT | ✅ | Full support |
| F031-19: REVOKE | 🔶 | Basic support |

### F041: Basic Joined Table

| Feature | Status | Notes |
|---------|--------|-------|
| F041-01: Inner join | ✅ | Full support |
| F041-02: INNER keyword | ✅ | Full support |
| F041-03: LEFT OUTER JOIN | ✅ | Full support |
| F041-04: RIGHT OUTER JOIN | ✅ | Full support |
| F041-05: Outer joins can be nested | ✅ | Full support |
| F041-07: FULL OUTER JOIN | ✅ | Full support |
| F041-08: All comparison operators | ✅ | Full support |

### F051: Basic Date and Time

| Feature | Status | Notes |
|---------|--------|-------|
| F051-01: DATE data type | ✅ | Full support |
| F051-02: TIME data type | ✅ | Full support |
| F051-03: TIMESTAMP data type | ✅ | Full support |
| F051-04: Comparison of dates | ✅ | Full support |
| F051-05: CAST to DATE | ✅ | Full support |
| F051-06: CURRENT_DATE | ✅ | Full support |
| F051-07: LOCALTIME | ✅ | Full support |
| F051-08: LOCALTIMESTAMP | ✅ | Full support |

### F081: UNION and EXCEPT in Views

| Feature | Status | Notes |
|---------|--------|-------|
| F081-01: UNION in views | ✅ | Full support |
| F081-02: EXCEPT in views | ✅ | Full support |

### F131: Grouped Operations

| Feature | Status | Notes |
|---------|--------|-------|
| F131-01: WHERE, GROUP BY, HAVING | ✅ | Full support |
| F131-02: Multiple tables in FROM | ✅ | Full support |
| F131-03: Set functions in subqueries | ✅ | Full support |
| F131-04: Subqueries in HAVING | ✅ | Full support |
| F131-05: Single row SELECT with GROUP BY | ✅ | Full support |

### F181: Multiple Module Support

| Feature | Status | Notes |
|---------|--------|-------|
| F181-01: Multiple schemas | ✅ | Full support |
| F181-02: Multiple modules | 🔶 | Basic support |
| F181-03: Schema name in qualified name | ✅ | Full support |
| F181-04: Catalog name in qualified name | 🔶 | Basic support |

### F201: CAST Function

| Feature | Status | Notes |
|---------|--------|-------|
| F201-01: CAST to CHARACTER | ✅ | Full support |
| F201-02: CAST to NUMERIC | ✅ | Full support |
| F201-03: CAST to DECIMAL | ✅ | Full support |
| F201-04: CAST to INTEGER | ✅ | Full support |
| F201-05: CAST to SMALLINT | ✅ | Full support |
| F201-06: CAST to FLOAT | ✅ | Full support |
| F201-07: CAST to REAL | ✅ | Full support |
| F201-08: CAST to DOUBLE PRECISION | ✅ | Full support |

### F221: Explicit Defaults

| Feature | Status | Notes |
|---------|--------|-------|
| F221-01: DEFAULT VALUES | ✅ | Full support |
| F221-02: DEFAULT in INSERT | ✅ | Full support |
| F221-03: DEFAULT in UPDATE | ✅ | Full support |
| F221-04: DEFAULT in column definition | ✅ | Full support |

### F261: CASE Expression

| Feature | Status | Notes |
|---------|--------|-------|
| F261-01: Simple CASE | ✅ | Full support |
| F261-02: Searched CASE | ✅ | Full support |
| F261-03: NULLIF | ✅ | Full support |
| F261-04: COALESCE | ✅ | Full support |

### F311: Schema Definition Statement

| Feature | Status | Notes |
|---------|--------|-------|
| F311-01: CREATE SCHEMA | ✅ | Full support |
| F311-02: CREATE TABLE for persistent base tables | ✅ | Full support |
| F311-03: CREATE VIEW | ✅ | Full support |
| F311-04: GRANT | 🔶 | Basic support |
| F311-05: CREATE DOMAIN | ❌ | Not implemented |

### F471: Scalar Subquery Values

| Feature | Status | Notes |
|---------|--------|-------|
| F471-01: Scalar subqueries in SELECT | ✅ | Full support |
| F471-02: Scalar subqueries in WHERE | ✅ | Full support |
| F471-03: Scalar subqueries in HAVING | ✅ | Full support |
| F471-04: Scalar subqueries in UPDATE | ✅ | Full support |
| F471-05: Scalar subqueries in INSERT | ✅ | Full support |

### F491: Constraint Management

| Feature | Status | Notes |
|---------|--------|-------|
| F491-01: Constraint names | ✅ | Full support |
| F491-02: ALTER TABLE DROP CONSTRAINT | ✅ | Full support |
| F491-03: ALTER TABLE ADD CONSTRAINT | ✅ | Full support |

### F501: Features and Conformance Views

| Feature | Status | Notes |
|---------|--------|-------|
| F501-01: SQL_FEATURES view | ❌ | Not implemented |
| F501-02: SQL_SIZING view | ❌ | Not implemented |
| F501-03: SQL_LANGUAGES view | ❌ | Not implemented |

### F531: Temporary Tables

| Feature | Status | Notes |
|---------|--------|-------|
| F531-01: CREATE TEMPORARY TABLE | ✅ | Full support |
| F531-02: ON COMMIT DELETE ROWS | 🔶 | Basic support |
| F531-03: ON COMMIT PRESERVE ROWS | ✅ | Full support |

### F591: Derived Tables

| Feature | Status | Notes |
|---------|--------|-------|
| F591-01: Derived tables in FROM | ✅ | Full support |
| F591-02: Derived table column names | ✅ | Full support |
| F591-03: Derived tables can be nested | ✅ | Full support |

### F611: Indicator Data Types

| Feature | Status | Notes |
|---------|--------|-------|
| F611-01: Indicator parameters | ❌ | Not implemented |
| F611-02: Indicator variables | ❌ | Not implemented |

### F641: Row and Table Constructors

| Feature | Status | Notes |
|---------|--------|-------|
| F641-01: ROW constructor | ✅ | Full support |
| F641-02: Table value constructor | ✅ | VALUES clause |
| F641-03: Multiple rows in VALUES | ✅ | Full support |

### F690: Collation Support

| Feature | Status | Notes |
|---------|--------|-------|
| F690-01: COLLATE clause | 🔶 | Basic support |
| F690-02: COLLATION_CATALOG | ❌ | Not implemented |
| F690-03: COLLATION_SCHEMA | ❌ | Not implemented |
| F690-04: COLLATION_NAME | ❌ | Not implemented |

### F721: Deferrable Constraints

| Feature | Status | Notes |
|---------|--------|-------|
| F721-01: INITIALLY DEFERRED | ❌ | Not implemented |
| F721-02: INITIALLY IMMEDIATE | ❌ | Not implemented |
| F721-03: SET CONSTRAINTS | ❌ | Not implemented |

### F731: INSERT Column Privileges

| Feature | Status | Notes |
|---------|--------|-------|
| F731-01: INSERT column privileges | ❌ | Not implemented |
| F731-02: UPDATE column privileges | ❌ | Not implemented |
| F731-03: REFERENCES column privileges | ❌ | Not implemented |

### F761: Session Management

| Feature | Status | Notes |
|---------|--------|-------|
| F761-01: SET SESSION CHARACTERISTICS | 🔶 | Basic support |
| F761-02: SET TRANSACTION | 🔶 | Basic support |
| F761-03: CURRENT_USER | ✅ | Full support |
| F761-04: SESSION_USER | ✅ | Full support |
| F761-05: SYSTEM_USER | ✅ | Full support |

### F771: Connection Management

| Feature | Status | Notes |
|---------|--------|-------|
| F771-01: CONNECT | ❌ | Not implemented |
| F771-02: DISCONNECT | ❌ | Not implemented |
| F771-03: SET CONNECTION | ❌ | Not implemented |

### F781: Self-Referencing Operations

| Feature | Status | Notes |
|---------|--------|-------|
| F781-01: Self-referencing UPDATE | ✅ | Full support |
| F781-02: Self-referencing DELETE | ✅ | Full support |
| F781-03: Self-referencing INSERT | ✅ | Full support |

### F791: Insensitive Cursors

| Feature | Status | Notes |
|---------|--------|-------|
| F791-01: INSENSITIVE cursors | ❌ | Not implemented |

### F801: Full Set Function

| Feature | Status | Notes |
|---------|--------|-------|
| F801-01: EVERY | ❌ | Not implemented |
| F801-02: ANY | 🔶 | Basic support |
| F801-03: SOME | 🔶 | Basic support |

### T121: WITH (excluding RECURSIVE)

| Feature | Status | Notes |
|---------|--------|-------|
| T121-01: WITH clause | ✅ | Full support |
| T121-02: Multiple WITH clauses | ✅ | Full support |
| T121-03: WITH in subqueries | ✅ | Full support |
| T121-04: WITH in views | ✅ | Full support |
| T121-06: WITH in INSERT | ✅ | Full support |
| T121-07: WITH in UPDATE | ✅ | Full support |
| T121-08: WITH in DELETE | ✅ | Full support |

### T131: Recursive Query

| Feature | Status | Notes |
|---------|--------|-------|
| T131-01: RECURSIVE in WITH | ✅ | Full support |
| T131-02: Recursive UNION | ✅ | Full support |
| T131-03: Multiple recursive terms | 🔶 | Basic support |

### T321: Basic SQL-invoked Routines

| Feature | Status | Notes |
|---------|--------|-------|
| T321-01: User-defined functions | 🔶 | Parsing only |
| T321-02: User-defined procedures | ❌ | Parser expects FUNCTION token |
| T321-03: CALL statement | ✅ | Full support |
| T321-04: RETURN statement | ❌ | Not implemented |
| T321-05: FUNCTION invocation | 🔶 | Basic support |

### T611: Elementary OLAP Operations

| Feature | Status | Notes |
|---------|--------|-------|
| T611-01: RANK | ✅ | Full support |
| T611-02: DENSE_RANK | ✅ | Full support |
| T611-03: PERCENT_RANK | ✅ | Full support |
| T611-04: CUME_DIST | ✅ | Full support |
| T611-05: ROW_NUMBER | ✅ | Full support |

### T612: Advanced OLAP Operations

| Feature | Status | Notes |
|---------|--------|-------|
| T612-01: NTILE | 🔶 | Parsing only |
| T612-02: LAG | 🔶 | Parsing only |
| T612-03: LEAD | 🔶 | Parsing only |
| T612-04: FIRST_VALUE | 🔶 | Parsing only |
| T612-05: LAST_VALUE | 🔶 | Parsing only |
| T612-06: NTH_VALUE | 🔶 | Parsing only |

---

## Implementation Status

### Overall Compliance

| Category | Coverage | Notes |
|----------|----------|-------|
| Core SQL (E-features) | ~85% | Most mandatory features |
| Basic Schema (F-features) | ~75% | Core DDL/DML |
| Transactions (T-features) | ~70% | Basic transactions, CTEs |
| Optional Features | ~60% | Selective implementation |
| Cursors | ~90% | Full parsing support |
| Routines | ~20% | Parsing only |

### SQL Standard Versions

| Version | Compliance | Notes |
|---------|------------|-------|
| SQL-86 | ✅ | Full compliance |
| SQL-89 | ✅ | Full compliance |
| SQL-92 (Entry) | ✅ | Full compliance |
| SQL-92 (Intermediate) | 🔶 | Partial compliance |
| SQL-92 (Full) | 🔶 | Partial compliance |
| SQL:1999 | 🔶 | Core features |
| SQL:2003 | 🔶 | Window functions partial |
| SQL:2008 | 🔶 | MERGE support |
| SQL:2011 | 🔶 | Temporal features partial |
| SQL:2016 | 🔶 | JSON support |
| SQL:2023 | 🔶 | Recent features partial |

### Priority Roadmap

**High Priority** (Core SQL):
1. ✅ Basic data types
2. ✅ Basic queries (SELECT, INSERT, UPDATE, DELETE)
3. ✅ Joins and subqueries
4. ✅ Transactions
5. ✅ Constraints

**Medium Priority** (Optional Features):
1. ✅ CTEs (WITH clause)
2. 🔶 Window functions
3. ✅ Cursors (Parsing)
4. 🔶 User-defined functions
5. ❌ Advanced OLAP

**Low Priority**:
1. ❌ Connection management
2. ❌ Deferrable constraints
3. ❌ Advanced privileges
4. ❌ Conformance views

---

## Known Limitations

1. **Cursors**: Full parsing support (DECLARE/FETCH/CLOSE/MOVE)
2. **Stored Procedures**: CALL supported, but CREATE PROCEDURE syntax has issues
3. **Advanced OLAP**: Limited window function support
4. **Connection Management**: Not implemented
5. **Deferrable Constraints**: Not supported
6. **Advanced Privileges**: Column-level privileges not implemented
7. **Conformance Views**: SQL_FEATURES views not available
8. **Indicator Types**: Not supported
9. **Multiple Connections**: Not supported
10. **Advanced Collation**: Limited collation support

---

## References

- [ISO/IEC 9075:2023 SQL Standard](https://www.iso.org/standard/76583.html)
- [SQL:2023 Feature List](https://www.wiscorp.com/sql-standard-features.html)
- [PostgreSQL SQL Conformance](https://www.postgresql.org/docs/current/features.html)
- [MySQL SQL Standard Compliance](https://dev.mysql.com/doc/refman/8.0/en/compatibility.html)
