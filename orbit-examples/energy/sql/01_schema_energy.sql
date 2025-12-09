CREATE TABLE IF NOT EXISTS grid_nodes (
  node_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  region VARCHAR(100)
);
CREATE TABLE IF NOT EXISTS meter_readings (
  reading_id SERIAL PRIMARY KEY,
  node_id INT REFERENCES grid_nodes(node_id),
  ts TIMESTAMP NOT NULL,
  voltage DECIMAL(6,2),
  load_kw DECIMAL(8,2)
);
CREATE TABLE IF NOT EXISTS outages (
  outage_id SERIAL PRIMARY KEY,
  node_id INT REFERENCES grid_nodes(node_id),
  duration_min INT,
  cause VARCHAR(100)
);
INSERT INTO grid_nodes(name, region) VALUES
('Substation A', 'North'),
('Substation B', 'South') ON CONFLICT DO NOTHING;
INSERT INTO meter_readings(node_id, ts, voltage, load_kw) VALUES
(1, NOW() - INTERVAL '2 hour', 230.0, 1200.0),
(1, NOW() - INTERVAL '1 hour', 228.5, 1350.0),
(2, NOW() - INTERVAL '2 hour', 231.2, 900.0),
(2, NOW() - INTERVAL '1 hour', 229.8, 980.0) ON CONFLICT DO NOTHING;
INSERT INTO outages(node_id, duration_min, cause) VALUES
(1, 30, 'Storm'),
(2, 15, 'Maintenance') ON CONFLICT DO NOTHING;
