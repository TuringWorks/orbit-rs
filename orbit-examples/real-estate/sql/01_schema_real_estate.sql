CREATE TABLE IF NOT EXISTS listings (
  listing_id SERIAL PRIMARY KEY,
  address VARCHAR(300) NOT NULL,
  bedrooms INT,
  bathrooms DECIMAL(3,1),
  area_sqft INT,
  year_built INT,
  price DECIMAL(12,2)
);
INSERT INTO listings(address, bedrooms, bathrooms, area_sqft, year_built, price) VALUES
('123 Oak St', 3, 2.0, 1500, 1995, 350000.00),
('45 Pine Ave', 4, 2.5, 2200, 2005, 520000.00) ON CONFLICT DO NOTHING;
