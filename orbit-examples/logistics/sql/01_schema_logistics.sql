CREATE TABLE IF NOT EXISTS facilities (
  facility_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  region VARCHAR(100)
);
CREATE TABLE IF NOT EXISTS shipments (
  shipment_id SERIAL PRIMARY KEY,
  origin_id INT REFERENCES facilities(facility_id),
  destination_id INT REFERENCES facilities(facility_id),
  weight_kg DECIMAL(10,2),
  distance_km DECIMAL(10,2),
  delivered BOOLEAN DEFAULT FALSE,
  delay_hours INT
);
INSERT INTO facilities(name, region) VALUES
('DC West', 'West'),
('Store 101', 'West') ON CONFLICT DO NOTHING;
INSERT INTO shipments(origin_id, destination_id, weight_kg, distance_km, delivered, delay_hours) VALUES
(1, 2, 500.0, 120.0, TRUE, 2),
(1, 2, 300.0, 120.0, FALSE, 12) ON CONFLICT DO NOTHING;
