# Entertainment & Media Streaming Examples

This directory contains Orbit-RS examples for the Entertainment and Media industry, modeling operations for studios and streaming providers like Netflix, HBO, Disney+, and Amazon Prime.

## Overview

The examples demonstrate a modern, polyglot data architecture required to run a global entertainment platform.

### Scenarios Covered

1.  **Studio Production Management** (SQL)
    - Managing production budgets, schedules, and talent contracts.
    - Relational data integrity for financial and legal records.

2.  **Content Management System (CMS)** (MongoDB)
    - Flexible metadata for Movies, TV Series, and Episodes.
    - Managing multi-region assets (subtitles, audio tracks, localized art).

3.  **Streaming & Session Platform** (Redis)
    - High-speed session management.
    - Real-time "Continue Watching" bookmarks.
    - Caching trending content.

4.  **Content Knowledge Graph** (Cypher)
    - Recommendation engine relationships.
    - Linking Actors, Directors, Genres, and Franchises.

## Directory Structure

- `sql/`: Production and Rights Management schemas.
- `mongodb/`: Content Catalog and Asset schemas.
- `redis/`: User Session and Cache operations.
- `cypher/`: Knowledge Graph queries and schema.
- `workflows/`: End-to-end business process documentation.

## Running the Examples

Each subdirectory contains specific instructions and files that can be run against an Orbit-RS instance.

```bash
# Example: Run SQL schema
orbit-client run -f entertainment/sql/01_studio_production.sql
```
