# Types, literals, and casts.

SELECT 1::smallint, 2::int, 3::bigint, 4.5::numeric(10,2), 5.5::real, 6.5::double precision

SELECT '42'::int + 1, '3.14'::numeric, '2.5'::float8, 't'::boolean, 'no'::bool

SELECT 'abc'::varchar(2)

SELECT 'abc'::char(5) || '|'

SELECT 10::numeric / 4, 10 / 4, 10.0 / 4.0, 10 % 3, -7 % 3, 2 ^ 10, 2.0 ^ 0.5

SELECT 7::int4 / 2::int4, (-7)::int / 2, abs(-7), sign(-2.5), ceil(2.1), floor(-2.1), round(2.5), round(3.5), round(2.567, 2), trunc(2.99)

SELECT 1 = 1, 1 <> 2, 1 < NULL, NULL IS NULL, NULL = NULL, NULL IS NOT DISTINCT FROM NULL, true AND NULL, false AND NULL, true OR NULL

SELECT 'a' < 'b', 'a' < 'B', 'abc' = 'ABC', lower('ABC') = 'abc'

SELECT 2147483647::int, 2147483648::bigint, 9223372036854775807::bigint

SELECT (2147483647::int + 1)

SELECT 'abc'::int

SELECT '1e3'::float8, '1.5e-2'::numeric, 'NaN'::float8, 'Infinity'::float8

SELECT 0.1 + 0.2, 0.1::float8 + 0.2::float8, 1.0 * 3, 1.50 + 1.5

SELECT date '2026-03-15', timestamp '2026-03-15 10:30:00', time '10:30:00', interval '1 day 2 hours'

SELECT date '2026-03-15' + 10, date '2026-03-15' - date '2026-01-01', timestamp '2026-03-15 10:30:00' + interval '90 minutes'

SELECT '2026-03-15'::date, '2026-03-15 10:30:00'::timestamp, '10:30'::time

SELECT '2026-02-30'::date

SELECT true::int, false::int, 1::boolean, 0::boolean

SELECT 'x'::text, 'x'::text || 1, 1 || 2, 'a' || NULL

SELECT '550e8400-e29b-41d4-a716-446655440000'::uuid

SELECT '{"a": 1}'::json, '{"a": 1}'::jsonb, '[1,2,3]'::jsonb

SELECT '\x414243'::bytea

SELECT ARRAY[1,2,3], ARRAY['a','b'], ARRAY[1,2,3]::text[], '{1,2,3}'::int[]

SELECT pg_typeof(1), pg_typeof(1.5), pg_typeof('a'), pg_typeof(true), pg_typeof(1::bigint), pg_typeof(now()::date)

SELECT pg_typeof(1 + 1.5), pg_typeof(1 + 1::bigint), pg_typeof(1.5 + 1::float8)

SELECT CAST(1.9 AS int), CAST(-1.9 AS int), CAST('7' AS bigint), CAST(65 AS text)

SELECT 5 BETWEEN 1 AND 10, 5 NOT BETWEEN 1 AND 4, 'b' BETWEEN 'a' AND 'c'

SELECT 3 IN (1,2,3), 4 NOT IN (1,2,3), NULL IN (1,2), 1 IN (1, NULL), 2 NOT IN (1, NULL)

SELECT CASE 2 WHEN 1 THEN 'one' WHEN 2 THEN 'two' ELSE 'many' END, CASE WHEN false THEN 1 END

SELECT coalesce(NULL, 2), nullif(5, 5), nullif(5, 6), greatest(1, 5, 3), least(4, 2, 9)
