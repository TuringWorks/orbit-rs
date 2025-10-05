Feature: Neo4j Cypher Query Language Support
  As a graph database user
  I want to execute Cypher queries against Orbit-RS
  So that I can manage graph data using familiar Neo4j syntax

  Background:
    Given an empty graph database

  Scenario: Creating a node with properties
    When I execute the Cypher query "CREATE (n:Person {name: 'Alice', age: 30}) RETURN n"
    Then the query should succeed
    And the result should contain 1 row
    And the result should have columns "n"

  Scenario: Querying nodes with filters
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
      | Person | Bob   | 25  |
      | Person | Carol | 35  |
    When I execute the Cypher query "MATCH (n:Person) WHERE n.age > 25 RETURN n.name, n.age"
    Then the query should succeed
    And the result should contain 1 row
    And the result should have columns "n.name, n.age"
    And the first row should contain "n.name" with value "Alice"
    And the first row should contain "n.age" with value "30"

  Scenario: Using parameterized queries
    Given nodes exist with the following data:
      | label  | name | age |
      | Person | Bob  | 25  |
    When I execute the Cypher query "MATCH (n:Person {name: $name}) RETURN n" with parameters:
      | parameter | value | type   |
      | name      | Bob   | string |
    Then the query should succeed
    And the result should contain 1 row

  Scenario: Creating relationships between nodes
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
      | Person | Bob   | 25  |
    When I execute the Cypher query "MATCH (a:Person {name: 'Alice'}), (b:Person {name: 'Bob'}) CREATE (a)-[:KNOWS {since: 2020}]->(b) RETURN a, b"
    Then the query should succeed

  Scenario: Traversing relationships
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
      | Person | Bob   | 25  |
      | Person | Carol | 35  |
    And relationships exist:
      | from  | to    | type  | properties    |
      | Alice | Bob   | KNOWS | since: 2020   |
      | Bob   | Carol | KNOWS | since: 2019   |
    When I execute the Cypher query "MATCH (a:Person)-[:KNOWS]->(b:Person) RETURN a.name, b.name"
    Then the query should succeed
    And the result should contain 2 rows

  Scenario: Variable-length path queries
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
      | Person | Bob   | 25  |
      | Person | Carol | 35  |
      | Person | Dave  | 28  |
    And relationships exist:
      | from  | to    | type  |
      | Alice | Bob   | KNOWS |
      | Bob   | Carol | KNOWS |
      | Carol | Dave  | KNOWS |
    When I execute the Cypher query "MATCH (a:Person {name: 'Alice'})-[:KNOWS*1..3]-(b:Person) RETURN b.name"
    Then the query should succeed
    And the result should contain 3 rows

  Scenario: Shortest path queries
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
      | Person | Bob   | 25  |
      | Person | Carol | 35  |
    And relationships exist:
      | from  | to    | type  |
      | Alice | Bob   | KNOWS |
      | Bob   | Carol | KNOWS |
      | Alice | Carol | KNOWS |
    When I execute the Cypher query "MATCH p = shortestPath((a:Person {name: 'Alice'})-[:KNOWS*]-(c:Person {name: 'Carol'})) RETURN p"
    Then the query should succeed

  Scenario: Aggregation queries
    Given nodes exist with the following data:
      | label  | name  | age | city     |
      | Person | Alice | 30  | New York |
      | Person | Bob   | 25  | New York |
      | Person | Carol | 35  | Boston   |
    When I execute the Cypher query "MATCH (p:Person) RETURN p.city, count(p) as people_count ORDER BY people_count DESC"
    Then the query should succeed

  Scenario: Updating node properties
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
    When I execute the Cypher query "MATCH (p:Person {name: 'Alice'}) SET p.age = 31 RETURN p"
    Then the query should succeed

  Scenario: Deleting nodes
    Given nodes exist with the following data:
      | label  | name  | age |
      | Person | Alice | 30  |
      | Person | Bob   | 25  |
    When I execute the Cypher query "MATCH (p:Person {name: 'Bob'}) DELETE p"
    Then the query should succeed

  Scenario: Complex pattern matching
    Given nodes exist with the following data:
      | label   | name     | type        |
      | Person  | Alice    |             |
      | Person  | Bob      |             |
      | Company | TechCorp | Technology  |
    And relationships exist:
      | from     | to       | type      | properties |
      | Alice    | TechCorp | WORKS_FOR | role: Engineer |
      | Bob      | TechCorp | WORKS_FOR | role: Manager  |
    When I execute the Cypher query "MATCH (p:Person)-[r:WORKS_FOR]->(c:Company) WHERE c.type = 'Technology' RETURN p.name, r.role, c.name"
    Then the query should succeed

  Scenario: Invalid query syntax
    When I execute the Cypher query "INVALID SYNTAX HERE"
    Then the query should fail with error "Query not found"

  Scenario: Non-existent node query
    When I execute the Cypher query "MATCH (n:NonExistent) RETURN n"
    Then the query should succeed
    And the result should contain 0 rows