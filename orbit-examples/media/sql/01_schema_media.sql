CREATE TABLE IF NOT EXISTS users (
  user_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  age INT
);
CREATE TABLE IF NOT EXISTS content (
  content_id SERIAL PRIMARY KEY,
  title VARCHAR(300) NOT NULL,
  category VARCHAR(100)
);
CREATE TABLE IF NOT EXISTS interactions (
  interaction_id SERIAL PRIMARY KEY,
  user_id INT REFERENCES users(user_id),
  content_id INT REFERENCES content(content_id),
  watch_time_min INT,
  liked BOOLEAN DEFAULT FALSE
);
INSERT INTO users(name, age) VALUES
('User A', 25),
('User B', 32) ON CONFLICT DO NOTHING;
INSERT INTO content(title, category) VALUES
('ML Tutorial', 'Education'),
('Sci-Fi Movie', 'Entertainment') ON CONFLICT DO NOTHING;
INSERT INTO interactions(user_id, content_id, watch_time_min, liked) VALUES
(1, 1, 40, TRUE),
(1, 2, 10, FALSE),
(2, 2, 90, TRUE) ON CONFLICT DO NOTHING;
