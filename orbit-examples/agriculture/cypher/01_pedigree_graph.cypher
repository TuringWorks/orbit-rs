// Livestock Pedigree & Lineage (Cypher)
// Managing genetic history (Sire/Dam) for breeding programs.

// 1. Create Animals
CREATE (a1:Animal {tag: 'TAG-001', name: 'Bessie', gender: 'F'})
CREATE (a2:Animal {tag: 'TAG-002', name: 'Bully', gender: 'M'})
CREATE (sire1:Animal {tag: 'TAG-999', name: 'Grand Champ', gender: 'M'})
CREATE (dam1:Animal {tag: 'TAG-888', name: 'Lady May', gender: 'F'})

// 2. Define Lineage
// a1 is offspring of sire1 and dam1
MATCH (child:Animal {tag: 'TAG-001'}), (father:Animal {tag: 'TAG-999'})
CREATE (father)-[:SIRED]->(child)

MATCH (child:Animal {tag: 'TAG-001'}), (mother:Animal {tag: 'TAG-888'})
CREATE (mother)-[:DAM_OF]->(child)

// 3. Query: Ancestry Tracing
// Find all ancestors of Bessie to check for genetic defects or traits
MATCH (a:Animal {tag: 'TAG-001'})<-[:SIRED|DAM_OF*1..5]-(ancestor:Animal)
RETURN ancestor.name, ancestor.tag, labels(ancestor)

// 4. Query: Inbreeding Check
// Find common ancestors between two potential mates (Bessie and Bully)
MATCH (m1:Animal {name: 'Bessie'})<-[:SIRED|DAM_OF*1..5]-(ancestor:Animal)
MATCH (m2:Animal {name: 'Bully'})<-[:SIRED|DAM_OF*1..5]-(ancestor)
RETURN ancestor.name AS CommonAncestor
