-- =============================================================================
-- Basic CQL Examples
-- Standard Cassandra Query Language statements
-- =============================================================================

-- Keyspace Operations
CREATE KEYSPACE IF NOT EXISTS my_app
WITH REPLICATION = {
    'class': 'NetworkTopologyStrategy',
    'datacenter1': 3,
    'datacenter2': 2
}
AND DURABLE_WRITES = true;

USE my_app;

ALTER KEYSPACE my_app
WITH REPLICATION = {'class': 'SimpleStrategy', 'replication_factor': 3};

-- Table Operations
CREATE TABLE users (
    user_id uuid,
    username text,
    email text,
    created_at timestamp,
    profile frozen<map<text, text>>,
    tags set<text>,
    PRIMARY KEY (user_id)
);

CREATE TABLE posts (
    user_id uuid,
    post_id timeuuid,
    title text,
    content text,
    published boolean,
    views counter,
    PRIMARY KEY ((user_id), post_id)
) WITH CLUSTERING ORDER BY (post_id DESC)
  AND comment = 'User posts table'
  AND gc_grace_seconds = 864000;

CREATE TABLE events (
    partition_date date,
    event_id uuid,
    event_type text STATIC,
    data blob,
    PRIMARY KEY ((partition_date), event_id)
);

ALTER TABLE users ADD last_login timestamp;
ALTER TABLE users DROP profile;
ALTER TABLE users RENAME username TO user_name;

-- Secondary Indexes
CREATE INDEX ON users (email);
CREATE INDEX user_tags_idx ON users (tags);
CREATE INDEX posts_title_idx ON posts (title);

DROP INDEX IF EXISTS user_tags_idx;

-- Insert Operations
INSERT INTO users (user_id, username, email, created_at, tags)
VALUES (
    550e8400-e29b-41d4-a716-446655440000,
    'johndoe',
    'john@example.com',
    '2024-01-15T10:30:00Z',
    {'developer', 'blogger'}
);

INSERT INTO users (user_id, username, email)
VALUES (uuid(), 'janedoe', 'jane@example.com')
IF NOT EXISTS;

INSERT INTO users JSON '{"user_id": "550e8400-e29b-41d4-a716-446655440001", "username": "alice"}';

INSERT INTO posts (user_id, post_id, title, content, published)
VALUES (550e8400-e29b-41d4-a716-446655440000, now(), 'Hello World', 'My first post', true)
USING TTL 86400;

-- Update Operations
UPDATE users
SET email = 'newemail@example.com', tags = tags + {'writer'}
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000;

UPDATE posts
SET title = 'Updated Title'
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000
  AND post_id = 550e8400-e29b-41d4-a716-446655440002
IF EXISTS;

UPDATE posts
USING TIMESTAMP 1234567890123
SET content = 'Updated content'
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000
  AND post_id = 550e8400-e29b-41d4-a716-446655440002;

UPDATE posts
SET views = views + 1
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000
  AND post_id = 550e8400-e29b-41d4-a716-446655440002;

-- Delete Operations
DELETE FROM users WHERE user_id = 550e8400-e29b-41d4-a716-446655440000;

DELETE tags FROM users WHERE user_id = 550e8400-e29b-41d4-a716-446655440001;

DELETE FROM posts
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000
  AND post_id = 550e8400-e29b-41d4-a716-446655440002
IF EXISTS;

-- Select Operations
SELECT * FROM users;

SELECT user_id, username, email FROM users LIMIT 10;

SELECT DISTINCT user_id FROM posts;

SELECT username, writetime(email), ttl(email) FROM users
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000;

SELECT * FROM posts
WHERE user_id = 550e8400-e29b-41d4-a716-446655440000
  AND post_id > minTimeuuid('2024-01-01')
ORDER BY post_id DESC
LIMIT 20;

SELECT * FROM users WHERE tags CONTAINS 'developer';

SELECT token(user_id), user_id, username FROM users;

SELECT * FROM posts
WHERE user_id IN (550e8400-e29b-41d4-a716-446655440000, 550e8400-e29b-41d4-a716-446655440001);

SELECT count(*) FROM users;

SELECT * FROM events
WHERE partition_date = '2024-01-15'
ALLOW FILTERING;

