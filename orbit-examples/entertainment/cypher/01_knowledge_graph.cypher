// Cypher: Entertainment Knowledge Graph
// Used for recommendations: "Users who watched X also watched Y" or "More movies with Actor Z".

// 1. Create Nodes (Movies, Actors, Directors, Genres, Users)
CREATE (m1:Movie {title: 'Interstellar Drift', id: 'mv_88392', year: 2024})
CREATE (m2:Movie {title: 'Space Odyssey 2050', id: 'mv_99100', year: 2023})
CREATE (p1:Person {name: 'Sarah Connor'})
CREATE (p2:Person {name: 'John Doe'})
CREATE (p3:Person {name: 'Chris Nolan_ish'})
CREATE (g1:Genre {name: 'Sci-Fi'})
CREATE (g2:Genre {name: 'Adventure'})
CREATE (u1:User {id: '1001', name: 'Alice'})

// 2. Create Relationships (ACTED_IN, DIRECTED, IN_GENRE, WATCHED)
CREATE (p1)-[:ACTED_IN {role: 'Commander'}]->(m1)
CREATE (p2)-[:ACTED_IN {role: 'Navigator'}]->(m1)
CREATE (p2)-[:ACTED_IN {role: 'Pilot'}]->(m2)
CREATE (p3)-[:DIRECTED]->(m1)
CREATE (p3)-[:DIRECTED]->(m2)
CREATE (m1)-[:IN_GENRE]->(g1)
CREATE (m1)-[:IN_GENRE]->(g2)
CREATE (m2)-[:IN_GENRE]->(g1)

// User Interactions
CREATE (u1)-[:WATCHED {rating: 5}]->(m2)

// 3. Recommendation Query: Collaborative Filtering
// "Find movies that other users watched who also watched the movie I just finished."
// (Simplified version: Find other movies in the same genre starring the same actor)
MATCH (m1:Movie {title: 'Interstellar Drift'})-[:IN_GENRE]->(g:Genre)<-[:IN_GENRE]-(rec:Movie)
MATCH (m1)<-[:ACTED_IN]-(a:Person)-[:ACTED_IN]->(rec)
WHERE m1 <> rec
RETURN rec.title, a.name, g.name

// 4. Recommendation Query: "Because you watched..."
// Transitive relationship: User -> Watched -> Movie -> DirectedBy -> Director -> Directed -> OtherMovie
MATCH (u:User {id: '1001'})-[:WATCHED]->(watched:Movie)<-[:DIRECTED]-(d:Person)-[:DIRECTED]->(suggestion:Movie)
WHERE NOT (u)-[:WATCHED]->(suggestion)
RETURN suggestion.title, d.name

// 5. Shortest Path: Bacon Number style
// How is 'Sarah Connor' connected to 'Chris Nolan_ish'?
MATCH path = shortestPath((p1:Person {name: 'Sarah Connor'})-[*]-(p2:Person {name: 'Chris Nolan_ish'}))
RETURN path
