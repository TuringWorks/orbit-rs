# Ride Hailing & Robo Taxi Fleet Management

This directory contains Orbit-RS examples for a **Ride Hailing and Autonomous Vehicle Fleet** application. It explicitly demonstrates how to manage high-velocity geospatial data, transactional ride ledgers, and fleet operations using Orbit-RS's multi-protocol support.

## Scenarios

### 1. Real-time Fleet Tracking (MongoDB)
**File**: `mongodb/01_fleet_tracking.js`
-   **Description**: Ingesting and querying real-time telemetry from thousands of vehicles.
-   **Features**: Geospatial indexing (`2dsphere`), time-series data for vehicle status, and efficient updates.
-   **Orbit-RS Capabilities**: MongoDB protocol support for document storage and geospatial queries.

### 2. Ride Ledger & Billing (SQL)
**File**: `sql/02_ride_ledger.sql`
-   **Description**: The "system of record" for financial transactions, ride bookings, and user profiles.
-   **Features**: ACID compliance, relational schemas for Riders/Drivers/Trips, and financial reporting views.
-   **Orbit-RS Capabilities**: SQL protocol for structured, relational data.

## Getting Started

1.  **Start Orbit Server**: Ensure your Orbit-RS server is running with MongoDB and SQL listeners enabled.
2.  **Run MongoDB Example**:
    ```bash
    orbit-mongo < mongodb/01_fleet_tracking.js
    ```
3.  **Run SQL Example**:
    ```bash
    orbit-sql < sql/02_ride_ledger.sql
    ```
