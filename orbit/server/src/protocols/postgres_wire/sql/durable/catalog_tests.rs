//! The `pg_catalog` and `information_schema` views as database tools query them.

use super::*;

async fn q(e: &DurableEngine, sql: &str) -> Vec<Vec<String>> {
    let mut s = Session::default();
    let mut last = QueryResult::Command {
        tag: "EMPTY".into(),
    };
    for stmt in SqlParser::new().parse_multiple(sql).unwrap() {
        last = e.execute(&mut s, &stmt, sql).await.unwrap();
    }
    match last.untyped() {
        QueryResult::Select { rows, .. } => rows
            .into_iter()
            .map(|r| {
                r.into_iter()
                    .map(|c| c.unwrap_or_else(|| "NULL".into()))
                    .collect()
            })
            .collect(),
        _ => Vec::new(),
    }
}

async fn shop() -> DurableEngine {
    let e = DurableEngine::in_memory();
    q(
        &e,
        "CREATE TABLE customer (id SERIAL PRIMARY KEY, name VARCHAR(40) NOT NULL, balance NUMERIC(10,2))",
    )
    .await;
    q(
        &e,
        "CREATE TABLE sale (id SERIAL PRIMARY KEY, customer_id INTEGER NOT NULL REFERENCES customer(id) ON DELETE CASCADE, total NUMERIC(10,2))",
    )
    .await;
    q(
        &e,
        "INSERT INTO customer (name, balance) VALUES ('ann', 12.5), ('bob', 3.25)",
    )
    .await;
    e
}

fn one(rows: Vec<Vec<String>>) -> String {
    assert_eq!(rows.len(), 1, "{rows:?}");
    rows[0][0].clone()
}

#[tokio::test]
async fn pg_class_lists_tables_indexes_and_sequences() {
    let e = shop().await;
    let rows = q(
        &e,
        "SELECT relname, relkind FROM pg_catalog.pg_class WHERE relnamespace = 2200 ORDER BY relname",
    )
    .await;
    let names: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r[0].as_str(), r[1].as_str()))
        .collect();
    assert!(names.contains(&("customer", "r")));
    assert!(names.contains(&("customer_pkey", "i")));
    assert!(names.contains(&("customer_id_seq", "S")));
    assert!(names.contains(&("sale", "r")));
}

#[tokio::test]
async fn pg_catalog_lists_itself_so_tools_hide_it() {
    let e = shop().await;
    let hidden = q(
        &e,
        "SELECT count(*) FROM pg_catalog.pg_namespace nsp WHERE nsp.nspname = 'pg_catalog' AND EXISTS (SELECT 1 FROM pg_catalog.pg_class WHERE relname = 'pg_class' AND relnamespace = nsp.oid LIMIT 1)",
    )
    .await;
    assert_eq!(one(hidden), "1");
}

#[tokio::test]
async fn columns_show_type_modifiers_and_defaults() {
    let e = shop().await;
    let rows = q(
        &e,
        "SELECT att.attname, pg_catalog.format_type(att.atttypid, att.atttypmod), att.attnotnull, pg_catalog.pg_get_expr(def.adbin, def.adrelid) FROM pg_catalog.pg_attribute att LEFT JOIN pg_catalog.pg_attrdef def ON (att.attrelid = def.adrelid AND att.attnum = def.adnum) WHERE att.attrelid = 'customer'::regclass AND att.attnum > 0 ORDER BY att.attnum",
    )
    .await;
    assert_eq!(
        rows,
        vec![
            vec!["id", "integer", "t", "nextval('customer_id_seq'::regclass)"],
            vec!["name", "character varying(40)", "t", "NULL"],
            vec!["balance", "numeric(10,2)", "f", "NULL"],
        ]
    );
}

#[tokio::test]
async fn information_schema_columns_exposes_constant_defaults() {
    let e = DurableEngine::in_memory();
    let _ = q(&e, "CREATE TABLE defaults_probe (id SERIAL PRIMARY KEY, active BOOLEAN DEFAULT true, label TEXT DEFAULT 'ready'); SELECT 1").await;
    assert_eq!(
        q(&e, "SELECT column_name, column_default FROM information_schema.columns WHERE table_name = 'defaults_probe' ORDER BY ordinal_position").await,
        vec![
            vec!["id", "nextval('defaults_probe_id_seq'::regclass)"],
            vec!["active", "true"],
            vec!["label", "ready"],
        ]
    );
}

