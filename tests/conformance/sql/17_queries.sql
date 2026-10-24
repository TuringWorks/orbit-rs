# Subqueries, set operations, joins, CTEs, LATERAL, generate_series.

DROP TABLE IF EXISTS cf17_emp

DROP TABLE IF EXISTS cf17_dept

CREATE TABLE cf17_dept (id int PRIMARY KEY, name text)

CREATE TABLE cf17_emp (id int PRIMARY KEY, name text, dept_id int, boss_id int, salary int)

INSERT INTO cf17_dept VALUES (1, 'eng'), (2, 'ops'), (3, 'empty')

INSERT INTO cf17_emp VALUES (1,'ann',1,NULL,100), (2,'bob',1,1,80), (3,'cy',2,1,70), (4,'dee',2,3,60), (5,'eve',NULL,1,50)

SELECT e.name, d.name FROM cf17_emp e INNER JOIN cf17_dept d ON e.dept_id = d.id ORDER BY e.id

SELECT e.name, d.name FROM cf17_emp e LEFT JOIN cf17_dept d ON e.dept_id = d.id ORDER BY e.id

SELECT e.name, d.name FROM cf17_emp e RIGHT JOIN cf17_dept d ON e.dept_id = d.id ORDER BY d.id, e.id

SELECT e.name, d.name FROM cf17_emp e FULL JOIN cf17_dept d ON e.dept_id = d.id ORDER BY e.id NULLS LAST, d.id

SELECT count(*) FROM cf17_emp CROSS JOIN cf17_dept

SELECT e.name, b.name AS boss FROM cf17_emp e LEFT JOIN cf17_emp b ON e.boss_id = b.id ORDER BY e.id

SELECT name FROM cf17_emp NATURAL JOIN (SELECT id, 'x' AS tag FROM cf17_dept) d ORDER BY name

SELECT name FROM cf17_emp JOIN cf17_dept USING (id) ORDER BY name

SELECT name FROM cf17_emp WHERE salary > (SELECT avg(salary) FROM cf17_emp) ORDER BY name

SELECT name FROM cf17_emp e WHERE salary = (SELECT max(salary) FROM cf17_emp WHERE dept_id = e.dept_id) ORDER BY name

SELECT name FROM cf17_dept d WHERE EXISTS (SELECT 1 FROM cf17_emp e WHERE e.dept_id = d.id) ORDER BY name

SELECT name FROM cf17_dept d WHERE NOT EXISTS (SELECT 1 FROM cf17_emp e WHERE e.dept_id = d.id)

SELECT name, (SELECT count(*) FROM cf17_emp e WHERE e.dept_id = d.id) FROM cf17_dept d ORDER BY d.id

SELECT name FROM cf17_emp WHERE dept_id IN (SELECT id FROM cf17_dept WHERE name = 'ops') ORDER BY name

SELECT name FROM cf17_emp WHERE salary > ALL (SELECT salary FROM cf17_emp WHERE dept_id = 2) ORDER BY name

SELECT name FROM cf17_emp WHERE salary > ANY (SELECT salary FROM cf17_emp WHERE dept_id = 2) ORDER BY name

SELECT id FROM cf17_emp UNION SELECT id FROM cf17_dept ORDER BY id

SELECT dept_id FROM cf17_emp UNION ALL SELECT id FROM cf17_dept ORDER BY 1 NULLS LAST

SELECT id FROM cf17_dept INTERSECT SELECT dept_id FROM cf17_emp ORDER BY id

SELECT id FROM cf17_dept EXCEPT SELECT dept_id FROM cf17_emp

SELECT id FROM cf17_dept EXCEPT ALL SELECT dept_id FROM cf17_emp ORDER BY id

WITH RECURSIVE chain AS (SELECT id, name, 0 AS depth FROM cf17_emp WHERE boss_id IS NULL UNION ALL SELECT e.id, e.name, c.depth + 1 FROM cf17_emp e JOIN chain c ON e.boss_id = c.id) SELECT name, depth FROM chain ORDER BY depth, name

WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 5) SELECT sum(i) FROM n

WITH a AS (SELECT dept_id, sum(salary) AS s FROM cf17_emp GROUP BY dept_id), b AS (SELECT max(s) AS m FROM a) SELECT a.dept_id FROM a, b WHERE a.s = b.m

WITH m AS MATERIALIZED (SELECT * FROM cf17_emp WHERE salary > 60) SELECT count(*) FROM m

SELECT d.name, e.name FROM cf17_dept d CROSS JOIN LATERAL (SELECT name FROM cf17_emp WHERE dept_id = d.id ORDER BY salary DESC LIMIT 1) e ORDER BY d.id

SELECT d.name, x.cnt FROM cf17_dept d LEFT JOIN LATERAL (SELECT count(*) AS cnt FROM cf17_emp WHERE dept_id = d.id) x ON true ORDER BY d.id

SELECT g, g * g FROM generate_series(1, 5) g

SELECT g FROM generate_series(10, 1, -3) g

SELECT g FROM generate_series(1, 0) g

SELECT * FROM generate_series(1, 3) WITH ORDINALITY AS t(v, n)

SELECT x, y FROM (VALUES (1, 'a'), (2, 'b')) AS t(x, y) ORDER BY x

VALUES (1, 'a'), (2, 'b') ORDER BY 1 DESC

SELECT * FROM (SELECT id, salary FROM cf17_emp) sub WHERE salary > 60 ORDER BY id

SELECT t.* FROM (SELECT 1 AS a, 2 AS b) t

SELECT name, salary FROM cf17_emp ORDER BY salary DESC LIMIT 2

SELECT name FROM cf17_emp ORDER BY 1 LIMIT ALL OFFSET 3

SELECT name FROM cf17_emp WHERE name ~ '^[a-c]' ORDER BY name

SELECT dept_id, count(*) FROM cf17_emp GROUP BY dept_id ORDER BY dept_id NULLS FIRST

SELECT 1 AS one, 2 two, 'three' "Three"

SELECT e.* , d.* FROM cf17_emp e JOIN cf17_dept d ON d.id = e.dept_id WHERE e.id = 1

SELECT * FROM cf17_emp WHERE id = ANY(ARRAY[1,3]) ORDER BY id

SELECT * FROM cf17_emp WHERE (dept_id, salary) IN ((1, 100), (2, 60)) ORDER BY id

SELECT * FROM cf17_emp WHERE name IS DISTINCT FROM 'ann' AND boss_id IS NOT DISTINCT FROM 1 ORDER BY id

SELECT count(*) FROM cf17_emp TABLESAMPLE BERNOULLI (100)

TABLE cf17_dept ORDER BY id

SELECT ALL name FROM cf17_dept ORDER BY 1

SELECT id INTO TEMP cf17_into FROM cf17_dept

SELECT count(*) FROM cf17_into

DROP TABLE cf17_emp, cf17_dept
