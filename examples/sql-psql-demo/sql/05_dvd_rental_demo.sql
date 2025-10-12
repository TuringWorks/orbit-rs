-- DVD Rental Database Demo for Orbit-RS
-- Simplified version of the PostgreSQL Tutorial DVD Rental Sample Database
-- Adapted to work with current Orbit-RS SQL parser capabilities
-- 
-- Original source: https://www.postgresqltutorial.com/postgresql-sample-database/
-- This version focuses on basic table structures, relationships, and sample data

\echo '🎬 DVD Rental Database Demo'
\echo '=========================='
\echo ''
\echo '1. Creating core tables...'

-- Country table
CREATE TABLE country (
    country_id INTEGER PRIMARY KEY,
    country_name TEXT NOT NULL,
    last_update TIMESTAMP
);

-- City table  
CREATE TABLE city (
    city_id INTEGER PRIMARY KEY,
    city_name TEXT NOT NULL,
    country_id INTEGER NOT NULL,
    last_update TIMESTAMP
);

-- Address table
CREATE TABLE address (
    address_id INTEGER PRIMARY KEY,
    address_line TEXT NOT NULL,
    district TEXT,
    city_id INTEGER NOT NULL,
    postal_code TEXT,
    phone TEXT,
    last_update TIMESTAMP
);

-- Actor table
CREATE TABLE actor (
    actor_id INTEGER PRIMARY KEY,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    last_update TIMESTAMP
);

-- Category table
CREATE TABLE category (
    category_id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    last_update TIMESTAMP
);

-- Film table (simplified - removed custom types and arrays)
CREATE TABLE film (
    film_id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    release_year INTEGER,
    language_id INTEGER NOT NULL,
    rental_duration INTEGER DEFAULT 3,
    rental_rate DECIMAL(4,2) DEFAULT 4.99,
    length INTEGER,
    replacement_cost DECIMAL(5,2) DEFAULT 19.99,
    rating TEXT DEFAULT 'G',
    last_update TIMESTAMP
);

-- Film-Actor relationship table
CREATE TABLE film_actor (
    actor_id INTEGER NOT NULL,
    film_id INTEGER NOT NULL,
    last_update TIMESTAMP
);

-- Film-Category relationship table
CREATE TABLE film_category (
    film_id INTEGER NOT NULL,
    category_id INTEGER NOT NULL,
    last_update TIMESTAMP
);

-- Store table
CREATE TABLE store (
    store_id INTEGER PRIMARY KEY,
    manager_staff_id INTEGER NOT NULL,
    address_id INTEGER NOT NULL,
    last_update TIMESTAMP
);

-- Staff table
CREATE TABLE staff (
    staff_id INTEGER PRIMARY KEY,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    address_id INTEGER NOT NULL,
    email TEXT,
    store_id INTEGER NOT NULL,
    active BOOLEAN DEFAULT TRUE,
    username TEXT NOT NULL,
    last_update TIMESTAMP
);

-- Customer table
CREATE TABLE customer (
    customer_id INTEGER PRIMARY KEY,
    store_id INTEGER NOT NULL,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    email TEXT,
    address_id INTEGER NOT NULL,
    active BOOLEAN DEFAULT TRUE,
    create_date DATE,
    last_update TIMESTAMP
);

-- Inventory table
CREATE TABLE inventory (
    inventory_id INTEGER PRIMARY KEY,
    film_id INTEGER NOT NULL,
    store_id INTEGER NOT NULL,
    last_update TIMESTAMP
);

-- Rental table
CREATE TABLE rental (
    rental_id INTEGER PRIMARY KEY,
    rental_date TIMESTAMP NOT NULL,
    inventory_id INTEGER NOT NULL,
    customer_id INTEGER NOT NULL,
    return_date TIMESTAMP,
    staff_id INTEGER NOT NULL,
    last_update TIMESTAMP
);

-- Payment table
CREATE TABLE payment (
    payment_id INTEGER PRIMARY KEY,
    customer_id INTEGER NOT NULL,
    staff_id INTEGER NOT NULL,
    rental_id INTEGER NOT NULL,
    amount DECIMAL(5,2) NOT NULL,
    payment_date TIMESTAMP NOT NULL
);