#[tokio::test]
async fn constraints_and_index_definitions_are_readable() {
    let e = shop().await;
    assert_eq!(
        q(&e, "SELECT conname, contype, pg_get_constraintdef(oid) FROM pg_constraint WHERE conrelid = 'customer'::regclass").await,
        vec![
            vec!["customer_pkey", "p", "PRIMARY KEY (id)"],
            vec!["customer_id_not_null", "n", "NOT NULL id"],
            vec!["customer_name_not_null", "n", "NOT NULL name"],
        ]
    );
    let fk = q(
        &e,
        "SELECT contype, pg_get_constraintdef(oid), confrelid = 'customer'::regclass FROM pg_constraint WHERE conrelid = 'sale'::regclass AND contype = 'f'",
    )
    .await;
    assert_eq!(fk[0][0], "f");
    assert!(fk[0][1].starts_with("FOREIGN KEY (customer_id) REFERENCES customer(id)"));
    assert_eq!(fk[0][2], "t");
    assert_eq!(
        one(q(&e, "SELECT pg_get_indexdef(indexrelid) FROM pg_index WHERE indrelid = 'customer'::regclass AND indisprimary").await),
        "CREATE UNIQUE INDEX customer_pkey ON public.customer USING btree (id)"
    );
}

#[tokio::test]
async fn names_resolve_to_object_ids_and_back() {
    let e = shop().await;
    assert_eq!(one(q(&e, "SELECT 'pg_class'::regclass").await), "1259");
    assert_eq!(
        one(q(
            &e,
            "SELECT relname FROM pg_class WHERE oid = 'sale'::regclass"
        )
        .await),
        "sale"
    );
    assert_eq!(one(q(&e, "SELECT to_regclass('nope') IS NULL").await), "t");
    assert_eq!(one(q(&e, "SELECT 'int4'::regtype").await), "23");
    assert_eq!(one(q(&e, "SELECT 2200::oid").await), "2200");
}

#[tokio::test]
async fn session_values_and_server_functions() {
    let e = shop().await;
    assert_eq!(
        q(&e, "SELECT current_user, session_user, current_schema()").await,
        vec![vec!["orbit", "orbit", "public"]]
    );
    assert_eq!(
        one(q(&e, "SELECT pg_catalog.pg_encoding_to_char(6)").await),
        "UTF8"
    );
    assert_eq!(
        one(q(&e, "SELECT has_schema_privilege(2200, 'CREATE')").await),
        "t"
    );
    assert_eq!(
        one(q(&e, "SELECT pg_size_pretty(2048)").await),
        "2048 bytes"
    );
    assert_eq!(
        one(q(&e, "SELECT pg_size_pretty(5242880)").await),
        "5120 kB"
    );
    assert!(
        one(q(&e, "SELECT pg_backend_pid()").await)
            .parse::<u32>()
            .is_ok_and(|pid| pid > 0),
        "pg_backend_pid() is a positive process id"
    );
    assert_eq!(
        one(q(
            &e,
            "SELECT rolname FROM pg_roles WHERE rolname = current_user"
        )
        .await),
        "orbit"
    );
}

#[tokio::test]
async fn catalog_views_with_no_rows_still_have_columns() {
    let e = shop().await;
    for view in [
        "pg_description",
        "pg_trigger",
        "pg_inherits",
        "pg_depend",
        "pg_extension",
        "pg_auth_members",
        "pg_shdescription",
    ] {
        assert!(q(&e, &format!("SELECT * FROM pg_catalog.{view}"))
            .await
            .is_empty());
    }
    assert_eq!(
        one(q(
            &e,
            "SELECT count(*) FROM pg_settings WHERE name = 'max_connections'"
        )
        .await),
        "1"
    );
    assert_eq!(one(q(&e, "SELECT count(*) FROM pg_tablespace").await), "2");
    assert_eq!(
        one(q(&e, "SELECT count(*) FROM pg_stat_activity").await),
        "1"
    );
}

