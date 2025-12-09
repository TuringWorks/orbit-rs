# Workflow: Content Ingestion Pipeline

This workflow describes the process of ingesting new production content into the streaming platform, ensuring it is available for users and discoverable via search.

## Overview

1.  **Greenlight & Production** (SQL)
    - A production is marked `Completed` in the SQL `productions` table.
    - Distribution rights are verified in `distribution_rights`.

2.  **Metadata Ingestion** (MongoDB)
    - Marketing team inputs rich metadata (synopsis, cast) into MongoDB `content_catalog`.
    - Localized assets (posters, subs) are registered.

3.  **Graph Update** (Cypher)
    - Nodes for new `Movie`, `Actor`, and `Director` entities are created in the Knowledge Graph.
    - Relationships (`ACTED_IN`, `IN_GENRE`) are established to power recommendations.

4.  **Cache Warming** (Redis)
    - If the release is high-profile, pre-warm the Redis cache with its metadata to handle high traffic on launch day.

## Step-by-Step Execution

### Step 1: Verify Rights (SQL)
Run this query to ensure we have rights to distribute in the US.
```sql
SELECT * FROM distribution_rights 
WHERE production_id = 123 
AND region_code = 'US' 
AND CURRENT_DATE BETWEEN license_start AND license_end;
```

### Step 2: Ingest Metadata (MongoDB)
Insert the document found in `entertainment/mongodb/01_content_catalog.js`.

### Step 3: Link Entities (Cypher)
Run the cypher creation script to link the new movie to its cast and genre.

### Step 4: Publish
The content is now "Live". The Application Layer would flip a visibility flag (e.g., in Redis or Mongo).
