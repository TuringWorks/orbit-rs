# Aggregates, GROUP BY variants, and window functions.

DROP TABLE IF EXISTS cf11_sales

CREATE TABLE cf11_sales (id int PRIMARY KEY, region text, product text, qty int, price numeric(8,2), sold date)

INSERT INTO cf11_sales VALUES (1,'east','apple',10,1.50,'2026-01-05'), (2,'east','pear',5,2.00,'2026-01-06'), (3,'west','apple',7,1.50,'2026-01-07'), (4,'west','fig',3,4.00,'2026-02-01'), (5,'north','apple',NULL,1.50,'2026-02-02'), (6,'east','apple',2,1.75,'2026-02-03')

SELECT count(*), count(qty), count(DISTINCT product), sum(qty), avg(qty), min(qty), max(qty) FROM cf11_sales

SELECT region, count(*), sum(qty) FROM cf11_sales GROUP BY region ORDER BY region

SELECT region, product, sum(qty * price) FROM cf11_sales GROUP BY region, product ORDER BY 1, 2

SELECT region, sum(qty) FROM cf11_sales GROUP BY region HAVING sum(qty) > 8 ORDER BY region

SELECT product, count(*) FILTER (WHERE region = 'east'), sum(qty) FILTER (WHERE qty > 3) FROM cf11_sales GROUP BY product ORDER BY product

SELECT region, string_agg(product, ',' ORDER BY product), array_agg(id ORDER BY id) FROM cf11_sales GROUP BY region ORDER BY region

SELECT bool_and(qty > 0), bool_or(qty > 9), every(price > 1) FROM cf11_sales

SELECT sum(qty), sum(qty) IS NULL FROM cf11_sales WHERE id > 100

SELECT count(*) FROM cf11_sales WHERE id > 100

SELECT avg(price), round(avg(price), 2), stddev_pop(price) > 0, variance(price) > 0 FROM cf11_sales

SELECT region, sum(qty) FROM cf11_sales GROUP BY ROLLUP (region) ORDER BY region NULLS LAST

SELECT region, product, count(*) FROM cf11_sales GROUP BY GROUPING SETS ((region), (product), ()) ORDER BY region NULLS LAST, product NULLS LAST

SELECT id, region, row_number() OVER (PARTITION BY region ORDER BY id) FROM cf11_sales ORDER BY id

SELECT id, rank() OVER (ORDER BY price), dense_rank() OVER (ORDER BY price) FROM cf11_sales ORDER BY id

SELECT id, sum(qty) OVER (ORDER BY id), sum(qty) OVER (PARTITION BY region ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) FROM cf11_sales ORDER BY id

SELECT id, lag(qty) OVER (ORDER BY id), lead(qty, 2, -1) OVER (ORDER BY id), first_value(id) OVER (PARTITION BY region ORDER BY id), last_value(id) OVER (PARTITION BY region ORDER BY id ROWS BETWEEN UNBOUNDED PRECEDING AND UNBOUNDED FOLLOWING) FROM cf11_sales ORDER BY id

SELECT id, ntile(3) OVER (ORDER BY id), percent_rank() OVER (ORDER BY id), cume_dist() OVER (ORDER BY id) FROM cf11_sales ORDER BY id

SELECT id, avg(price) OVER w, count(*) OVER w FROM cf11_sales WINDOW w AS (PARTITION BY region) ORDER BY id

SELECT DISTINCT region FROM cf11_sales ORDER BY region

SELECT DISTINCT ON (region) region, id FROM cf11_sales ORDER BY region, id DESC

SELECT region, qty FROM cf11_sales ORDER BY qty DESC NULLS LAST, id LIMIT 3

SELECT region, qty FROM cf11_sales ORDER BY qty NULLS FIRST, id LIMIT 2 OFFSET 1

SELECT id FROM cf11_sales ORDER BY id FETCH FIRST 2 ROWS ONLY

SELECT percentile_cont(0.5) WITHIN GROUP (ORDER BY price), mode() WITHIN GROUP (ORDER BY product) FROM cf11_sales

DROP TABLE cf11_sales
