# Car Sharing (Peer-to-Peer & Fleet)

This directory contains Orbit-RS examples for a **Car Sharing Service** (similar to Zipcar or Turo). It demonstrates managing membership plans, hourly bookings, and real-time vehicle IoT states.

## Scenarios

### 1. Membership & Hourly Bookings (SQL)
**File**: `sql/01_membership_bookings.sql`
-   **Description**: Manage user memberships, pods/stations, and short-term bookings.
-   **Features**: Time-overlap constraints, late return penalties, and membership tier pricing.
-   **Orbit-RS Capabilities**: SQL protocol for transactional integrity of bookings.

### 2. Vehicle IoT Lock State (Redis)
**File**: `redis/02_vehicle_lock_state.redis`
-   **Description**: Real-time handling of vehicle lock/unlock commands and status updates from the car's modem.
-   **Features**: Key expiration for temporary access codes, Pub/Sub for command delivery.
-   **Orbit-RS Capabilities**: Redis protocol for low-latency IoT command and control.

## Getting Started

1.  **Start Orbit Server**: Ensure your Orbit-RS server is running with SQL and Redis listeners enabled.
2.  **Run SQL Example**:
    ```bash
    orbit-sql < sql/01_membership_bookings.sql
    ```
3.  **Run Redis Example**:
    ```bash
    orbit-redis < redis/02_vehicle_lock_state.redis
    ```
