Feature: Vector Similarity Search
  As a machine learning engineer
  I want to store and search for similar vectors
  So that I can build recommendation systems and semantic search

  Scenario: Add vectors to index
    Given I have a vector store with dimension 3
    When I add a vector with id "vec1" and data "[1.0, 2.0, 3.0]"
    And I add a vector with id "vec2" and data "[2.0, 3.0, 4.0]"
    And I add a vector with id "vec3" and data "[1.1, 2.1, 3.1]"
    Then the operation should succeed

  Scenario: Search for similar vectors
    Given I have a vector store with dimension 3
    When I add a vector with id "doc1" and data "[1.0, 0.0, 0.0]"
    And I add a vector with id "doc2" and data "[0.0, 1.0, 0.0]"
    And I add a vector with id "doc3" and data "[0.9, 0.1, 0.0]"
    And I search for vectors similar to "[1.0, 0.0, 0.0]" with limit 2
    Then the operation should succeed
    And I should find 2 similar vectors
    And the first result should have id "doc1"

  Scenario: Handle empty vector store
    Given I have a vector store with dimension 5
    When I search for vectors similar to "[1.0, 2.0, 3.0, 4.0, 5.0]" with limit 10
    Then the operation should succeed
    And I should find 0 similar vectors

  Scenario: Cosine similarity search
    Given I have a vector store with dimension 2
    When I add a vector with id "positive" and data "[1.0, 1.0]"
    And I add a vector with id "negative" and data "[-1.0, -1.0]"
    And I add a vector with id "orthogonal" and data "[1.0, -1.0]"
    And I search for vectors similar to "[2.0, 2.0]" with limit 3
    Then the operation should succeed
    And I should find 3 similar vectors
    And the first result should have id "positive"

  Scenario: Large vector dimensions
    Given I have a vector store with dimension 128
    When I add a vector with id "embedding1" and data "[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]"
    And I search for vectors similar to "[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]" with limit 1
    Then the operation should succeed
    And I should find 1 similar vectors
    And the first result should have id "embedding1"