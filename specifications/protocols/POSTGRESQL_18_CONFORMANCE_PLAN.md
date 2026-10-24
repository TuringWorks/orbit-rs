# PostgreSQL 18 Conformance Plan

**Status**: in progress · **Started**: 2026-10-23 · **Owner**: Ravindra

This plan replaces "full PostgreSQL 18 compatibility" with a *measured, defined* target. Coverage
is reported as pass rates from a corpus that real PostgreSQL 18 also passes, not as an estimate.
The older feature inventory, [`POSTGRESQL_18_COMPATIBILITY.md`](POSTGRESQL_18_COMPATIBILITY.md),
remains the checklist of what exists; its coverage percentage is superseded by this document.

## Position

Full PostgreSQL 18 parity is not a goal. CockroachDB (a Go reimplementation, 10+ years) still
lacks extensions and full PL/pgSQL; Yugabyte reuses real Postgres code and trails upstream by
years. Orbit's target is:

> Drivers, ORMs, `psql`, and standard tooling work, and every behaviour we claim is verified
> against real PostgreSQL 18.

Out of scope: C-ABI extensions (PostGIS, etc.), foreign data wrappers, logical-replication wire
compatibility, `pg_upgrade`, bit-for-bit `EXPLAIN` output.

## Audit (2026-10-23)

| Area | State |
|---|---|
| Conformance harness | **Built** (this change): `tests/conformance/`, `make conformance` |
| Grammar | Hand-rolled, ~21k lines (`sql/parser/`). **Now switchable** with PostgreSQL 18's own grammar |
| `pg_catalog` / `information_schema` | Started: `sql/durable/pg_catalog.rs`, `information_schema.rs` |
| SQLSTATE | Mostly `XX000` (internal error) on the legacy path; the durable path emits real codes in places (e.g. `42703`) |
| Distribution | One Raft group replicating the whole SQL store (`raft/sql.rs`). **Not sharded.** No HLC, no cross-shard SQL transactions, no distributed planning, no online schema change |
| Verification of distribution | `raft/sim.rs` covers Raft; nothing checks SQL isolation under faults (Jepsen/Elle-style) |
| PG18-specific features | Version string agrees everywhere (`postgres_version_tests.rs`); the rest is tracked by `07_postgres18.slt` |

The "~85%" in the older spec counted parsing as implemented (many rows read "Parsing complete" or
"no execution"). Do not quote it.

## Parser: choice and switch

Rust options for PostgreSQL's real grammar:

