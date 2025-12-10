// Healthcare Use Case: Drug Interaction & Knowledge Graph
// Purpose: Prevent adverse drug events (ADEs) by checking prescriptions against a graph of known interactions.

// 1. Setup Medical Knowledge Base
CREATE (d1:Drug {name: "Warfarin", code: "RX-001"})
CREATE (d2:Drug {name: "Aspirin", code: "RX-002"})
CREATE (d3:Drug {name: "Ibuprofen", code: "RX-003"})
CREATE (ing1:Ingredient {name: "NSAID"})

// Relationships
CREATE (d2)-[:CONTAINS]->(ing1)
CREATE (d3)-[:CONTAINS]->(ing1)

// Interaction Rules (Knowledge)
CREATE (d1)-[:INTERACTS_WITH {severity: "HIGH", description: "Bleeding Risk"}]->(d2)
CREATE (d1)-[:INTERACTS_WITH {severity: "HIGH", description: "Bleeding Risk"}]->(d3)
CREATE (d1)-[:INTERACTS_WITH {severity: "MEDIUM"}]->(d1) -- Dosage check?

// 2. Patient Data
CREATE (p:Patient {id: "pat-123", name: "John Doe"})

// Active Prescriptions
CREATE (p)-[:TAKES {since: date('2023-01-01')}]->(d1)

// Allergies
CREATE (p)-[:ALLERGIC_TO {reaction: "Hives"}]->(ing1)

// ==========================================
// 3. Clinical Decision Support Queries
// ==========================================

// Scenario A: Doctor tries to prescribe Aspirin (d2) to Patient (p)
// Check 1: Drug-Drug Interactions (DDIs) with existing meds
MATCH (p:Patient {id: "pat-123"})-[:TAKES]->(existing_drug:Drug)
MATCH (new_drug:Drug {code: "RX-002"}) -- Aspirin
MATCH (new_drug)-[r:INTERACTS_WITH]-(existing_drug)
RETURN existing_drug.name as ConflictWith, r.severity, r.description;

// Result: ConflictWith="Warfarin", Severity="HIGH", "Bleeding Risk"

// Scenario B: Allergy Check
// Check 2: Does the new drug contain an ingredient the patient is allergic to?
MATCH (p:Patient {id: "pat-123"})-[:ALLERGIC_TO]->(allergen:Ingredient)
MATCH (new_drug:Drug {code: "RX-002"})
MATCH (new_drug)-[:CONTAINS]->(allergen)
RETURN new_drug.name as Drug, allergen.name as AllergicTo;

// Result: Drug="Aspirin", AllergicTo="NSAID"