\echo '✅ Tables created successfully!'
\echo ''
\echo '2. Inserting sample data...'

-- Insert sample countries
INSERT INTO country (country_id, country_name, last_update) VALUES
(1, 'United States', '2020-02-15 09:45:25'),
(2, 'Canada', '2020-02-15 09:45:25'),
(3, 'United Kingdom', '2020-02-15 09:45:25'),
(4, 'France', '2020-02-15 09:45:25'),
(5, 'Germany', '2020-02-15 09:45:25');

-- Insert sample cities
INSERT INTO city (city_id, city_name, country_id, last_update) VALUES
(1, 'New York', 1, '2020-02-15 09:45:25'),
(2, 'Los Angeles', 1, '2020-02-15 09:45:25'),
(3, 'Toronto', 2, '2020-02-15 09:45:25'),
(4, 'London', 3, '2020-02-15 09:45:25'),
(5, 'Paris', 4, '2020-02-15 09:45:25'),
(6, 'Berlin', 5, '2020-02-15 09:45:25');

-- Insert sample addresses
INSERT INTO address (address_id, address_line, district, city_id, postal_code, phone, last_update) VALUES
(1, '123 Main St', 'Manhattan', 1, '10001', '555-1234', '2020-02-15 09:45:25'),
(2, '456 Hollywood Blvd', 'Hollywood', 2, '90028', '555-5678', '2020-02-15 09:45:25'),
(3, '789 Queen St', 'Downtown', 3, 'M5H 2M9', '555-9012', '2020-02-15 09:45:25'),
(4, '321 Oxford St', 'Westminster', 4, 'W1A 0AX', '555-3456', '2020-02-15 09:45:25'),
(5, '654 Champs-Élysées', '8th arrondissement', 5, '75008', '555-7890', '2020-02-15 09:45:25');

-- Insert sample actors
INSERT INTO actor (actor_id, first_name, last_name, last_update) VALUES
(1, 'Johnny', 'Depp', '2020-02-15 09:45:25'),
(2, 'Meryl', 'Streep', '2020-02-15 09:45:25'),
(3, 'Leonardo', 'DiCaprio', '2020-02-15 09:45:25'),
(4, 'Scarlett', 'Johansson', '2020-02-15 09:45:25'),
(5, 'Tom', 'Hanks', '2020-02-15 09:45:25'),
(6, 'Emma', 'Stone', '2020-02-15 09:45:25'),
(7, 'Robert', 'Downey Jr.', '2020-02-15 09:45:25'),
(8, 'Jennifer', 'Lawrence', '2020-02-15 09:45:25');

-- Insert sample categories
INSERT INTO category (category_id, name, last_update) VALUES
(1, 'Action', '2020-02-15 09:45:25'),
(2, 'Comedy', '2020-02-15 09:45:25'),
(3, 'Drama', '2020-02-15 09:45:25'),
(4, 'Horror', '2020-02-15 09:45:25'),
(5, 'Romance', '2020-02-15 09:45:25'),
(6, 'Sci-Fi', '2020-02-15 09:45:25'),
(7, 'Documentary', '2020-02-15 09:45:25'),
(8, 'Family', '2020-02-15 09:45:25');

