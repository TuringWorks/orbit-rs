# Workflow: Real-Time Bidding (RTB) Auction

## Overview
This workflow describes the lifecyle of a single Ad Request, from the user visiting a page to the ad being served. This entire process must happen in under **100 milliseconds**.

## Workflow Steps

### 1. Ad Request (Sell Side)
**Actor**: User's Browser / SSP (Supply Side Platform)  
**Input**: URL, IP Address, User Agent, Cookie ID.
**Action**: SSP sends a Bid Request to the Orbit Ad Server.

### 2. Identity Resolution (Cypher & Redis)
**System**: Orbit Graph Engine  
**Goal**: Match the anonymous Cookie ID to a rich User Profile.

-   **Fast Path**: Check Redis cache for `cookie_id -> user_id` mapping.
-   **Slow Path (Async)**: If new, ingest into Cypher Identity Graph for future linking.

### 3. Profile Lookup (MongoDB)
**System**: MongoDB (Orbit Document Engine)  
**Action**: Retrieve user segments for targeting.
-   *Input*: `user_id` or `cookie_id`
-   *Output*: `["Auto_Intender", "Male_25-34", "NY_Metro"]`

### 4. Auction & Targeting (Redis + In-Memory)
**System**: Redis (Orbit Protocol)  
**Goal**: Pick the winning campaign.

1.  **Candidate Selection**: Find campaigns targeting "Auto_Intender" + "NY_Metro" (using Redis Sets).
2.  **Filtering**:
    -   **Budget Check**: `GET budget:C-999:remaining`. If > 0, proceed.
    -   **Frequency Cap**: `GET freq:C-999:U-123`. If < 3, proceed.
3.  **Bidding**: Campaign C-999 bids $1.50 CPM. Campaign C-888 bids $1.20 CPM.
4.  **Winner**: Campaign C-999.

### 5. Response & Impression (SQL)
**System**: SQL Engine (Orbit SQL)  
**Action**:
1.  Return creative markup (HTML/VAST) to SSP.
2.  Async Log "Win" event to `daily_performance_stats`.

```sql
UPDATE daily_performance_stats 
SET impressions = impressions + 1, spend = spend + 0.0015
WHERE campaign_id = 'C-999' AND date = CURRENT_DATE;
```