-- Batch Operations
BEGIN BATCH
    INSERT INTO users (user_id, username) VALUES (uuid(), 'user1');
    INSERT INTO users (user_id, username) VALUES (uuid(), 'user2');
    UPDATE users SET email = 'user1@test.com' WHERE user_id = 550e8400-e29b-41d4-a716-446655440000;
APPLY BATCH;

BEGIN UNLOGGED BATCH USING TIMESTAMP 1234567890123
    DELETE FROM posts WHERE user_id = 550e8400-e29b-41d4-a716-446655440000 AND post_id = now();
    INSERT INTO events (partition_date, event_id, data) VALUES ('2024-01-15', uuid(), 0x00FF);
APPLY BATCH;

-- Materialized Views
CREATE MATERIALIZED VIEW users_by_email AS
    SELECT user_id, username, email, created_at
    FROM users
    WHERE email IS NOT NULL AND user_id IS NOT NULL
    PRIMARY KEY (email, user_id);

ALTER MATERIALIZED VIEW users_by_email WITH gc_grace_seconds = 172800;

DROP MATERIALIZED VIEW IF EXISTS users_by_email;

-- User-Defined Types
CREATE TYPE address (
    street text,
    city text,
    zip_code text,
    country text
);

ALTER TYPE address ADD state text;
ALTER TYPE address RENAME zip_code TO postal_code;

DROP TYPE IF EXISTS address;

-- User-Defined Functions (UDF)
CREATE FUNCTION my_app.to_uppercase(input text)
CALLED ON NULL INPUT
RETURNS text
LANGUAGE java
AS 'return input != null ? input.toUpperCase() : null;';

CREATE OR REPLACE FUNCTION my_app.calculate_age(birth_date date)
RETURNS NULL ON NULL INPUT
RETURNS int
LANGUAGE java
AS $$
    return (int) java.time.temporal.ChronoUnit.YEARS.between(
        birth_date.toLocalDate(), 
        java.time.LocalDate.now()
    );
$$;

DROP FUNCTION IF EXISTS my_app.to_uppercase;

-- User-Defined Aggregates (UDA)
CREATE FUNCTION state_func(state tuple<bigint, int>, val int)
CALLED ON NULL INPUT
RETURNS tuple<bigint, int>
LANGUAGE java
AS 'return state;';

CREATE FUNCTION final_func(state tuple<bigint, int>)
CALLED ON NULL INPUT
RETURNS double
LANGUAGE java
AS 'return 0.0;';

CREATE AGGREGATE my_avg(int)
SFUNC state_func
STYPE tuple<bigint, int>
FINALFUNC final_func
INITCOND (0, 0);

DROP AGGREGATE IF EXISTS my_avg;

-- Roles and Permissions
CREATE ROLE app_user WITH PASSWORD = 'secret123' AND LOGIN = true;

CREATE ROLE admin_role WITH SUPERUSER = true;

ALTER ROLE app_user WITH PASSWORD = 'newsecret456';

GRANT SELECT ON KEYSPACE my_app TO app_user;
GRANT ALL PERMISSIONS ON TABLE users TO admin_role;
GRANT EXECUTE ON FUNCTION my_app.to_uppercase TO app_user;

REVOKE SELECT ON users FROM app_user;

LIST ALL PERMISSIONS OF app_user;
LIST ROLES;
LIST ROLES OF admin_role NORECURSIVE;

DROP ROLE IF EXISTS app_user;

-- Triggers
CREATE TRIGGER audit_trigger ON users
USING 'com.example.AuditTrigger';

DROP TRIGGER IF EXISTS audit_trigger ON users;

-- Utility Statements
TRUNCATE TABLE posts;

DESCRIBE KEYSPACE my_app;
DESCRIBE TABLE users;
DESCRIBE INDEX user_tags_idx;

-- Prepared Statements (bind markers)
SELECT * FROM users WHERE user_id = ?;
INSERT INTO users (user_id, username, email) VALUES (?, ?, ?);
UPDATE users SET email = :email WHERE user_id = :user_id;

-- Clean up
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS posts;
DROP TABLE IF EXISTS users;
DROP KEYSPACE IF EXISTS my_app;
