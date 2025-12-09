# Car Rental Management System

This directory contains Orbit-RS examples for a **Car Rental Agency**. It demonstrates how to combine relational databases for booking consistency with Redis for high-speed availability checking.

## Scenarios

### 1. Booking System & Inventory (SQL)
**File**: `sql/01_rental_system.sql`
-   **Description**: Core management of vehicles, customers, and rental agreements.
-   **Features**: Complex relationships (fleet management), temporal queries (reservation overlaps), and financial calculations.
-   **Orbit-RS Capabilities**: SQL protocol for "system of record" storage.

### 2. Live Availability Cache (Redis)
**File**: `redis/02_availability_cache.redis`
-   **Description**: Millisecond-latency queries for "Is this car class available now?".
-   **Features**: Set operations for fast inventory filtering, TTLs for temporary holds, and atomic booking counters.
-   **Orbit-RS Capabilities**: Redis protocol for caching layer and high-concurrency counters.

## Getting Started

1.  **Start Orbit Server**: Ensure your Orbit-RS server is running with SQL and Redis listeners enabled.
2.  **Run SQL Example**:
    ```bash
    orbit-sql < sql/01_rental_system.sql
    ```
3.  **Run Redis Example**:
    ```bash
    orbit-redis < redis/02_availability_cache.redis
    ```
