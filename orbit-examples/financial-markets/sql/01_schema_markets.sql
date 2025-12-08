CREATE TABLE IF NOT EXISTS symbols (
  symbol_id SERIAL PRIMARY KEY,
  ticker VARCHAR(20) UNIQUE NOT NULL,
  sector VARCHAR(100)
);
CREATE TABLE IF NOT EXISTS prices (
  price_id SERIAL PRIMARY KEY,
  symbol_id INT REFERENCES symbols(symbol_id),
  ts TIMESTAMP NOT NULL,
  close DECIMAL(12,4),
  volume BIGINT
);
CREATE TABLE IF NOT EXISTS indicators (
  indicator_id SERIAL PRIMARY KEY,
  symbol_id INT REFERENCES symbols(symbol_id),
  ts TIMESTAMP NOT NULL,
  volatility DECIMAL(8,4),
  momentum DECIMAL(8,4),
  rsi DECIMAL(6,2)
);
INSERT INTO symbols(ticker, sector) VALUES
('ORBT', 'Technology'),
('UTIL', 'Utilities') ON CONFLICT DO NOTHING;
INSERT INTO prices(symbol_id, ts, close, volume) VALUES
(1, NOW() - INTERVAL '2 day', 100.50, 1000000),
(1, NOW() - INTERVAL '1 day', 103.20, 1200000),
(2, NOW() - INTERVAL '2 day', 50.00, 800000),
(2, NOW() - INTERVAL '1 day', 51.25, 850000) ON CONFLICT DO NOTHING;
INSERT INTO indicators(symbol_id, ts, volatility, momentum, rsi) VALUES
(1, NOW() - INTERVAL '1 day', 0.25, 0.10, 55.0),
(2, NOW() - INTERVAL '1 day', 0.12, 0.04, 48.0) ON CONFLICT DO NOTHING;
