# Car Dealership & Vehicle Sales

This directory contains Orbit-RS examples for a **Car Dealership** or **Automotive Retailer**. It showcases how to handle structured sales data alongside unstructured customer interaction histories.

## Scenarios

### 1. Inventory & Sales Management (SQL)
**File**: `sql/01_inventory_sales.sql`
-   **Description**: Manage vehicle inventory, sales transactions, and financing details.
-   **Features**: Relational tracking of VINs, complex sales invoices with tax/fees, and salesman performance tracking.
-   **Orbit-RS Capabilities**: SQL protocol for financial integrity and reporting.

### 2. Customer 360 View (MongoDB)
**File**: `mongodb/02_customer_360.js`
-   **Description**: A unified view of the customer journey, including web visits, test drives, service requests, and notes.
-   **Features**: Rich document structure for heterogeneous interaction logs and preferences.
-   **Orbit-RS Capabilities**: MongoDB protocol for flexible, document-oriented CRM data.

## Getting Started

1.  **Start Orbit Server**: Ensure your Orbit-RS server is running with SQL and MongoDB listeners enabled.
2.  **Run SQL Example**:
    ```bash
    orbit-sql < sql/01_inventory_sales.sql
    ```
3.  **Run MongoDB Example**:
    ```bash
    orbit-mongo < mongodb/02_customer_360.js
    ```
