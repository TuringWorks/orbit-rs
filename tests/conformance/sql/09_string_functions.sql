# String functions and operators.

SELECT length('hello'), char_length('héllo'), octet_length('héllo'), upper('abc'), lower('ABC'), initcap('hello world')

SELECT substring('postgres' from 2 for 3), substring('postgres', 3), substr('postgres', 1, 4), left('postgres', 4), right('postgres', 3)

SELECT position('gre' in 'postgres'), strpos('postgres', 'g'), strpos('postgres', 'z')

SELECT trim('  x  ') || '|', ltrim('  x') || '|', rtrim('x  ') || '|', trim(both 'x' from 'xxhixx'), btrim('xxhixx', 'x')

SELECT lpad('7', 3, '0'), rpad('7', 3, '*'), lpad('abcd', 2), repeat('ab', 3), reverse('abc')

SELECT replace('aXbXc', 'X', '-'), translate('12345', '143', 'ax'), overlay('Txxxxas' placing 'hom' from 2 for 4)

SELECT concat('a', NULL, 'b'), concat_ws('-', 'a', NULL, 'b'), 'a' || 'b' || 'c'

SELECT split_part('a,b,c', ',', 2), split_part('a,b,c', ',', 4) = ''

SELECT string_to_array('a,b,c', ','), array_to_string(ARRAY['a','b','c'], '|')

SELECT 'abc' LIKE 'a%', 'abc' LIKE '_b_', 'abc' LIKE 'A%', 'abc' ILIKE 'A%', 'abc' NOT LIKE 'x%', 'a%c' LIKE 'a\%c'

SELECT 'abc' ~ '^a', 'abc' ~ '^b', 'abc' ~* '^A', 'abc' !~ 'z'

SELECT regexp_replace('hello world', 'o', '0'), regexp_replace('hello world', 'o', '0', 'g'), regexp_match('abc123', '[0-9]+')

SELECT ascii('A'), chr(66), md5('abc'), encode('abc'::bytea, 'hex'), decode('616263', 'hex')

SELECT format('%s-%s', 'a', 'b'), format('%5s|%-5s|', 'ab', 'cd'), format('%I', 'Mixed Case'), format('%L', 'it''s')

SELECT quote_ident('a b'), quote_literal('it''s'), quote_nullable(NULL)

SELECT starts_with('postgres', 'post'), 'postgres' ^@ 'post'

SELECT to_char(1234.5, '9999.99'), to_char(7, 'FM000'), to_hex(255), to_number('1,234.5', '9,999.9')

SELECT upper(NULL) IS NULL, length('') , '' IS NULL, 'a' || '' = 'a'

SELECT 'é' < 'z', length(E'a\tb'), 'it''s', E'line\\n'
