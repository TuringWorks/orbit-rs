# AdTech Industry Examples

This directory contains examples demonstrating Orbit-RS in the Advertising Technology (AdTech) industry, covering the complete lifecycle from campaign management to real-time bidding (RTB) and identify resolution.

## Scenarios

### 1. Buy-Side & Sell-Side Management (SQL)
- **File**: `sql/01_campaign_inventory.sql`
- **Description**: Relational schema for managing Advertisers, Campaigns, and Creative Assets (Buy Side) alongside Publishers, Sites, and Ad Units (Sell Side).
- **Features**: UUID keys, JSONB for targeting, and aggregated reporting tables.

### 2. Real-Time Bidding (RTB) Engine (Redis)
- **File**: `redis/01_realtime_bidding.redis`
- **Description**: High-performance caching and logic for the auction engine, capable of sub-millisecond responses.
- **Features**: 
    - **Frequency Capping**: Limiting ad exposure per user.
    - **Budget Pacing**: Real-time token bucket algorithms.
    - **Geo Targeting**: Spatial indexing for local campaigns.

### 3. Data Management Platform (DMP) (MongoDB)
- **File**: `mongodb/01_dmp_profiles.js`
- **Description**: Storing rich, flexible user profiles with behavioral data and audience segments.
- **Features**: Nested documents for activity streams, sparse attribute storage, and real-time segment qualification.

### 4. Identity Graph (Cypher)
- **File**: `cypher/01_identity_graph.cypher`
- **Description**: Graph model for resolving user identities across disparate keys (Cookies, Device IDs, Emails).
- **Features**: Deterministic and probabilistic linking, finding "household" devices for retargeting.

## Workflows

### 01_rtb_auction
- **File**: `workflows/01_rtb_auction.md`
- **Description**: End-to-end flow of a single Ad Request, traversing the entire stack from identity resolution to the final bid and impression logging.
