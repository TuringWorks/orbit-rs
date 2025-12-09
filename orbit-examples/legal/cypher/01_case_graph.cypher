// Legal Citation Graph (Cypher)
// Analyzing relationships between cases to find precedents and overrulings.

// 1. Create Cases
CREATE (c1:Case {citation: '347 U.S. 483', name: 'Brown v. Board of Education', year: 1954})
CREATE (c2:Case {citation: '163 U.S. 537', name: 'Plessy v. Ferguson', year: 1896})
CREATE (c3:Case {citation: '551 U.S. 701', name: 'Parents Involved v. Seattle', year: 2007});

// 2. Create Relationships
// Brown v. Board OVERRULED Plessy v. Ferguson
MATCH (a:Case {name: 'Brown v. Board of Education'}), (b:Case {name: 'Plessy v. Ferguson'})
CREATE (a)-[:OVERRULED]->(b);

// Parents Involved CITED Brown v. Board
MATCH (a:Case {name: 'Parents Involved v. Seattle'}), (b:Case {name: 'Brown v. Board of Education'})
CREATE (a)-[:CITED]->(b);


// 3. Query: Find "Good Law" vs "Bad Law"
// Find cases that have been overruled
MATCH (c:Case)<-[:OVERRULED]-(overruler:Case)
RETURN c.name AS OverruledCase, overruler.name AS OverruledBy;

// 4. Query: Influence Analysis
// Find how many cases cite 'Brown v. Board' (Degree Centrality)
MATCH (c:Case)-[:CITED]->(target:Case {name: 'Brown v. Board of Education'})
RETURN count(c) as CitationCount;