| Option | Rust | PG version | Verdict |
|---|---|---|---|
| `pg_query` 6.2.1 on crates.io | yes (wraps C `libpg_query`) | **17** | Not 18 |
| `pg_query.rs` branch `pg-18` (PR #79 "Release 18.0.0", unreleased) | yes | **18** | **Used**, pinned by commit |
| `libpg_query` 18.0.0 / 18.1.0 | C library | 18 | What the above wraps |
| `pg_parse` 0.15 | yes (older JSON interface) | not verified | Not chosen |
| `sqlparser` 0.63 | pure Rust | none (own grammar) | Not Postgres's grammar |

The `pg-query-parser` cargo feature on `orbit-server` pulls the `pg-18` branch at
`ad0c87ed59c8…`. Move to the crates.io release once an 18.x version is published. The 18 grammar
is verified by a unit test (`linked_libpg_query_speaks_postgres_18`) that parses `RETURNING
OLD/NEW`, virtual generated columns, and `WITHOUT OVERLAPS`.

Selection is at run time through `ORBIT_SQL_PARSER` (`sql/parser/backend.rs`):

| Value | Behaviour |
|---|---|
| `hand-rolled` (default) | The hand-rolled parser alone. |
| `shadow` | Both parse; disagreements are logged (`SQL parser disagreement`) and counted via `shadow_counts()`. The hand-rolled result is used. |
| `pg-query` | `libpg_query` gates: SQL PostgreSQL rejects is rejected. Accepted SQL is still built into Orbit's AST by the hand-rolled parser. |

**Limit, stated plainly:** `libpg_query` produces PostgreSQL's parse tree, not Orbit's `Statement`
AST, so the hand-rolled parser still builds every AST. Replacing it means lowering `pg_query`
nodes to `Statement` one statement type at a time, with the hand-rolled parser as fallback for
unlowered nodes. The disagreement report below picks the order. Without the cargo feature,
`shadow` and `pg-query` fall back to `hand-rolled` with a warning.

## Harness

`tests/conformance/slt/*.slt` is a sqllogictest corpus, run over the PostgreSQL wire protocol by
`tests/orbit-protocols/postgres_conformance.rs`. The same files run against two targets:

| Target | Role | Command |
|---|---|---|
| Orbit | **Measured.** Per-file pass count may not fall below `tests/conformance/baseline.json` | `make conformance` |
| Real PostgreSQL 18 (docker) | **Oracle.** Must pass 100%; a failure is a bug in the corpus | `make conformance-reference` |
| libpg_query vs hand-rolled parser | **Agreement report**, ceiling in `parser_disagreements_max` (19 of 533 statements disagree today) | `make conformance-parser` |

`make conformance` starts Orbit in-process wired as `main.rs` wires it (query engine over the
durable RocksDB engine). To measure a running server set `ORBIT_CONFORMANCE_URL`. Results are
written to `target/conformance/{orbit,reference,parser}.json`. Update the ratchet after an
improvement with `CONFORMANCE_UPDATE_BASELINE=1 make conformance`.

### Measurements

The corpus grew from 118 records (basics) to 533 across 17 files, recorded from real PostgreSQL 18
(`make conformance-record`), so expectations are PostgreSQL's output, not ours.

| Date | Corpus | Real PostgreSQL 18 | Orbit |
|---|---|---|---|
| 2026-10-23 | 118 records (basics) | 118 / 118 | 107 / 118 (90.7%) |
| 2026-10-23 | 535 records (+types, strings, dates, aggregates, DDL, DML, transactions, catalog, JSON/arrays, queries) | 535 / 535 | 338 / 535 (63.2%) |
| 2026-10-24 | 533 records (4 environment-dependent records removed) | 533 / 533 | **465 / 533 (87.2%)** |

The jump from 90.7% to 63.2% was the corpus becoming honest, not a regression: the first corpus
only covered what already worked. Read 87.2% as "the 533 recorded behaviours", not "87% of
PostgreSQL"; the corpus still omits large areas (see *Not yet measured*).

### Fixed by this work (each verified against the oracle)

- **Silent wrong answers**: `stddev`, `variance` and friends returned `0` for every group;
  `round(numeric, n)` dropped trailing zeros (`4` for `4.0000`); `avg(int)` printed `25` where
  PostgreSQL prints `25.0000000000000000`; `strpos(a, b)` took its arguments backwards; unary minus
  failed on any `numeric` value (`-2.5`); `jsonb` and `interval` printed in a non-PostgreSQL format.
- **SQLSTATE**: unclassified errors are no longer all `XX000`; parse errors are `42601`, bad input
  `22P02`, range `22003`, unknown function `42883`, and so on (`classify_error`).
- **Grammar**: `FROM` items (comma joins, `NATURAL`, joins to subqueries and functions, `LATERAL`,
  table functions with `WITH ORDINALITY`), `LIMIT ALL`/`OFFSET n ROWS`/`FETCH FIRST`, `FOR UPDATE`
  (all forms), `DISTINCT ON`, `SELECT ALL`, `VALUES`/`TABLE` statements, `WITH RECURSIVE`,
  `[NOT] MATERIALIZED`, `NULLS FIRST/LAST`, `^`, `~ ~* !~ !~*`, jsonb `? ?| ?&`, `= ANY(..)`,
  `> ALL (subquery)`, `BETWEEN`, `SUBSTRING/EXTRACT/TRIM/POSITION/OVERLAY` keyword forms,
  `LEFT()`/`RIGHT()`, every cast type name (`::jsonb`, `::numeric(10,2)`, `::uuid`, `int[]`).
  Subqueries and CTE bodies now use the same grammar as top-level queries.
- **Engine**: views, `CREATE TABLE AS`, `GENERATED ... AS IDENTITY` (and virtual generated
  columns), `RETURNING OLD/NEW`, `DROP CONSTRAINT`, set-returning functions in `FROM` and in the
  select list (`generate_series`, `unnest`, `json[b]_each/_array_elements/_object_keys`),
  `percentile_cont/disc`, `mode`, `json[b]_agg`, the `json[b]_*` function family, `concat_ws`,
  `format` with widths, `encode/decode` on bytea, date/interval arithmetic, `current_setting`.
- **Sessions**: real per-connection `SET/SHOW/RESET`, `SET LOCAL`, rollback of `SET`, `set_config`,
  `current_user`/`current_database()` from the startup message, `READ ONLY` transactions,
  `SHOW transaction_isolation` (`sql/durable/session_vars.rs`).

### Not yet working (the remaining 68 records)

| Theme | What is missing |
|---|---|
| Sequences, schemas | `CREATE SEQUENCE` / `nextval` / `currval` / `setval` as objects (only `SERIAL`/`IDENTITY` exist); `CREATE/DROP SCHEMA` live outside the durable engine |
| Date and time | `age()`, `make_time`, `to_char`/`to_date`/`to_timestamp` formats, `AT TIME ZONE`, `OVERLAPS`, `infinity`, `justify_*`, `time '..'`/`timestamptz '..'` literals, `timestamptz` printing |
| Grouping | `ROLLUP`, `CUBE`, `GROUPING SETS`; the named `WINDOW w AS (..)` clause |
| Statements | data-modifying CTEs (`WITH x AS (DELETE .. RETURNING ..)`), SQL-level `PREPARE`/`EXECUTE`, `EXPLAIN`, `SELECT .. INTO`, `TABLESAMPLE`, `E'..'` strings |
| Strictness | Orbit accepts SQL PostgreSQL rejects: ambiguous column (`42702`), `GROUP BY` validation (`42803`), unknown type in DDL (`42704`), `LOCK TABLE` outside a transaction (`25P01`), `USING` on a column that does not exist on both sides |
| Introspection | `NOT NULL`/unique constraints in `pg_constraint` and `information_schema.table_constraints`; `ON DELETE` in `pg_get_constraintdef`; `pg_tables`, `pg_indexes`, `pg_proc`; `COMMENT ON` for tables and columns; `regclass` printing |
| JSON and arrays | `@>` where the literal is untyped (`doc @> '{"a":1}'`, needs implicit typing of unknown literals); `jsonb_path_query` / `@?`; `(ARRAY[..])[i]` and nested `ARRAY[[..]]`; several set-returning functions in one select list (lockstep) |
| Values | text ordering follows byte order, not PostgreSQL's locale collation (`'a' < 'B'`); `power(numeric, numeric)` is computed as a float |

### Not yet measured

Corpus areas that do not exist yet, so their pass rate is unknown rather than good: the extended
query protocol (Parse/Bind/Execute, binary formats, pipelining), `COPY`, prepared-statement
behaviour of real drivers, ORM traffic, `pg_dump`/`pg_restore`, `psql \d*`, concurrency and
isolation anomalies, large result sets, and everything distributed.

### Known weaknesses of the harness

- `statement error (SQLSTATE)` asserts the code, but only where the oracle returned a code Orbit
  also classifies; a statement can still pass for the wrong reason when the code is common (`42601`).
- A failed setup statement cascades into every later record in its file, inflating the failure
  count for that file.
- Records count equally; common features and rare ones weigh the same.
- Results come from PostgreSQL 18's default settings in a container (UTC, `en_US.utf8`); records
  that depended on the container (patch version, time-zone name) were removed rather than faked.

## Roadmap

1. **Grow the corpus** toward real coverage: import a `pg_regress` subset, SQLAlchemy / Prisma /
   JDBC / Django / Rails introspection queries, and `psql \d*` and `pg_dump` round-trips. Every
   file must pass 100% on the oracle before it is added.
2. **Assert SQLSTATE** in `statement error` records and fix the `XX000` default.
3. **Lower `pg_query` nodes to the Orbit AST**, expression and `SELECT` first (the report's two
   gaps are here), then DML, then DDL. Flip the default to `pg-query` when the disagreement
   ceiling reaches 0 on a corpus of real tool traffic.
4. **Catalogs**: make `pg_catalog` / `information_schema` complete enough that `psql \d` and ORM
   introspection run unmodified.
5. **Semantics**: casts, numeric, timestamptz, collation, NULL ordering, error codes.
6. **Distribution** is a separate decision. Today Orbit is a replicated store (one Raft group).
   Sharding needs HLC, cross-shard transactions, distributed planning and online schema change,
   and must be verified under faults before any distributed-SQL claim.
7. **PG18 features**: `uuidv7()`, `RETURNING OLD/NEW`, virtual generated
   columns, `WITHOUT OVERLAPS`, OAuth. Tracked by `07_postgres18.slt`. PRD.md marks protocol
   v3/v3.2 negotiation "Complete"; no corpus record exercises it yet, so treat it as unverified.

## Using the parser report to drive work

```bash
make conformance-parser                       # which statements the hand-rolled parser gets wrong
ORBIT_SQL_PARSER=shadow orbit-server ...      # same, on live traffic (needs --features pg-query-parser)
```
