# JSON/JSONB and arrays.

SELECT '{"a": 1, "b": {"c": [1,2,3]}}'::jsonb -> 'a', '{"a": 1, "b": {"c": [1,2,3]}}'::jsonb ->> 'a', '{"a": 1, "b": {"c": [1,2,3]}}'::jsonb -> 'b' -> 'c' -> 1, '{"a": 1, "b": {"c": [1,2,3]}}'::jsonb #> '{b,c,0}', '{"a": 1, "b": {"c": [1,2,3]}}'::jsonb #>> '{b,c,2}'

SELECT '{"a": 1}'::jsonb ? 'a', '{"a": 1}'::jsonb ? 'z', '{"a": 1, "b": 2}'::jsonb ?& array['a','b'], '{"a": 1}'::jsonb ?| array['z','a']

SELECT '{"a": 1, "b": 2}'::jsonb @> '{"a": 1}'::jsonb, '{"a": 1}'::jsonb <@ '{"a": 1, "b": 2}'::jsonb

SELECT '{"a": 1}'::jsonb || '{"b": 2}'::jsonb, '{"a": 1, "b": 2}'::jsonb - 'a', '[1,2,3]'::jsonb - 1

SELECT jsonb_build_object('a', 1, 'b', 'x'), jsonb_build_array(1, 'two', null), to_jsonb(ARRAY[1,2]), to_jsonb('s'::text)

SELECT jsonb_typeof('1'::jsonb), jsonb_typeof('"s"'::jsonb), jsonb_typeof('[]'::jsonb), jsonb_typeof('{}'::jsonb), jsonb_typeof('null'::jsonb)

SELECT jsonb_array_length('[1,2,3]'::jsonb), jsonb_extract_path_text('{"a": {"b": "x"}}'::jsonb, 'a', 'b')

SELECT key, value FROM jsonb_each('{"a": 1, "b": "x"}'::jsonb) ORDER BY key

SELECT key, value FROM jsonb_each_text('{"a": 1, "b": "x"}'::jsonb) ORDER BY key

SELECT jsonb_array_elements('[1,2,3]'::jsonb)

SELECT jsonb_object_keys('{"a": 1, "b": 2}'::jsonb)

SELECT jsonb_set('{"a": 1}'::jsonb, '{b}', '2'), jsonb_set('{"a": {"b": 1}}'::jsonb, '{a,b}', '9'), jsonb_insert('[1,3]'::jsonb, '{1}', '2')

SELECT jsonb_strip_nulls('{"a": null, "b": 1}'::jsonb), jsonb_pretty('{"a":1}'::jsonb) LIKE '%a%'

SELECT '{"a": 1, "b": [true, null]}'::jsonb::text, '{"a":1,"a":2}'::jsonb, '{"b":1,"a":2}'::jsonb

SELECT '{"a": 1}'::json::jsonb = '{"a": 1}'::jsonb, '[1, 2]'::jsonb = '[1,2]'::jsonb

SELECT '{"a": 1}'::jsonb @? '$.a', jsonb_path_query('{"a": [1,2,3]}'::jsonb, '$.a[*] ? (@ > 1)')

SELECT '{bad'::jsonb

DROP TABLE IF EXISTS cf16_docs

CREATE TABLE cf16_docs (id int PRIMARY KEY, doc jsonb)

INSERT INTO cf16_docs VALUES (1, '{"name": "ann", "tags": ["a","b"], "n": 5}'), (2, '{"name": "bob", "tags": ["b"], "n": 7}'), (3, '{"name": "cy"}')

SELECT id, doc ->> 'name' FROM cf16_docs WHERE doc @> '{"tags": ["b"]}' ORDER BY id

SELECT id FROM cf16_docs WHERE (doc ->> 'n')::int > 5 ORDER BY id

SELECT id FROM cf16_docs WHERE doc ? 'tags' ORDER BY id

SELECT id, jsonb_array_length(doc -> 'tags') FROM cf16_docs WHERE doc ? 'tags' ORDER BY id

CREATE INDEX cf16_docs_gin ON cf16_docs USING gin (doc)

UPDATE cf16_docs SET doc = jsonb_set(doc, '{n}', '10') WHERE id = 1

SELECT doc -> 'n' FROM cf16_docs WHERE id = 1

SELECT ARRAY[1,2,3] || ARRAY[4], ARRAY[1,2,3] || 4, 0 || ARRAY[1], array_cat(ARRAY[1], ARRAY[2]), array_append(ARRAY[1], 2), array_prepend(0, ARRAY[1])

SELECT (ARRAY[10,20,30])[2], (ARRAY[10,20,30])[2:3], array_length(ARRAY[1,2,3], 1), cardinality(ARRAY[1,2,3]), array_ndims(ARRAY[[1,2],[3,4]]), array_dims(ARRAY[1,2,3])

SELECT 2 = ANY(ARRAY[1,2,3]), 5 = ANY(ARRAY[1,2,3]), 1 = ALL(ARRAY[1,1]), 3 > ALL(ARRAY[1,2])

SELECT ARRAY[1,2,3] @> ARRAY[2], ARRAY[1,2] <@ ARRAY[1,2,3], ARRAY[1,2] && ARRAY[2,3], ARRAY[1,2] = ARRAY[1,2]

SELECT array_position(ARRAY['a','b','c'], 'b'), array_remove(ARRAY[1,2,1], 1), array_replace(ARRAY[1,2,1], 1, 9)

SELECT unnest(ARRAY[3,1,2])

SELECT unnest(ARRAY[1,2]), unnest(ARRAY['a','b'])

SELECT * FROM unnest(ARRAY[1,2,3]) WITH ORDINALITY

SELECT array_agg(x ORDER BY x DESC) FROM unnest(ARRAY[3,1,2]) x

SELECT ARRAY(SELECT g FROM generate_series(1,3) g), ARRAY[]::int[], ARRAY[NULL]::int[]

SELECT '{1,2,3}'::int[] , '{{1,2},{3,4}}'::int[][], '{a,"b c",NULL}'::text[]

SELECT array_to_string(ARRAY[1,NULL,3], ',', '*'), string_to_array('a,b,,c', ','), string_to_array('a,b', ',', 'b')

SELECT ARRAY[1,2,3][0], ARRAY[1,2,3][4], (ARRAY[]::int[])[1]

DROP TABLE cf16_docs
