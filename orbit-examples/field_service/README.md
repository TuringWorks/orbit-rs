# Field Service Management

This directory contains Orbit-RS examples for **Field Service Management** (e.g., HVAC, Plumbing, Repair services, similar to ServiceTitan). It demonstrates optimizing workforce dispatch using relational data for orders and impact graphs for technician skills.

## Scenarios

### 1. Work Orders & Scheduling (SQL)
**File**: `sql/01_work_orders.sql`
-   **Description**: Manage service calls, technician assignments, and invoicing.
-   **Features**: Status tracking, time-window scheduling, and job costing.
-   **Orbit-RS Capabilities**: SQL protocol for business process management.

### 2. Technician Skill Matching (Cypher Graph)
**File**: `cypher/02_skill_graph.cypher`
-   **Description**: Find the best technician for a job based on skills, certifications, and location proximity.
-   **Features**: Graph traversals to find "Technicians who KNOW X and live NEAR Y".
-   **Orbit-RS Capabilities**: Cypher (OpenCypher) protocol for complex relationship logic.

## Getting Started

1.  **Start Orbit Server**: Ensure your Orbit-RS server is running with SQL and Cypher (Graph) listeners enabled.
2.  **Run SQL Example**:
    ```bash
    orbit-sql < sql/01_work_orders.sql
    ```
3.  **Run Cypher Example**:
    ```bash
    orbit-cypher < cypher/02_skill_graph.cypher
    ```