-- Insert sample films
INSERT INTO film (film_id, title, description, release_year, language_id, rental_duration, rental_rate, length, replacement_cost, rating, last_update) VALUES
(1, 'Pirates Adventure', 'A thrilling pirate adventure on the high seas', 2003, 1, 5, 2.99, 143, 20.99, 'PG-13', '2020-02-15 09:45:25'),
(2, 'The Great Performance', 'A masterful dramatic performance', 2012, 1, 3, 4.99, 121, 22.99, 'R', '2020-02-15 09:45:25'),
(3, 'Ocean Dreams', 'A romantic story set by the ocean', 2016, 1, 3, 3.99, 138, 19.99, 'PG-13', '2020-02-15 09:45:25'),
(4, 'Space Mission', 'An epic space adventure', 2019, 1, 4, 3.99, 156, 24.99, 'PG-13', '2020-02-15 09:45:25'),
(5, 'Comedy Central', 'A hilarious comedy adventure', 2018, 1, 3, 2.99, 98, 18.99, 'PG', '2020-02-15 09:45:25'),
(6, 'Family Time', 'A heartwarming family story', 2020, 1, 3, 3.99, 102, 19.99, 'G', '2020-02-15 09:45:25'),
(7, 'Mystery Solved', 'A thrilling mystery drama', 2017, 1, 4, 4.99, 134, 23.99, 'PG-13', '2020-02-15 09:45:25'),
(8, 'Future World', 'A sci-fi thriller about the future', 2021, 1, 5, 4.99, 167, 26.99, 'R', '2020-02-15 09:45:25');

-- Insert film-actor relationships
INSERT INTO film_actor (actor_id, film_id, last_update) VALUES
(1, 1, '2020-02-15 09:45:25'), -- Johnny Depp in Pirates Adventure
(2, 2, '2020-02-15 09:45:25'), -- Meryl Streep in The Great Performance
(3, 3, '2020-02-15 09:45:25'), -- Leonardo DiCaprio in Ocean Dreams
(4, 4, '2020-02-15 09:45:25'), -- Scarlett Johansson in Space Mission
(5, 5, '2020-02-15 09:45:25'), -- Tom Hanks in Comedy Central
(6, 6, '2020-02-15 09:45:25'), -- Emma Stone in Family Time
(7, 7, '2020-02-15 09:45:25'), -- Robert Downey Jr. in Mystery Solved
(8, 8, '2020-02-15 09:45:25'), -- Jennifer Lawrence in Future World
(1, 7, '2020-02-15 09:45:25'), -- Johnny Depp also in Mystery Solved
(3, 2, '2020-02-15 09:45:25'); -- Leonardo DiCaprio also in The Great Performance

-- Insert film-category relationships
INSERT INTO film_category (film_id, category_id, last_update) VALUES
(1, 1, '2020-02-15 09:45:25'), -- Pirates Adventure -> Action
(2, 3, '2020-02-15 09:45:25'), -- The Great Performance -> Drama
(3, 5, '2020-02-15 09:45:25'), -- Ocean Dreams -> Romance
(4, 6, '2020-02-15 09:45:25'), -- Space Mission -> Sci-Fi
(5, 2, '2020-02-15 09:45:25'), -- Comedy Central -> Comedy
(6, 8, '2020-02-15 09:45:25'), -- Family Time -> Family
(7, 3, '2020-02-15 09:45:25'), -- Mystery Solved -> Drama
(8, 6, '2020-02-15 09:45:25'); -- Future World -> Sci-Fi

-- Insert sample stores
INSERT INTO store (store_id, manager_staff_id, address_id, last_update) VALUES
(1, 1, 1, '2020-02-15 09:45:25'),
(2, 2, 2, '2020-02-15 09:45:25');

-- Insert sample staff
INSERT INTO staff (staff_id, first_name, last_name, address_id, email, store_id, active, username, last_update) VALUES
(1, 'Mike', 'Johnson', 1, 'mike@dvdrental.com', 1, TRUE, 'mike', '2020-02-15 09:45:25'),
(2, 'Sarah', 'Williams', 2, 'sarah@dvdrental.com', 2, TRUE, 'sarah', '2020-02-15 09:45:25'),
(3, 'James', 'Brown', 3, 'james@dvdrental.com', 1, TRUE, 'james', '2020-02-15 09:45:25');

