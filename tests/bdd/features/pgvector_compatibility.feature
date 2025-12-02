Feature: pgvector PostgreSQL Extension Compatibility
  As a developer using pgvector-compatible SQL
  I want Orbit to support standard pgvector syntax
  So that I can migrate existing applications without code changes

  Background:
    Given I have a connection to Orbit PostgreSQL interface
    And the vector extension is available

  # ==========================================================================
  # Extension Management
  # ==========================================================================

  Scenario: Create pgvector extension
    When I execute "CREATE EXTENSION IF NOT EXISTS vector"
    Then the operation should succeed
    And the extension "vector" should be registered

  # ==========================================================================
  # Table Creation with Vector Types
  # ==========================================================================

  Scenario: Create table with vector column
    When I execute "CREATE TABLE items (id SERIAL PRIMARY KEY, embedding vector(3))"
    Then the operation should succeed
    And the table "items" should exist
    And the column "embedding" should have dimension 3

  Scenario: Create table with vector column for OpenAI ada-002 embeddings
    When I execute "CREATE TABLE openai_docs (id SERIAL, content TEXT, embedding vector(1536))"
    Then the operation should succeed
    And the column "embedding" should have dimension 1536

  Scenario: Create table with vector column for OpenAI text-embedding-3-large
    When I execute "CREATE TABLE large_docs (id SERIAL, content TEXT, embedding vector(3072))"
    Then the operation should succeed
    And the column "embedding" should have dimension 3072

  Scenario: Create table with vector column for Sentence-Transformers (all-MiniLM-L6-v2)
    When I execute "CREATE TABLE sentences (id SERIAL, text TEXT, embedding vector(384))"
    Then the operation should succeed
    And the column "embedding" should have dimension 384

  Scenario: Create table with VECTOR type without dimension
    When I execute "CREATE TABLE flexible (id SERIAL, embedding VECTOR)"
    Then the operation should succeed
    And the column "embedding" should have unspecified dimension

  Scenario: Create table with HALFVEC type
    When I execute "CREATE TABLE half_precision (id SERIAL, embedding halfvec(256))"
    Then the operation should succeed
    And the column "embedding" should be of type "halfvec"

  Scenario: Create table with multiple vector columns
    When I execute:
      """
      CREATE TABLE multi_embed (
        id SERIAL,
        title TEXT,
        title_embedding vector(128),
        content TEXT,
        content_embedding vector(384)
      )
      """
    Then the operation should succeed
    And the column "title_embedding" should have dimension 128
    And the column "content_embedding" should have dimension 384

  # ==========================================================================
  # ANSI SQL Identifier Handling
  # ==========================================================================

  Scenario: Table names are case-insensitive (ANSI SQL)
    When I execute "CREATE TABLE Documents (id SERIAL, embedding vector(3))"
    Then the operation should succeed
    When I execute "INSERT INTO documents (id, embedding) VALUES ('1', '[1,2,3]')"
    Then the operation should succeed
    When I execute "INSERT INTO DOCUMENTS (id, embedding) VALUES ('2', '[4,5,6]')"
    Then the operation should succeed

  Scenario: Quoted identifiers preserve case
    When I execute 'CREATE TABLE "MixedCase" (id SERIAL, embedding vector(3))'
    Then the operation should succeed
    And the table "MixedCase" should exist
    And the table "MIXEDCASE" should not exist

  # ==========================================================================
  # Vector Data Insertion
  # ==========================================================================

  Scenario: Insert vector with array-style literal
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    When I execute "INSERT INTO items (id, embedding) VALUES ('doc1', '[1,2,3]')"
    Then the operation should succeed
    And the table should contain 1 row

  Scenario: Insert vector with floating point values
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    When I execute "INSERT INTO items (id, embedding) VALUES ('doc1', '[0.1, 0.2, 0.3]')"
    Then the operation should succeed

  Scenario: Insert vector with negative values
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    When I execute "INSERT INTO items (id, embedding) VALUES ('doc1', '[-1.0, -0.5, 0.5]')"
    Then the operation should succeed

  Scenario: Insert vector with scientific notation
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    When I execute "INSERT INTO items (id, embedding) VALUES ('doc1', '[1e-5, 2e-5, 3e-5]')"
    Then the operation should succeed

  # ==========================================================================
  # Vector Dimension Validation
  # ==========================================================================

  Scenario: Auto-detect dimension from first insert
    Given a table "flexible" with columns "id SERIAL, embedding VECTOR"
    When I execute "INSERT INTO flexible (id, embedding) VALUES ('1', '[1,2,3,4]')"
    Then the operation should succeed
    And the column "embedding" should have dimension 4
    When I execute "INSERT INTO flexible (id, embedding) VALUES ('2', '[5,6,7,8]')"
    Then the operation should succeed

  Scenario: Dimension mismatch on insert is rejected
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    When I execute "INSERT INTO items (id, embedding) VALUES ('1', '[1,2,3]')"
    Then the operation should succeed
    When I execute "INSERT INTO items (id, embedding) VALUES ('2', '[1,2,3,4,5]')"
    Then the operation should fail
    And the error should mention "dimension mismatch"

  Scenario: Query vector dimension mismatch is rejected
    Given a table "items" with columns "id SERIAL, content TEXT, embedding vector(3)"
    And I have inserted:
      | id  | content | embedding   |
      | 1   | test    | [1,2,3]     |
    When I execute "SELECT content, embedding <-> '[1,2,3,4,5]' AS distance FROM items"
    Then the operation should fail
    And the error should mention "dimension mismatch"

  # ==========================================================================
  # Index Creation
  # ==========================================================================

  Scenario: Create HNSW index with cosine distance
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    And I have inserted a vector "[1,0,0]" with id "1"
    When I execute "CREATE INDEX ON items USING hnsw (embedding vector_cosine_ops)"
    Then the operation should succeed
    And an HNSW index should exist on "items.embedding"

  Scenario: Create HNSW index with L2 distance
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    And I have inserted a vector "[1,0,0]" with id "1"
    When I execute "CREATE INDEX ON items USING hnsw (embedding vector_l2_ops)"
    Then the operation should succeed

  Scenario: Create HNSW index with inner product
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    And I have inserted a vector "[1,0,0]" with id "1"
    When I execute "CREATE INDEX ON items USING hnsw (embedding vector_ip_ops)"
    Then the operation should succeed

  Scenario: Create IVFFlat index
    Given a table "items" with columns "id SERIAL, embedding vector(3)"
    And I have inserted a vector "[1,0,0]" with id "1"
    When I execute "CREATE INDEX ON items USING ivfflat (embedding vector_cosine_ops)"
    Then the operation should succeed

  Scenario: Cannot create index on unspecified dimension
    Given a table "flexible" with columns "id SERIAL, embedding VECTOR"
    When I execute "CREATE INDEX ON flexible USING hnsw (embedding vector_cosine_ops)"
    Then the operation should fail
    And the error should mention "dimension not specified"

  # ==========================================================================
  # Similarity Search Operations
  # ==========================================================================

  Scenario: L2 distance search with <-> operator
    Given a table "items" with columns "id SERIAL, content TEXT, embedding vector(3)"
    And I have inserted:
      | id  | content | embedding   |
      | 1   | first   | [1,0,0]     |
      | 2   | second  | [0,1,0]     |
      | 3   | third   | [0,0,1]     |
    When I execute "SELECT content, embedding <-> '[1,0,0]' AS distance FROM items ORDER BY distance LIMIT 2"
    Then the operation should succeed
    And I should get 2 results
    And the first result should have content "first"

  Scenario: Cosine distance search with <=> operator
    Given a table "items" with columns "id SERIAL, content TEXT, embedding vector(3)"
    And I have inserted:
      | id  | content | embedding   |
      | 1   | similar | [1,1,0]     |
      | 2   | opposite| [-1,-1,0]   |
    When I execute "SELECT content, embedding <=> '[1,1,0]' AS distance FROM items ORDER BY distance LIMIT 2"
    Then the operation should succeed
    And the first result should have content "similar"

  Scenario: Inner product search with <#> operator
    Given a table "items" with columns "id SERIAL, content TEXT, embedding vector(3)"
    And I have inserted:
      | id  | content | embedding   |
      | 1   | high    | [1,1,1]     |
      | 2   | low     | [0,0,0.1]   |
    When I execute "SELECT content, embedding <#> '[1,1,1]' AS score FROM items ORDER BY score LIMIT 2"
    Then the operation should succeed

  # ==========================================================================
  # RAG (Retrieval Augmented Generation) Workflow
  # ==========================================================================

  Scenario: Complete RAG workflow
    # Step 1: Create table for documents
    When I execute:
      """
      CREATE TABLE rag_documents (
        id SERIAL,
        content TEXT,
        embedding vector(3)
      )
      """
    Then the operation should succeed

    # Step 2: Insert documents with embeddings
    When I execute "INSERT INTO rag_documents (id, content, embedding) VALUES ('1', 'The quick brown fox', '[0.1, 0.8, 0.3]')"
    Then the operation should succeed
    When I execute "INSERT INTO rag_documents (id, content, embedding) VALUES ('2', 'A lazy dog sleeps', '[0.9, 0.1, 0.2]')"
    Then the operation should succeed
    When I execute "INSERT INTO rag_documents (id, content, embedding) VALUES ('3', 'The fox jumps over', '[0.15, 0.75, 0.35]')"
    Then the operation should succeed

    # Step 3: Create index for fast retrieval
    When I execute "CREATE INDEX ON rag_documents USING hnsw (embedding vector_cosine_ops)"
    Then the operation should succeed

    # Step 4: Search for similar documents
    When I execute "SELECT content, embedding <=> '[0.12, 0.78, 0.32]' AS similarity FROM rag_documents ORDER BY similarity LIMIT 2"
    Then the operation should succeed
    And I should get 2 results
    # "The quick brown fox" and "The fox jumps over" should be most similar

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Helpful error for creating index on nonexistent table
    When I execute "CREATE INDEX ON nonexistent USING hnsw (embedding vector_cosine_ops)"
    Then the operation should fail
    And the error should mention "not found"
    And the error should suggest creating the table first

  Scenario: Helpful error for dimension mismatch
    Given a table "items" with columns "id SERIAL, embedding vector(384)"
    When I execute "INSERT INTO items (id, embedding) VALUES ('1', '[1,2,3]')"
    Then the operation should fail
    And the error should mention "expected 384 dimensions"
    And the error should mention common model dimensions

  # ==========================================================================
  # Vector Literal Parsing Edge Cases
  # ==========================================================================

  Scenario Outline: Parse various vector literal formats
    Given a table "parse_test" with columns "id SERIAL, embedding vector(3)"
    When I execute "INSERT INTO parse_test (id, embedding) VALUES ('1', '<vector_literal>')"
    Then the operation should succeed

    Examples:
      | vector_literal        |
      | [1, 2, 3]             |
      | [1,2,3]               |
      | [ 1, 2, 3 ]           |
      | [1.0, 2.0, 3.0]       |
      | [0.001, 0.002, 0.003] |
      | [-1, -2, -3]          |
