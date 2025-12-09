# Gaming Workflow: Matchmaking & Post-Game Processing

This workflow demonstrates how multiple protocols interact during a standard game loop (finding a match -> playing -> result processing).

## Flow Description

1. **Matchmaking Request** (Redis)
   - Player initiates search.
   - Server adds player to Redis matchmaking queue (List/Stream).
   - Server checks Redis for suitable opponents based on ELO (Sorted Set).

2. **Match Start** (Redis & SQL)
   - Server creates a temporary match session in Redis (`session:match_123`).
   - Server validates player inventory in PostgreSQL (e.g., checking if they have the required entry ticket).

3. **Game Play** (UDP/TCP - Game Server)
   - Game logic runs on dedicated game servers (external to Orbit, but using Orbit for state).
   - Real-time state updates (HP, Position) might go to Redis if persistence is needed.

4. **Match End** (Transaction across Protocols)
   - **PostgreSQL**: Update player currency (rewards) and XP.
   - **Redis**: Update ELO rating in the Leaderboard ZSET.
   - **MongoDB**: Archive full match replay/log for analytics and anti-cheat analysis.
   - **Cypher**: Update "Played With" edges to suggest future parties.

## Step-by-Step Code Example

### 1. Matchmaking Queue (Redis)
```bash
# Player joins queue
RPUSH queue:moba:unranked "player_id:1001"
```

### 2. Post-Game Processing (Pseudo-code)

```python
def process_match_result(match_data):
    # 1. Archive Match (MongoDB)
    mongo.db.matches.insert_one(match_data)
    
    # 2. Update Leaderboard (Redis)
    for player in match_data['winners']:
        redis.zincrby("leaderboard:season_1", 25, player['username'])
    
    # 3. Reward Currency (PostgreSQL)
    # ACID transaction ensures no dupes
    with postgres.transaction():
        pg.execute("UPDATE currencies SET gold = gold + 100 WHERE player_id = %s", (winner_id,))
    
    # 4. Social Update (Cypher - Optional Async)
    # Create PLAYED_WITH relationship
    neo4j.run("MATCH (a:Player {id: $p1}), (b:Player {id: $p2}) MERGE (a)-[:PLAYED_WITH]->(b)", p1=..., p2=...)
```