#[tokio::test]
async fn pgadmin_table_list_query_runs() {
    let e = shop().await;
    let rows = q(
        &e,
        "SELECT rel.oid, rel.relname AS name, (SELECT count(*) FROM pg_catalog.pg_trigger WHERE tgrelid=rel.oid AND tgisinternal = FALSE) AS triggercount, (CASE WHEN rel.relkind = 'p' THEN true ELSE false END) AS is_partitioned, (SELECT count(1) FROM pg_catalog.pg_inherits WHERE inhrelid=rel.oid LIMIT 1) as is_inherits, des.description FROM pg_catalog.pg_class rel LEFT OUTER JOIN pg_catalog.pg_description des ON (des.objoid=rel.oid AND des.objsubid=0 AND des.classoid='pg_class'::regclass) WHERE rel.relkind IN ('r','s','t','p') AND rel.relnamespace = 2200::oid AND NOT rel.relispartition ORDER BY rel.relname",
    )
    .await;
    let names: Vec<&str> = rows.iter().map(|r| r[1].as_str()).collect();
    assert_eq!(names, vec!["customer", "sale"]);
    assert_eq!(rows[0][2], "0");
}

#[tokio::test]
async fn information_schema_describes_constraints_and_keys() {
    let e = shop().await;
    assert_eq!(
        q(&e, "SELECT constraint_name, constraint_type FROM information_schema.table_constraints WHERE table_name = 'sale' ORDER BY constraint_type").await.len(),
        4
    );
    let keys = q(
        &e,
        "SELECT column_name, ordinal_position, position_in_unique_constraint FROM information_schema.key_column_usage WHERE table_name = 'sale' AND column_name = 'customer_id'",
    )
    .await;
    assert_eq!(keys, vec![vec!["customer_id", "1", "1"]]);
    let refs = q(
        &e,
        "SELECT delete_rule, update_rule FROM information_schema.referential_constraints",
    )
    .await;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0][0], "CASCADE");
    assert_eq!(
        q(&e, "SELECT table_name, column_name FROM information_schema.constraint_column_usage WHERE table_name = 'customer' ORDER BY column_name").await,
        vec![vec!["customer", "id"], vec!["customer", "id"]]
    );
}

#[tokio::test]
async fn information_schema_columns_carry_precision_and_lengths() {
    let e = shop().await;
    let rows = q(
        &e,
        "SELECT column_name, data_type, character_maximum_length, numeric_precision, numeric_scale, udt_name FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'customer' ORDER BY ordinal_position",
    )
    .await;
    assert_eq!(
        rows,
        vec![
            vec!["id", "integer", "NULL", "32", "0", "int4"],
            vec!["name", "character varying", "40", "NULL", "NULL", "varchar"],
            vec!["balance", "numeric", "NULL", "10", "2", "numeric"],
        ]
    );
}

#[tokio::test]
async fn information_schema_lists_schemas_sequences_and_privileges() {
    let e = shop().await;
    assert_eq!(
        q(
            &e,
            "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name"
        )
        .await,
        vec![
            vec!["information_schema"],
            vec!["pg_catalog"],
            vec!["pg_toast"],
            vec!["public"]
        ]
    );
    assert_eq!(
        q(&e, "SELECT sequence_name, data_type FROM information_schema.sequences ORDER BY sequence_name").await,
        vec![vec!["customer_id_seq", "integer"], vec!["sale_id_seq", "integer"]]
    );
    assert_eq!(
        one(q(&e, "SELECT count(*) FROM information_schema.table_privileges WHERE table_name = 'customer' AND privilege_type = 'SELECT'").await),
        "1"
    );
    assert_eq!(
        one(q(
            &e,
            "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'public'"
        )
        .await),
        "2"
    );
    assert!(
        one(q(
            &e,
            "SELECT count(*) FROM information_schema.tables WHERE table_schema = 'pg_catalog'"
        )
        .await)
        .parse::<i32>()
        .unwrap()
            > 10
    );
    for view in [
        "views",
        "routines",
        "parameters",
        "triggers",
        "domains",
        "collations",
        "element_types",
        "applicable_roles",
    ] {
        assert!(q(&e, &format!("SELECT * FROM information_schema.{view}"))
            .await
            .is_empty());
    }
    assert_eq!(
        one(q(&e, "SELECT role_name FROM information_schema.enabled_roles").await),
        "orbit"
    );
}

#[tokio::test]
async fn round_accepts_numeric_values() {
    let e = shop().await;
    assert_eq!(
        one(q(&e, "SELECT round(avg(balance), 1) FROM customer").await),
        "7.9"
    );
    assert_eq!(one(q(&e, "SELECT round(2.5)").await), "3");
}
