Feature: PostgreSQL transactions and savepoints
  As the data backend for Symphony
  I want transactions to commit or roll back atomically, with PostgreSQL's error states
  So that a failed statement or a dropped connection never leaves partial data behind

  Background:
    Given a fresh SQL engine with:
      """
      CREATE TABLE w (id INTEGER PRIMARY KEY, n VARCHAR, q INTEGER);
      INSERT INTO w VALUES (1, 'a', 10), (2, 'b', 20), (3, 'c', 30), (4, 'd', 40);
      """

  # ---------------------------------------------------------------- commit / rollback

  Scenario: A committed transaction is visible to other sessions
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "COMMIT"
    Then session "A" should be outside a transaction
    And session "B" querying "SELECT count(*) FROM w" should return the value "5"

  Scenario: A rolled back transaction leaves no trace
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "UPDATE w SET q = 0 WHERE id = 1"
    And session "A" executes "DELETE FROM w WHERE id = 2"
    Then session "A" querying "SELECT count(*) FROM w" should return the value "4"
    When session "A" executes "ROLLBACK"
    Then session "B" querying "SELECT id, q FROM w ORDER BY id" should return:
      | 1 | 10 |
      | 2 | 20 |
      | 3 | 30 |
      | 4 | 40 |

  Scenario: Uncommitted changes are invisible to other sessions
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    Then session "A" querying "SELECT count(*) FROM w" should return the value "5"
    And session "B" querying "SELECT count(*) FROM w" should return the value "4"

  Scenario: The primary key index is consistent after a rollback
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "ROLLBACK"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    Then the statement should succeed
    And session "A" querying "SELECT count(*) FROM w WHERE id = 5" should return the value "1"

  # ---------------------------------------------------------------- failed transactions

  Scenario: A failed statement aborts the transaction until it ends
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (6, 'f', 60)"
    And session "A" executes "INSERT INTO w VALUES (6, 'dup', 0)"
    Then the statement should fail
    And session "A" should be in a failed transaction
    When session "A" executes "SELECT 1"
    Then the statement should fail with SQLSTATE "25P02"

  Scenario: Committing a failed transaction commits nothing
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (6, 'f', 60)"
    And session "A" executes "INSERT INTO w VALUES (6, 'dup', 0)"
    And session "A" executes "COMMIT"
    Then session "A" should be outside a transaction
    And session "B" querying "SELECT count(*) FROM w WHERE id = 6" should return the value "0"

  # ---------------------------------------------------------------- savepoints

  Scenario: Rolling back to a savepoint keeps earlier work and undoes later work
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "SAVEPOINT sp"
    And session "A" executes "INSERT INTO w VALUES (6, 'f', 60)"
    And session "A" executes "ROLLBACK TO SAVEPOINT sp"
    Then session "A" querying "SELECT id FROM w WHERE id > 4 ORDER BY id" should return:
      | 5 |
    When session "A" executes "COMMIT"
    Then session "B" querying "SELECT id FROM w WHERE id > 4 ORDER BY id" should return:
      | 5 |

  Scenario: Rolling back to a savepoint recovers from a failed statement
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "SAVEPOINT sp"
    And session "A" executes "INSERT INTO w VALUES (6, 'f', 60)"
    And session "A" executes "INSERT INTO w VALUES (6, 'dup', 0)"
    Then session "A" should be in a failed transaction
    When session "A" executes "ROLLBACK TO SAVEPOINT sp"
    Then session "A" should be in an open transaction
    When session "A" executes "INSERT INTO w VALUES (7, 'g', 70)"
    And session "A" executes "RELEASE SAVEPOINT sp"
    And session "A" executes "COMMIT"
    Then session "B" querying "SELECT id FROM w WHERE id > 4 ORDER BY id" should return:
      | 5 |
      | 7 |

  Scenario: Releasing a savepoint keeps the work done after it
    When session "A" executes "BEGIN"
    And session "A" executes "SAVEPOINT sp"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "RELEASE SAVEPOINT sp"
    And session "A" executes "COMMIT"
    Then session "B" querying "SELECT count(*) FROM w WHERE id = 5" should return the value "1"

  Scenario: Rolling back to an outer savepoint discards the inner savepoint's work
    When session "A" executes "BEGIN"
    And session "A" executes "SAVEPOINT outer_sp"
    And session "A" executes "INSERT INTO w VALUES (5, 'e', 50)"
    And session "A" executes "SAVEPOINT inner_sp"
    And session "A" executes "INSERT INTO w VALUES (6, 'f', 60)"
    And session "A" executes "ROLLBACK TO SAVEPOINT outer_sp"
    And session "A" executes "COMMIT"
    Then session "B" querying "SELECT count(*) FROM w" should return the value "4"

  Scenario: A savepoint outside a transaction is refused
    When session "A" executes "SAVEPOINT x"
    Then the statement should fail with SQLSTATE "25P01"

  # ---------------------------------------------------------------- isolation

  Scenario: READ COMMITTED sees rows committed between statements
    When session "A" executes "BEGIN"
    Then session "A" querying "SELECT q FROM w WHERE id = 1" should return the value "10"
    When session "B" executes "UPDATE w SET q = 7 WHERE id = 1"
    Then session "A" querying "SELECT q FROM w WHERE id = 1" should return the value "7"

  Scenario: REPEATABLE READ keeps its snapshot
    When session "A" executes "BEGIN ISOLATION LEVEL REPEATABLE READ"
    Then session "A" querying "SELECT q FROM w WHERE id = 1" should return the value "10"
    When session "B" executes "UPDATE w SET q = 7 WHERE id = 1"
    Then session "A" querying "SELECT q FROM w WHERE id = 1" should return the value "10"

  Scenario: REPEATABLE READ refuses to overwrite a row changed since its snapshot
    When session "A" executes "BEGIN ISOLATION LEVEL REPEATABLE READ"
    And session "A" executes "SELECT q FROM w WHERE id = 1"
    And session "B" executes "UPDATE w SET q = 7 WHERE id = 1"
    And session "A" executes "UPDATE w SET q = 1 WHERE id = 1"
    Then the statement should fail with SQLSTATE "40001"
    When session "A" executes "ROLLBACK"
    Then session "B" querying "SELECT q FROM w WHERE id = 1" should return the value "7"

  # ---------------------------------------------------------------- disconnects

  Scenario: A session dropped mid-transaction is undone and releases the writer
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (9, 'x', 9)"
    And session "A" executes "DELETE FROM w WHERE id = 1"
    And session "A" disconnects
    Then session "B" querying "SELECT id FROM w ORDER BY id" should return:
      | 1 |
      | 2 |
      | 3 |
      | 4 |
    When session "B" executes "INSERT INTO w VALUES (9, 'x', 9)"
    Then the statement should succeed

  Scenario: Committed work survives its session ending
    When session "A" executes "BEGIN"
    And session "A" executes "INSERT INTO w VALUES (10, 'y', 10)"
    And session "A" executes "COMMIT"
    And session "A" disconnects
    Then session "B" querying "SELECT count(*) FROM w" should return the value "5"
