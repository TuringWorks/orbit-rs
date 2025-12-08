CREATE TABLE IF NOT EXISTS missions (
  mission_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  mission_type VARCHAR(50) NOT NULL,
  duration_hours INT,
  risk_level INT
);
CREATE TABLE IF NOT EXISTS assets (
  asset_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  asset_type VARCHAR(50) NOT NULL,
  age_years INT
);
CREATE TABLE IF NOT EXISTS incidents (
  incident_id SERIAL PRIMARY KEY,
  asset_id INT REFERENCES assets(asset_id),
  severity INT,
  ts TIMESTAMP NOT NULL
);
INSERT INTO missions(name, mission_type, duration_hours, risk_level) VALUES
('Horizon Shield', 'Recon', 12, 2),
('Iron Path', 'Convoy', 36, 3) ON CONFLICT DO NOTHING;
INSERT INTO assets(name, asset_type, age_years) VALUES
('Falcon-1', 'UAV', 3),
('Atlas-7', 'Armored', 8) ON CONFLICT DO NOTHING;
INSERT INTO incidents(asset_id, severity, ts) VALUES
(1, 1, NOW() - INTERVAL '2 day'),
(1, 2, NOW() - INTERVAL '1 day'),
(2, 3, NOW() - INTERVAL '3 day') ON CONFLICT DO NOTHING;
