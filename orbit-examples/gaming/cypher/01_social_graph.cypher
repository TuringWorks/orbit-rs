// Gaming Social Graph (Cypher)
// Managing friends, guilds, parties, and recommendations.

// 1. Create Players and Guilds
CREATE (p1:Player {username: 'Slayer99', level: 50})
CREATE (p2:Player {username: 'HealerPro', level: 48})
CREATE (p3:Player {username: 'TankMaster', level: 50})
CREATE (p4:Player {username: 'RogueOne', level: 20})
CREATE (g1:Guild {name: 'OrbitGuardians', tag: '[ORBIT]'});

// 2. Create Friendships
MATCH (a:Player), (b:Player) WHERE a.username = 'Slayer99' AND b.username = 'HealerPro'
CREATE (a)-[:FRIEND {since: date('2023-01-01')}]->(b);

MATCH (a:Player), (b:Player) WHERE a.username = 'HealerPro' AND b.username = 'TankMaster'
CREATE (a)-[:FRIEND {since: date('2023-02-15')}]->(b);

// 3. Join Guilds
MATCH (p:Player), (g:Guild) WHERE p.username IN ['Slayer99', 'HealerPro', 'TankMaster'] AND g.name = 'OrbitGuardians'
CREATE (p)-[:MEMBER_OF {rank: 'Member'}]->(g);

// 4. Query: Find Friends of Friends (Friend Recommendation)
// Suggest friends for Slayer99 who are friends with his friends, but not yet friends with him
MATCH (me:Player {username: 'Slayer99'})-[:FRIEND]->(friend)-[:FRIEND]->(fof)
WHERE NOT (me)-[:FRIEND]->(fof) AND me <> fof
RETURN fof.username AS RecommendedFriend, count(friend) AS MutualFriends;

// 5. Query: Find Guild Members suitable for a High-Level Raid (Level >= 50)
MATCH (g:Guild {name: 'OrbitGuardians'})<-[:MEMBER_OF]-(p:Player)
WHERE p.level >= 50
RETURN p.username, p.level;
