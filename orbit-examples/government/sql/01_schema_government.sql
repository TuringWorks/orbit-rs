CREATE TABLE IF NOT EXISTS applicants (
  applicant_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  age INT,
  income DECIMAL(12,2)
);
CREATE TABLE IF NOT EXISTS permits (
  permit_id SERIAL PRIMARY KEY,
  applicant_id INT REFERENCES applicants(applicant_id),
  type VARCHAR(100),
  fee DECIMAL(10,2),
  approved BOOLEAN DEFAULT FALSE
);
INSERT INTO applicants(name, age, income) VALUES
('Jane Doe', 34, 78000.00),
('John Smith', 29, 52000.00) ON CONFLICT DO NOTHING;
INSERT INTO permits(applicant_id, type, fee, approved) VALUES
(1, 'Construction', 250.00, TRUE),
(2, 'Food Service', 150.00, FALSE) ON CONFLICT DO NOTHING;
