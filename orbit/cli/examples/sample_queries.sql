-- Sample SQL queries for testing Orbit CLI
-- Execute with: orbit -f examples/sample_queries.sql

-- Create a users table
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    age INTEGER,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Insert sample data
INSERT INTO users (name, email, age) VALUES
    ('Alice Johnson', 'alice@example.com', 30),
    ('Bob Smith', 'bob@example.com', 25),
    ('Carol Davis', 'carol@example.com', 35),
    ('David Wilson', 'david@example.com', 28),
    ('Eve Martinez', 'eve@example.com', 32);

-- Select all users
SELECT * FROM users;

-- Select users over 30
SELECT id, name, email
FROM users
WHERE age > 30
ORDER BY name;

-- Count users by age range
SELECT
    CASE
        WHEN age < 30 THEN 'Under 30'
        WHEN age >= 30 AND age < 40 THEN '30-39'
        ELSE '40+'
    END AS age_range,
    COUNT(*) as user_count
FROM users
GROUP BY age_range;

-- Update a user's email
UPDATE users
SET email = 'alice.johnson@example.com'
WHERE name = 'Alice Johnson';

-- Delete users under 26
DELETE FROM users
WHERE age < 26;

-- View remaining users
SELECT name, email, age
FROM users
ORDER BY age DESC;
