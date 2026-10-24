# What psql, ORMs and drivers ask a server about itself.

DROP TABLE IF EXISTS cf15_child

DROP TABLE IF EXISTS cf15_parent

CREATE TABLE cf15_parent (id serial PRIMARY KEY, name varchar(40) NOT NULL, score numeric(6,2), active boolean DEFAULT true, born date)

CREATE TABLE cf15_child (id int PRIMARY KEY, parent_id int NOT NULL REFERENCES cf15_parent (id) ON DELETE CASCADE, tag text UNIQUE)

CREATE INDEX cf15_child_parent_idx ON cf15_child (parent_id)

SELECT current_database(), current_schema(), current_user, session_user

SELECT current_schemas(false), current_setting('server_version_num')::int >= 180000, current_setting('standard_conforming_strings')

SELECT version() LIKE 'PostgreSQL 18%'

SELECT table_schema, table_name, table_type FROM information_schema.tables WHERE table_name LIKE 'cf15%' ORDER BY table_name

SELECT column_name, data_type, is_nullable, column_default, character_maximum_length, numeric_precision, numeric_scale, ordinal_position FROM information_schema.columns WHERE table_name = 'cf15_parent' ORDER BY ordinal_position

SELECT constraint_name, constraint_type FROM information_schema.table_constraints WHERE table_name = 'cf15_child' ORDER BY constraint_type, constraint_name

SELECT kcu.column_name, ccu.table_name, ccu.column_name FROM information_schema.key_column_usage kcu JOIN information_schema.constraint_column_usage ccu ON ccu.constraint_name = kcu.constraint_name WHERE kcu.table_name = 'cf15_child' AND kcu.column_name = 'parent_id'

SELECT c.relname, c.relkind, c.relhasindex FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE n.nspname = 'public' AND c.relname LIKE 'cf15%' ORDER BY c.relname

SELECT a.attname, format_type(a.atttypid, a.atttypmod), a.attnotnull, a.attnum FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid WHERE c.relname = 'cf15_parent' AND a.attnum > 0 AND NOT a.attisdropped ORDER BY a.attnum

SELECT i.relname, ix.indisunique, ix.indisprimary FROM pg_index ix JOIN pg_class i ON i.oid = ix.indexrelid JOIN pg_class t ON t.oid = ix.indrelid WHERE t.relname = 'cf15_child' ORDER BY i.relname

SELECT conname, contype FROM pg_constraint WHERE conrelid = 'cf15_child'::regclass ORDER BY conname

SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conrelid = 'cf15_child'::regclass AND contype = 'f'

SELECT pg_get_indexdef(indexrelid) FROM pg_index WHERE indrelid = 'cf15_child'::regclass ORDER BY 1

SELECT 'cf15_parent'::regclass::oid IS NOT NULL, 'cf15_parent'::regclass::text, to_regclass('cf15_nope') IS NULL

SELECT t.typname, t.typlen FROM pg_type t WHERE t.typname IN ('int4', 'text', 'bool', 'numeric', 'varchar', 'timestamptz', 'uuid', 'jsonb') ORDER BY t.typname

SELECT oid, typname FROM pg_type WHERE typname IN ('bool', 'int4', 'int8', 'text', 'float8') ORDER BY oid

SELECT nspname FROM pg_namespace WHERE nspname IN ('public', 'pg_catalog', 'information_schema') ORDER BY nspname

SELECT schemaname, tablename FROM pg_tables WHERE tablename LIKE 'cf15%' ORDER BY tablename

SELECT schemaname, indexname FROM pg_indexes WHERE tablename = 'cf15_child' ORDER BY indexname

SELECT count(*) > 0 FROM pg_proc WHERE proname = 'lower'

SELECT rolname FROM pg_roles WHERE rolname = current_user

SELECT datname FROM pg_database WHERE datname = current_database()

SELECT name FROM pg_settings WHERE name IN ('server_version_num', 'max_connections') ORDER BY name

SELECT pg_table_is_visible('cf15_parent'::regclass), has_table_privilege('cf15_parent', 'SELECT')

SELECT obj_description('cf15_parent'::regclass, 'pg_class') IS NULL

COMMENT ON TABLE cf15_parent IS 'people'

SELECT obj_description('cf15_parent'::regclass, 'pg_class')

COMMENT ON COLUMN cf15_parent.name IS 'full name'

SELECT col_description('cf15_parent'::regclass, 2)

SELECT pg_get_serial_sequence('cf15_parent', 'id')

SELECT pg_get_expr(adbin, adrelid) FROM pg_attrdef WHERE adrelid = 'cf15_parent'::regclass ORDER BY adnum

SELECT set_config('application_name', 'cf', false), current_setting('application_name')

SELECT pg_backend_pid() > 0, pg_postmaster_start_time() IS NOT NULL, pg_is_in_recovery()

SELECT inet_server_port() IS NOT NULL OR true

SHOW server_encoding

SHOW client_encoding

SHOW DateStyle

SHOW standard_conforming_strings

SHOW max_identifier_length

SHOW transaction_isolation

SHOW ALL LIKE 'nothing_matches'

DROP TABLE cf15_child, cf15_parent