-- Insert sample customers
INSERT INTO customer (customer_id, store_id, first_name, last_name, email, address_id, active, create_date, last_update) VALUES
(1, 1, 'John', 'Smith', 'john.smith@email.com', 1, TRUE, '2020-01-01', '2020-02-15 09:45:25'),
(2, 1, 'Jane', 'Doe', 'jane.doe@email.com', 2, TRUE, '2020-01-15', '2020-02-15 09:45:25'),
(3, 2, 'Bob', 'Wilson', 'bob.wilson@email.com', 3, TRUE, '2020-02-01', '2020-02-15 09:45:25'),
(4, 2, 'Alice', 'Davis', 'alice.davis@email.com', 4, TRUE, '2020-02-10', '2020-02-15 09:45:25'),
(5, 1, 'Charlie', 'Miller', 'charlie.miller@email.com', 5, TRUE, '2020-03-01', '2020-02-15 09:45:25');

-- Insert sample inventory
INSERT INTO inventory (inventory_id, film_id, store_id, last_update) VALUES
(1, 1, 1, '2020-02-15 09:45:25'),
(2, 1, 1, '2020-02-15 09:45:25'), -- Multiple copies
(3, 2, 1, '2020-02-15 09:45:25'),
(4, 3, 1, '2020-02-15 09:45:25'),
(5, 4, 2, '2020-02-15 09:45:25'),
(6, 5, 2, '2020-02-15 09:45:25'),
(7, 6, 1, '2020-02-15 09:45:25'),
(8, 7, 2, '2020-02-15 09:45:25'),
(9, 8, 1, '2020-02-15 09:45:25'),
(10, 2, 2, '2020-02-15 09:45:25'); -- Same film in different store

-- Insert sample rentals
INSERT INTO rental (rental_id, rental_date, inventory_id, customer_id, return_date, staff_id, last_update) VALUES
(1, '2020-05-24 22:53:30', 1, 1, '2020-05-26 22:04:30', 1, '2020-02-15 09:45:25'),
(2, '2020-05-24 23:03:39', 3, 2, '2020-05-28 19:40:33', 1, '2020-02-15 09:45:25'),
(3, '2020-05-25 00:00:40', 5, 3, '2020-05-27 01:12:04', 2, '2020-02-15 09:45:25'),
(4, '2020-05-25 00:02:21', 6, 4, '2020-05-26 02:26:17', 2, '2020-02-15 09:45:25'),
(5, '2020-05-25 00:09:02', 7, 5, NULL, 1, '2020-02-15 09:45:25'), -- Still rented
(6, '2020-05-25 00:22:55', 8, 1, '2020-05-27 05:33:08', 2, '2020-02-15 09:45:25'),
(7, '2020-05-25 01:15:17', 9, 2, NULL, 1, '2020-02-15 09:45:25'); -- Still rented

-- Insert sample payments
INSERT INTO payment (payment_id, customer_id, staff_id, rental_id, amount, payment_date) VALUES
(1, 1, 1, 1, 2.99, '2020-05-24 23:53:30'),
(2, 2, 1, 2, 4.99, '2020-05-25 00:03:39'),
(3, 3, 2, 3, 3.99, '2020-05-25 01:00:40'),
(4, 4, 2, 4, 2.99, '2020-05-25 01:02:21'),
(5, 5, 1, 5, 3.99, '2020-05-25 01:09:02'),
(6, 1, 2, 6, 4.99, '2020-05-25 01:22:55'),
(7, 2, 1, 7, 4.99, '2020-05-25 02:15:17');

\echo '✅ Sample data inserted successfully!'
\echo ''
\echo '3. Testing basic queries...'

\echo '3.1 Total number of films:'
SELECT COUNT(*) as total_films FROM film;

\echo ''
\echo '3.2 Films by category:'
SELECT c.name as category, COUNT(fc.film_id) as film_count
FROM category c
LEFT JOIN film_category fc ON c.category_id = fc.category_id
GROUP BY c.category_id, c.name
ORDER BY film_count DESC;

\echo ''
\echo '3.3 Top actors by number of films:'
SELECT a.first_name, a.last_name, COUNT(fa.film_id) as film_count
FROM actor a
LEFT JOIN film_actor fa ON a.actor_id = fa.actor_id
GROUP BY a.actor_id, a.first_name, a.last_name
ORDER BY film_count DESC;

\echo ''
\echo '3.4 Current rentals (not yet returned):'
SELECT 
    c.first_name || ' ' || c.last_name as customer_name,
    f.title as film_title,
    r.rental_date
