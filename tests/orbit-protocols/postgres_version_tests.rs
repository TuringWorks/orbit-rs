//! Orbit reports itself as PostgreSQL 18 on every path a client can ask.
//!
//! Drivers and tools gate features on `server_version`/`server_version_num`, so the answer must
//! agree between `SHOW`, `current_setting`, `version()` and the executor's own settings.

use orbit_server::protocols::postgres_wire::query_engine::{QueryEngine, QueryResult};
use orbit_server::protocols::postgres_wire::sql::executor::{ExecutionResult, SqlExecutor};

async fn engine_scalar(sql: &str) -> String {
    let engine = QueryEngine::new();
    match engine.execute_query(sql).await.unwrap().untyped() {
        QueryResult::Select { rows, .. } => rows[0][0].clone().unwrap(),
        other => panic!("expected rows for {sql}, got {other:?}"),
    }
}

async fn executor_show(variable: &str) -> String {
    let executor = SqlExecutor::new().await.unwrap();
    match executor.execute(&format!("SHOW {variable}")).await.unwrap() {
        ExecutionResult::Show { value, .. } => value,
        other => panic!("expected SHOW result, got {other:?}"),
    }
}

#[tokio::test]
async fn client_path_show_server_version_is_18() {
    assert_eq!(engine_scalar("SHOW server_version").await, "18.0");
}

#[tokio::test]
async fn client_path_show_server_version_num_is_180000() {
    assert_eq!(engine_scalar("SHOW server_version_num").await, "180000");
}

#[tokio::test]
async fn client_path_current_setting_matches() {
    assert_eq!(
        engine_scalar("SELECT current_setting('server_version_num')").await,
        "180000"
    );
}

#[tokio::test]
async fn client_path_version_function_names_postgresql_18() {
    let v = engine_scalar("SELECT version()").await;
    assert!(v.starts_with("PostgreSQL 18"), "{v}");
}

#[tokio::test]
async fn executor_settings_agree_with_the_client_path() {
    assert_eq!(executor_show("server_version").await, "18.0");
    assert_eq!(executor_show("server_version_num").await, "180000");
}
