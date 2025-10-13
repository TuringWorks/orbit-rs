Feature: Database Operations
  As a database user
  I want to perform CRUD operations on tables
  So that I can manage my data effectively

  Background:
    Given I have an empty database

  Scenario: Create a new table
    When I execute the SQL statement "CREATE TABLE users (id INTEGER, name TEXT, email TEXT)"
    Then the operation should succeed

  Scenario: Insert data into a table
    Given I have a table "products" with columns "id INTEGER, name TEXT, price DECIMAL"
    When I execute the SQL statement "INSERT INTO products VALUES (1, 'Laptop', 999.99)"
    Then the operation should succeed

  Scenario: Select data from a table
    Given I have a table "customers" with columns "id INTEGER, name TEXT, city TEXT"
    When I insert the following data into "customers":
      | id | name    | city      |
      | 1  | Alice   | New York  |
      | 2  | Bob     | Boston    |
      | 3  | Charlie | Chicago   |
    And I execute the SQL statement "SELECT * FROM customers"
    Then the operation should succeed
    And the result should have 3 rows

  Scenario: Update existing data
    Given I have a table "accounts" with columns "id INTEGER, name TEXT, balance DECIMAL"
    When I insert the following data into "accounts":
      | id | name  | balance |
      | 1  | Alice | 100.00  |
      | 2  | Bob   | 200.00  |
    And I execute the SQL statement "UPDATE accounts SET balance = 150.00 WHERE id = 1"
    Then the operation should succeed

  Scenario: Delete data from a table
    Given I have a table "temp_data" with columns "id INTEGER, value TEXT"
    When I insert the following data into "temp_data":
      | id | value  |
      | 1  | keep   |
      | 2  | delete |
      | 3  | keep   |
    And I execute the SQL statement "DELETE FROM temp_data WHERE value = 'delete'"
    Then the operation should succeed
    And the table "temp_data" should have 2 rows

  Scenario: Join operations
    Given I have a table "orders" with columns "id INTEGER, customer_id INTEGER, amount DECIMAL"
    And I have a table "customers" with columns "id INTEGER, name TEXT"
    When I insert the following data into "customers":
      | id | name  |
      | 1  | Alice |
      | 2  | Bob   |
    And I insert the following data into "orders":
      | id  | customer_id | amount |
      | 100 | 1           | 50.00  |
      | 101 | 2           | 75.00  |
    And I execute the SQL statement "SELECT c.name, o.amount FROM customers c INNER JOIN orders o ON c.id = o.customer_id"
    Then the operation should succeed
    And the result should have 2 rows

  Scenario: Error handling for non-existent table
    When I execute the SQL statement "SELECT * FROM non_existent_table"
    Then the operation should fail
    And the error message should contain "does not exist"

  Scenario: Error handling for duplicate table creation
    Given I have a table "duplicate_test" with columns "id INTEGER"
    When I execute the SQL statement "CREATE TABLE duplicate_test (id INTEGER)"
    Then the operation should fail
    And the error message should contain "already exists"

  Scenario: IF NOT EXISTS handling
    Given I have a table "existing_table" with columns "id INTEGER"
    When I execute the SQL statement "CREATE TABLE IF NOT EXISTS existing_table (id INTEGER)"
    Then the operation should succeed