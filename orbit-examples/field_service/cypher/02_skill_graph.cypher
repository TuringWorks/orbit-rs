// ============================================================================
// OrbitRS Field Service Examples - Skill Graph (Cypher)
// ============================================================================
// Graph database for skills, certifications, and proximity
// ============================================================================

// 1. Create Nodes (Technicians, Skills, Locations)
CREATE (t1:Technician {id: 1, name: 'Mario', level: 'Master'})
CREATE (t2:Technician {id: 2, name: 'Luigi', level: 'Journeyman'})
CREATE (t3:Technician {id: 3, name: 'Wario', level: 'Apprentice'})

CREATE (s1:Skill {name: 'Plumbing'})
CREATE (s2:Skill {name: 'HVAC'})
CREATE (s3:Skill {name: 'Electrical'})

CREATE (c1:Certification {name: 'Master Plumber License'})
CREATE (c2:Certification {name: 'EPA 608 Universal'})

CREATE (l1:Location {zip: '90210', city: 'Beverly Hills'})
CREATE (l2:Location {zip: '90001', city: 'Los Angeles'})

// 2. Create Relationships
// Who knows what?
CREATE (t1)-[:KNOWS {years: 15}]->(s1)
CREATE (t1)-[:HAS_CERT]->(c1)
CREATE (t2)-[:KNOWS {years: 10}]->(s1)
CREATE (t3)-[:KNOWS {years: 2}]->(s2) -- Wario does HVAC

// Who lives where?
CREATE (t1)-[:LIVES_IN]->(l1)
CREATE (t2)-[:LIVES_IN]->(l1)
CREATE (t3)-[:LIVES_IN]->(l2)

// Locations near each other (Proximity Graph)
CREATE (l1)-[:NEAR {miles: 5}]->(l2)

// 3. Query: Find a Master Plumber near 90210
MATCH (t:Technician)-[:HAS_CERT]->(c:Certification {name: 'Master Plumber License'})
MATCH (t)-[:LIVES_IN]->(l:Location {zip: '90210'})
RETURN t.name, c.name, l.city

// 4. Query: Find technicians who know 'Plumbing' and can service '90001' 
// (Either live there OR live in a NEAR location)
MATCH (t:Technician)-[k:KNOWS]->(s:Skill {name: 'Plumbing'})
WHERE k.years > 5
MATCH (t)-[:LIVES_IN]->(home:Location)
OPTIONAL MATCH (home)-[n:NEAR]->(job_loc:Location {zip: '90001'})
WHERE home.zip = '90001' OR n.miles <= 10
RETURN t.name, t.level, home.zip

// 5. Query: Identifying Skill Gaps (Skills with no Certified Techs)
MATCH (s:Skill)
WHERE NOT (s)<-[:KNOWS]-(:Technician)-[:HAS_CERT]->(:Certification)
RETURN s.name as Uncertified_Skill
