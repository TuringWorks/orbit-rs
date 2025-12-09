# OrbitQL Examples

This directory contains examples for **OrbitQL**, Orbit-RS's native unified query language capable of querying relational, graph, document, and vector data in a single syntax.

## Examples

- **[01_schema_definition.orbitql](01_schema_definition.orbitql)**: Defining schemas for multi-model data.
- **[02_crud_operations.orbitql](02_crud_operations.orbitql)**: Basic Create, Read, Update, Delete queries.
- **[03_graph_operations.orbitql](03_graph_operations.orbitql)**: Graph traversals using `->` syntax.
- **[04_vector_operations.orbitql](04_vector_operations.orbitql)**: Vector similarity search (`ANN`) queries.
- **[05_transactions.orbitql](05_transactions_and_control_flow.orbitql)**: Transaction blocks and control flow.
- **[06_functions.orbitql](06_functions_and_operators.orbitql)**: Built-in functions, math operations, and aggregations.
- **[07_realtime.orbitql](07_realtime_and_live_queries.orbitql)**: Live query subscriptions (`SUBSCRIPTION`).

## Running Examples

You can execute these files using the `orbit-cli` tool or through language SDKs.

```bash
# Example
orbit-cli < 01_schema_definition.orbitql
```
