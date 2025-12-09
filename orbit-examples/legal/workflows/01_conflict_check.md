# Legal Workflow: Conflict Check & Intake

Before a law firm accepts a new client, they must ensure no conflict of interest exists (e.g., representing a plaintiff against an existing client).

## Flow Description

1. **Intake Request**
   - New potential client: "Roadrunner Inc".
   - Adverse party: "Acme Corp".

2. **Conflict Search (Graph + SQL)**
   - **SQL**: Check if "Acme Corp" is a current client in `clients` table.
   - **Graph (Cypher)**: Check if "Acme Corp" is related to any existing clients (e.g., Subsidiary, Officer).
   - *Example*: `MATCH (c:Client)-[:OWNS]->(s:Company {name: 'Acme Corp'}) RETURN c`

3. **Result**
   - If SQL returns "Acme Corp" is a current client -> **CONFLICT DETECTED**.
   - If Graph returns relationship -> **POTENTIAL CONFLICT**.

4. **Matter Opening**
   - If cleared, create Client and Matter records in PostgreSQL.
   - Initialize Document Collection in ArangoDB for discovery.

## Step-by-Step Code Example

### 1. SQL Check
```sql
SELECT status FROM matters 
JOIN clients ON matters.client_id = clients.client_id
WHERE clients.name = 'Acme Corp';
-- Returns 'OPEN' -> Conflict!
```

### 2. Graph Check (Pseudo-Cypher)
```cypher
// Check if existing clients are related to Roadrunner Inc
MATCH (existing:Client)-[:RELATED_TO*1..2]-(new:Entity {name: 'Roadrunner Inc'})
RETURN existing.name, relationship(existing, new)
```
