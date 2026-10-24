# DML: upsert, UPDATE FROM, DELETE USING, INSERT SELECT, CTE DML, MERGE, RETURNING.

DROP TABLE IF EXISTS cf13_stock

DROP TABLE IF EXISTS cf13_incoming

CREATE TABLE cf13_stock (sku text PRIMARY KEY, qty int NOT NULL DEFAULT 0, note text)

CREATE TABLE cf13_incoming (sku text, qty int)

INSERT INTO cf13_stock VALUES ('a', 10, 'x'), ('b', 5, NULL), ('c', 0, 'z')

INSERT INTO cf13_incoming VALUES ('a', 5), ('b', 1), ('d', 7)

INSERT INTO cf13_stock (sku, qty) VALUES ('a', 99) ON CONFLICT (sku) DO NOTHING

SELECT sku, qty FROM cf13_stock ORDER BY sku

INSERT INTO cf13_stock (sku, qty) VALUES ('a', 99) ON CONFLICT (sku) DO UPDATE SET qty = cf13_stock.qty + excluded.qty

SELECT sku, qty FROM cf13_stock ORDER BY sku

INSERT INTO cf13_stock (sku, qty) VALUES ('a', 1), ('e', 2) ON CONFLICT (sku) DO UPDATE SET qty = excluded.qty WHERE cf13_stock.qty > 50 RETURNING sku, qty

UPDATE cf13_stock s SET qty = s.qty + i.qty FROM cf13_incoming i WHERE s.sku = i.sku

SELECT sku, qty FROM cf13_stock ORDER BY sku

INSERT INTO cf13_stock (sku, qty) SELECT sku, qty FROM cf13_incoming WHERE sku NOT IN (SELECT sku FROM cf13_stock)

SELECT sku, qty FROM cf13_stock ORDER BY sku

DELETE FROM cf13_stock s USING cf13_incoming i WHERE s.sku = i.sku AND i.qty > 6

SELECT sku FROM cf13_stock ORDER BY sku

UPDATE cf13_stock SET qty = qty * 2, note = 'bumped' WHERE qty > 5 RETURNING sku, qty, note

UPDATE cf13_stock SET (qty, note) = (1, 'one') WHERE sku = 'c'

SELECT sku, qty, note FROM cf13_stock WHERE sku = 'c'

DELETE FROM cf13_stock WHERE qty = 1 RETURNING sku

UPDATE cf13_stock SET qty = NULL WHERE sku = 'b'

UPDATE cf13_stock SET sku = 'a' WHERE sku = 'b'

DELETE FROM cf13_stock WHERE sku = 'zzz'

WITH moved AS (DELETE FROM cf13_stock WHERE sku = 'e' RETURNING sku, qty) INSERT INTO cf13_incoming SELECT sku, qty FROM moved

SELECT sku, qty FROM cf13_incoming ORDER BY sku, qty

WITH ins AS (INSERT INTO cf13_stock VALUES ('m', 1, 'cte') RETURNING sku) SELECT count(*) FROM ins

INSERT INTO cf13_stock VALUES ('only')

INSERT INTO cf13_stock VALUES ('n', 1, 'a', 'extra')

INSERT INTO cf13_stock (sku, nope) VALUES ('n', 1)

INSERT INTO cf13_stock DEFAULT VALUES

INSERT INTO cf13_stock (sku) VALUES ('def') RETURNING qty

MERGE INTO cf13_stock t USING cf13_incoming s ON t.sku = s.sku WHEN MATCHED THEN UPDATE SET qty = t.qty + s.qty WHEN NOT MATCHED THEN INSERT (sku, qty) VALUES (s.sku, s.qty)

SELECT sku, qty FROM cf13_stock ORDER BY sku

MERGE INTO cf13_stock t USING cf13_incoming s ON t.sku = s.sku WHEN MATCHED AND s.qty > 100 THEN DELETE WHEN MATCHED THEN DO NOTHING

SELECT count(*) FROM cf13_stock

INSERT INTO cf13_stock SELECT 'bulk' || g, g, NULL FROM generate_series(1, 5) g

SELECT count(*), sum(qty) FROM cf13_stock WHERE sku LIKE 'bulk%'

DELETE FROM cf13_stock WHERE sku LIKE 'bulk%'

TRUNCATE cf13_incoming

SELECT count(*) FROM cf13_incoming

DROP TABLE cf13_stock, cf13_incoming
