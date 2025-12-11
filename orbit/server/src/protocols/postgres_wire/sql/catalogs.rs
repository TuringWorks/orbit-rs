use super::types::SqlType;
use std::collections::HashMap;

/// System Catalog definition
#[derive(Debug, Clone)]
pub struct CatalogDefinition {
    pub name: String,
    pub columns: Vec<(String, SqlType)>,
}

impl CatalogDefinition {
    pub fn new(name: &str, columns: Vec<(&str, SqlType)>) -> Self {
        Self {
            name: name.to_string(),
            columns: columns
                .into_iter()
                .map(|(n, t)| (n.to_string(), t))
                .collect(),
        }
    }
}

/// Get all supported system catalogs
pub fn get_system_catalogs() -> Vec<CatalogDefinition> {
    vec![
        // pg_class: Relations (tables, indexes, sequences, views)
        CatalogDefinition::new(
            "pg_class",
            vec![
                ("oid", SqlType::Oid),
                ("relname", SqlType::Name),
                ("relnamespace", SqlType::Oid),
                ("reltype", SqlType::Oid),
                ("reloftype", SqlType::Oid),
                ("relowner", SqlType::Oid),
                ("relam", SqlType::Oid),
                ("relfilenode", SqlType::Oid),
                ("reltablespace", SqlType::Oid),
                ("relpages", SqlType::Integer),
                ("reltuples", SqlType::Real),
                ("relallvisible", SqlType::Integer),
                ("reltoastrelid", SqlType::Oid),
                ("relhasindex", SqlType::Boolean),
                ("relisshared", SqlType::Boolean),
                ("relpersistence", SqlType::Char(Some(1))),
                ("relkind", SqlType::Char(Some(1))),
                ("relnatts", SqlType::SmallInt),
                ("relchecks", SqlType::SmallInt),
                ("relhasrules", SqlType::Boolean),
                ("relhastriggers", SqlType::Boolean),
                ("relhassubclass", SqlType::Boolean),
                ("relrowsecurity", SqlType::Boolean),
                ("relforcerowsecurity", SqlType::Boolean),
                ("relispopulated", SqlType::Boolean),
                ("relreplident", SqlType::Char(Some(1))),
                ("relispartition", SqlType::Boolean),
                ("relrewrite", SqlType::Oid),
                ("relfrozenxid", SqlType::Xid),
                ("relminmxid", SqlType::Xid),
                ("relacl", SqlType::AclItem), // Array of ACL
                ("reloptions", SqlType::Array { element_type: Box::new(SqlType::Text), dimensions: Some(1) }),
                ("relpartbound", SqlType::PgNodeTree),
            ],
        ),
        // pg_attribute: Columns
        CatalogDefinition::new(
            "pg_attribute",
            vec![
                ("attrelid", SqlType::Oid),
                ("attname", SqlType::Name),
                ("atttypid", SqlType::Oid),
                ("attstattarget", SqlType::Integer),
                ("attlen", SqlType::SmallInt),
                ("attnum", SqlType::SmallInt),
                ("attndims", SqlType::Integer),
                ("attcacheoff", SqlType::Integer),
                ("atttypmod", SqlType::Integer),
                ("attbyval", SqlType::Boolean),
                ("attstorage", SqlType::Char(Some(1))),
                ("attalign", SqlType::Char(Some(1))),
                ("attnotnull", SqlType::Boolean),
                ("atthasdef", SqlType::Boolean),
                ("atthasmissing", SqlType::Boolean),
                ("attidentity", SqlType::Char(Some(1))),
                ("attgenerated", SqlType::Char(Some(1))),
                ("attisdropped", SqlType::Boolean),
                ("attislocal", SqlType::Boolean),
                ("attinhcount", SqlType::Integer),
                ("attcollation", SqlType::Oid),
                ("attacl", SqlType::AclItem),
                ("attoptions", SqlType::Array { element_type: Box::new(SqlType::Text), dimensions: Some(1) }),
                ("attfdwoptions", SqlType::Array { element_type: Box::new(SqlType::Text), dimensions: Some(1) }),
                ("attmissingval", SqlType::Array { element_type: Box::new(SqlType::Bytea), dimensions: Some(1) }), // Simplified
            ],
        ),
        // pg_index: Index information
        CatalogDefinition::new(
            "pg_index",
            vec![
                ("indexrelid", SqlType::Oid),
                ("indrelid", SqlType::Oid),
                ("indnatts", SqlType::SmallInt),
                ("indnkeyatts", SqlType::SmallInt),
                ("indisunique", SqlType::Boolean),
                ("indisprimary", SqlType::Boolean),
                ("indisexclusion", SqlType::Boolean),
                ("indimmediate", SqlType::Boolean),
                ("indisclustered", SqlType::Boolean),
                ("indisvalid", SqlType::Boolean),
                ("indcheckxmin", SqlType::Boolean),
                ("indisready", SqlType::Boolean),
                ("indislive", SqlType::Boolean),
                ("indisreplident", SqlType::Boolean),
                ("indkey", SqlType::Int2Vector), 
                ("indcollation", SqlType::OidVector),
                ("indclass", SqlType::OidVector),
                ("indoption", SqlType::Int2Vector),
                ("indexprs", SqlType::PgNodeTree),
                ("indpred", SqlType::PgNodeTree),
            ],
        ),
        // pg_namespace: Schemas
        CatalogDefinition::new(
            "pg_namespace",
            vec![
                ("oid", SqlType::Oid),
                ("nspname", SqlType::Name),
                ("nspowner", SqlType::Oid),
                ("nspacl", SqlType::AclItem),
            ],
        ),
        // pg_database: Databases
        CatalogDefinition::new(
            "pg_database",
            vec![
                ("oid", SqlType::Oid),
                ("datname", SqlType::Name),
                ("datdba", SqlType::Oid),
                ("encoding", SqlType::Integer),
                ("datcollate", SqlType::Name),
                ("datctype", SqlType::Name),
                ("datistemplate", SqlType::Boolean),
                ("datallowconn", SqlType::Boolean),
                ("datconnlimit", SqlType::Integer),
                ("datlastsysoid", SqlType::Oid),
                ("datfrozenxid", SqlType::Xid),
                ("datminmxid", SqlType::Xid),
                ("dattablespace", SqlType::Oid),
                ("datacl", SqlType::AclItem),
            ],
        ),
        // pg_roles: Roles (via pg_authid view usually, but simpler map here)
        CatalogDefinition::new(
            "pg_roles", // This is technically a view in PG on top of pg_authid
            vec![
                ("oid", SqlType::Oid),
                ("rolname", SqlType::Name),
                ("rolsuper", SqlType::Boolean),
                ("rolinherit", SqlType::Boolean),
                ("rolcreaterole", SqlType::Boolean),
                ("rolcreatedb", SqlType::Boolean),
                ("rolcanlogin", SqlType::Boolean),
                ("rolreplication", SqlType::Boolean),
                ("rolconnlimit", SqlType::Integer),
                ("rolpassword", SqlType::Text),
                ("rolvaliduntil", SqlType::Timestamp { with_timezone: true }),
                ("rolbypassrls", SqlType::Boolean),
                ("rolconfig", SqlType::Array { element_type: Box::new(SqlType::Text), dimensions: Some(1) }),
                ("oid", SqlType::Oid),
            ],
        ),
        // pg_proc: Functions
        CatalogDefinition::new(
            "pg_proc",
            vec![
                ("oid", SqlType::Oid),
                ("proname", SqlType::Name),
                ("pronamespace", SqlType::Oid),
                ("proowner", SqlType::Oid),
                ("prolang", SqlType::Oid),
                ("procost", SqlType::Real),
                ("prorows", SqlType::Real),
                ("provariadic", SqlType::Oid),
                ("prosupport", SqlType::Regproc),
                ("prokind", SqlType::Char(Some(1))),
                ("prosecdef", SqlType::Boolean),
                ("proleakproof", SqlType::Boolean),
                ("proisstrict", SqlType::Boolean),
                ("proretset", SqlType::Boolean),
                ("provolatile", SqlType::Char(Some(1))),
                ("proparallel", SqlType::Char(Some(1))),
                ("pronargs", SqlType::SmallInt),
                ("pronargdefaults", SqlType::SmallInt),
                ("prorettype", SqlType::Oid),
                ("proargtypes", SqlType::OidVector),
                ("proallargtypes", SqlType::Array { element_type: Box::new(SqlType::Oid), dimensions: Some(1) }),
                ("proargmodes", SqlType::Array { element_type: Box::new(SqlType::Char(Some(1))), dimensions: Some(1) }),
                ("proargnames", SqlType::Array { element_type: Box::new(SqlType::Text), dimensions: Some(1) }),
                ("proargdefaults", SqlType::PgNodeTree),
                ("protrftypes", SqlType::Array { element_type: Box::new(SqlType::Oid), dimensions: Some(1) }),
                ("prosrc", SqlType::Text),
                ("probin", SqlType::Text),
                ("prosqlbody", SqlType::PgNodeTree),
                ("proconfig", SqlType::Array { element_type: Box::new(SqlType::Text), dimensions: Some(1) }),
                ("proacl", SqlType::AclItem),
            ],
        ),
        // pg_operator: Operators
        CatalogDefinition::new(
            "pg_operator",
            vec![
                ("oid", SqlType::Oid),
                ("oprname", SqlType::Name),
                ("oprnamespace", SqlType::Oid),
                ("oprowner", SqlType::Oid),
                ("oprkind", SqlType::Char(Some(1))),
                ("oprcanmerge", SqlType::Boolean),
                ("oprcanhash", SqlType::Boolean),
                ("oprleft", SqlType::Oid),
                ("oprright", SqlType::Oid),
                ("oprresult", SqlType::Oid),
                ("oprcom", SqlType::Oid),
                ("oprnegate", SqlType::Oid),
                ("oprcode", SqlType::Regproc),
                ("oprrest", SqlType::Regproc),
                ("oprjoin", SqlType::Regproc),
            ],
        ),
        // pg_constraint: Constraints
        CatalogDefinition::new(
            "pg_constraint",
            vec![
                ("oid", SqlType::Oid),
                ("conname", SqlType::Name),
                ("connamespace", SqlType::Oid),
                ("contype", SqlType::Char(Some(1))),
                ("condeferrable", SqlType::Boolean),
                ("condeferred", SqlType::Boolean),
                ("convalidated", SqlType::Boolean),
                ("conrelid", SqlType::Oid),
                ("contypid", SqlType::Oid),
                ("conindid", SqlType::Oid),
                ("conparentid", SqlType::Oid),
                ("confrelid", SqlType::Oid),
                ("confupdtype", SqlType::Char(Some(1))),
                ("confdeltype", SqlType::Char(Some(1))),
                ("confmatchtype", SqlType::Char(Some(1))),
                ("conislocal", SqlType::Boolean),
                ("coninhcount", SqlType::Integer),
                ("connoinherit", SqlType::Boolean),
                ("conkey", SqlType::Array { element_type: Box::new(SqlType::SmallInt), dimensions: Some(1) }),
                ("confkey", SqlType::Array { element_type: Box::new(SqlType::SmallInt), dimensions: Some(1) }),
                ("conpfeqop", SqlType::Array { element_type: Box::new(SqlType::Oid), dimensions: Some(1) }),
                ("conppeqop", SqlType::Array { element_type: Box::new(SqlType::Oid), dimensions: Some(1) }),
                ("conffeqop", SqlType::Array { element_type: Box::new(SqlType::Oid), dimensions: Some(1) }),
                ("conexclop", SqlType::Array { element_type: Box::new(SqlType::Oid), dimensions: Some(1) }),
                ("conbin", SqlType::PgNodeTree),
            ],
        ),
    ]
}
