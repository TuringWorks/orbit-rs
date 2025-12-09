// Gaming Example: Match History Archival
// MongoDB is ideal for storing varying schema of game matches (different modes, player counts, event logs).

db = db.getSiblingDB('gaming_history');

// 1. Insert a MOBA Match Result
db.matches.insertOne({
    match_id: "m_1001",
    game_mode: "MOBA_5v5",
    region: "NA-East",
    duration_seconds: 1845,
    timestamp: new Date(),
    teams: {
        radiant: {
            win: true,
            players: [
                { id: 1, hero: "Paladin", kda: [5, 0, 12], items: ["shield", "boots"] },
                { id: 2, hero: "Mage", kda: [10, 2, 8], items: ["staff", "wd"] }
            ]
        },
        dire: {
            win: false,
            players: [
                { id: 3, hero: "Assassin", kda: [1, 5, 2], items: ["dagger"] },
                { id: 4, hero: "Warrior", kda: [0, 4, 1], items: ["axe"] }
            ]
        }
    },
    performance_metrics: {
        server_tick_rate: 64,
        avg_latency_ms: 25
    }
});

// 2. Insert a Battle Royale Match Result (Completely different structure)
db.matches.insertOne({
    match_id: "br_5050",
    game_mode: "BattleRoyale_Solo",
    region: "EU-West",
    duration_seconds: 900,
    timestamp: new Date(),
    winner_player_id: 42,
    total_players: 100,
    events: [
        { time: 60, event: "circle_shrink" },
        { time: 120, event: "airdrop", loc: [100, 200] }
    ]
});

// 3. Query: Find all matches won by a specific player (checking deep structure)
print("Matches won by Player 1 (Radiant team):");
cursor = db.matches.find({
    "teams.radiant.players.id": 1,
    "teams.radiant.win": true
});
while (cursor.hasNext()) {
    printjson(cursor.next());
}

// 4. Aggregation: Average match duration by game mode
print("Avg Duration per Mode:");
cursor = db.matches.aggregate([
    {
        $group: {
            _id: "$game_mode",
            avgDuration: { $avg: "$duration_seconds" },
            totalMatches: { $sum: 1 }
        }
    }
]);
while (cursor.hasNext()) {
    printjson(cursor.next());
}