FROM rental r
INNER JOIN customer c ON r.customer_id = c.customer_id
INNER JOIN inventory i ON r.inventory_id = i.inventory_id
INNER JOIN film f ON i.film_id = f.film_id
WHERE r.return_date IS NULL
ORDER BY r.rental_date;

\echo ''
\echo '3.5 Revenue by store:'
SELECT 
    s.store_id,
    COUNT(p.payment_id) as total_transactions,
    SUM(p.amount) as total_revenue
FROM payment p
INNER JOIN staff st ON p.staff_id = st.staff_id
INNER JOIN store s ON st.store_id = s.store_id
GROUP BY s.store_id
ORDER BY total_revenue DESC;

\echo ''
\echo '4. Testing complex JOINs...'

\echo '4.1 Film details with category and actors:'
SELECT 
    f.title,
    f.release_year,
    f.rating,
    c.name as category,
    a.first_name || ' ' || a.last_name as actor_name
FROM film f
LEFT JOIN film_category fc ON f.film_id = fc.film_id
LEFT JOIN category c ON fc.category_id = c.category_id
LEFT JOIN film_actor fa ON f.film_id = fa.film_id
LEFT JOIN actor a ON fa.actor_id = a.actor_id
ORDER BY f.title, actor_name;

\echo ''
\echo '4.2 Customer rental history with film details:'
SELECT 
    c.first_name || ' ' || c.last_name as customer_name,
    f.title as film_title,
    cat.name as category,
    r.rental_date,
    r.return_date,
    p.amount as payment_amount
FROM customer c
INNER JOIN rental r ON c.customer_id = r.customer_id
INNER JOIN inventory i ON r.inventory_id = i.inventory_id
INNER JOIN film f ON i.film_id = f.film_id
LEFT JOIN film_category fc ON f.film_id = fc.film_id
LEFT JOIN category cat ON fc.category_id = cat.category_id
LEFT JOIN payment p ON r.rental_id = p.rental_id
ORDER BY c.last_name, c.first_name, r.rental_date;

\echo ''
\echo '5. Testing information_schema integration...'

\echo '5.1 All tables in our DVD rental database:'
SELECT table_name, table_type
FROM information_schema.tables
WHERE table_schema = 'public'
ORDER BY table_name;

\echo ''
\echo '5.2 Table with most columns:'
SELECT 
    table_name,
    COUNT(*) as column_count
FROM information_schema.columns
WHERE table_schema = 'public'
GROUP BY table_name
ORDER BY column_count DESC
LIMIT 5;

\echo ''
\echo '5.3 All TEXT columns across all tables:'
SELECT 
    table_name,
    column_name,
    data_type
FROM information_schema.columns
WHERE table_schema = 'public'
AND data_type = 'text'
ORDER BY table_name, column_name;

\echo ''
\echo '✅ DVD Rental demo completed successfully!'
\echo ''
\echo '🎬 Summary:'
\echo '   ✅ Created 15 interconnected tables'
\echo '   ✅ Inserted realistic sample data'
\echo '   ✅ Demonstrated complex JOINs across multiple tables'
\echo '   ✅ Showed aggregate queries and grouping'
\echo '   ✅ Integrated with information_schema for metadata'
\echo '   ✅ Tested real-world DVD rental business logic'
\echo ''
\echo 'Try these additional queries:'
\echo '   SELECT * FROM film WHERE rating = '"'"'PG-13'"'"';'
\echo '   SELECT COUNT(*) FROM rental WHERE return_date IS NULL;'
\echo '   SELECT customer_id, SUM(amount) FROM payment GROUP BY customer_id;'
\echo '   SELECT f.title, COUNT(r.rental_id) as times_rented'
\echo '   FROM film f'
\echo '   LEFT JOIN inventory i ON f.film_id = i.film_id'
\echo '   LEFT JOIN rental r ON i.inventory_id = r.inventory_id'
\echo '   GROUP BY f.film_id, f.title'
\echo '   ORDER BY times_rented DESC;'