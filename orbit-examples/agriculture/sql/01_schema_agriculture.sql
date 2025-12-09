CREATE TABLE IF NOT EXISTS farms (
    farm_id SERIAL PRIMARY KEY,
    name VARCHAR(200) NOT NULL,
    location VARCHAR(200) NOT NULL
);
CREATE TABLE IF NOT EXISTS fields (
    field_id SERIAL PRIMARY KEY,
    farm_id INT REFERENCES farms(farm_id),
    crop VARCHAR(100) NOT NULL,
    area_ha DECIMAL(10,2),
    soil_ph DECIMAL(4,2)
);
CREATE TABLE IF NOT EXISTS sensor_readings (
    reading_id SERIAL PRIMARY KEY,
    farm_id INT REFERENCES farms(farm_id),
    ts TIMESTAMP NOT NULL,
    moisture DECIMAL(5,2),
    temperature DECIMAL(5,2)
);
CREATE TABLE IF NOT EXISTS yields (
    yield_id SERIAL PRIMARY KEY,
    farm_id INT REFERENCES farms(farm_id),
    season VARCHAR(20),
    total_yield_tons DECIMAL(10,2)
);
INSERT INTO farms(name, location) VALUES
('Green Valley', 'Iowa'),
('Sunrise Acres', 'Nebraska');
INSERT INTO fields(farm_id, crop, area_ha, soil_ph) VALUES
(1, 'Corn', 120.50, 6.5),
(1, 'Soy', 80.00, 6.3),
(2, 'Wheat', 95.20, 6.7);
INSERT INTO sensor_readings(farm_id, ts, moisture, temperature) VALUES
(1, NOW() - INTERVAL '3 day', 22.5, 18.0),
(1, NOW() - INTERVAL '2 day', 24.1, 19.2),
(1, NOW() - INTERVAL '1 day', 23.0, 20.0),
(2, NOW() - INTERVAL '3 day', 18.4, 17.0),
(2, NOW() - INTERVAL '2 day', 19.2, 17.8),
(2, NOW() - INTERVAL '1 day', 20.0, 18.5);
INSERT INTO yields(farm_id, season, total_yield_tons) VALUES
(1, '2024', 450.0),
(2, '2024', 370.0);
