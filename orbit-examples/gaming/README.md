# Gaming Industry Examples 🎮

This directory contains end-to-end examples for a **Massively Multiplayer Online (MMO) Game** backend using Orbit-RS.

Key scenarios include player management, real-time leaderboards, match history archival, and social graph features.

## 🏗 Architecture

| Component | Protocol | Port | Usage |
|-----------|----------|------|-------|
| **Core Systems** | PostgreSQL | 5432 | Player accounts, inventory, currency (ACID) |
| **Real-Time** | Redis | 6379 | Global leaderboards, session caching, presence |
| **History** | MongoDB | 27017 | Match replays, game events log (flexible schema) |
| **Social** | Cypher (Bolt)| 7687 | Friend graph, guild memberships, party recommendations |

## 🚀 Running the Examples

### 1. Player Accounts (PostgreSQL)
Create the core schema for players and items.

```bash
psql -h localhost -p 5432 -U orbit -d postgres -f sql/01_player_schema.sql
```

### 2. Leaderboards (Redis)
Simulate real-time score updates and rank retrieval.

```bash
redis-cli -h localhost -p 6379 < redis/01_leaderboard_ops.redis
```

### 3. Match History (MongoDB)
Archive complex match results for analytics.

```bash
mongosh mongodb://localhost:27017 --file mongodb/01_match_history.js
```

### 4. Social Graph (Cypher)
Manage friend requests and query "friends of friends".

```bash
cypher-shell -a bolt://localhost:7687 -u orbit -p orbit -f cypher/01_social_graph.cypher
```

## 📚 Workflows

- **[Matchmaking & Post-Game Flow](workflows/01_matchmaking_flow.md)**: How data flows across protocols during a match lifecycle.
