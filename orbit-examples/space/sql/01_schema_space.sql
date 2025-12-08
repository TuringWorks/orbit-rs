CREATE TABLE IF NOT EXISTS satellites (
  sat_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  orbit_type VARCHAR(50),
  age_years INT
);
CREATE TABLE IF NOT EXISTS telemetry (
  telem_id SERIAL PRIMARY KEY,
  sat_id INT REFERENCES satellites(sat_id),
  ts TIMESTAMP NOT NULL,
  temp_c DECIMAL(6,2),
  power_w DECIMAL(8,2),
  vibration DECIMAL(6,3)
);
INSERT INTO satellites(name, orbit_type, age_years) VALUES
('Orbiter-1', 'LEO', 5),
('Surveyor-2', 'GEO', 8) ON CONFLICT DO NOTHING;
INSERT INTO telemetry(sat_id, ts, temp_c, power_w, vibration) VALUES
(1, NOW() - INTERVAL '2 hour', 22.3, 120.0, 0.012),
(1, NOW() - INTERVAL '1 hour', 23.0, 118.5, 0.020),
(2, NOW() - INTERVAL '2 hour', 30.5, 250.0, 0.008),
(2, NOW() - INTERVAL '1 hour', 31.2, 248.0, 0.010) ON CONFLICT DO NOTHING;
