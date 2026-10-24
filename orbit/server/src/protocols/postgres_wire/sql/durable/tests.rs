use super::*;

async fn run(e: &DurableEngine, s: &mut Session, sql: &str) -> ProtocolResult<QueryResult> {
    let mut last = QueryResult::Command {
        tag: "EMPTY".into(),
    };
    for stmt in SqlParser::new().parse_multiple(sql).unwrap() {
        assert!(DurableEngine::handles(&stmt), "not handled: {sql}");
        last = e.execute(s, &stmt, sql).await?;
    }
    Ok(last)
}

fn cells(r: QueryResult) -> Vec<Vec<String>> {
    let rows = match r.untyped() {
        QueryResult::Select { rows, .. } => rows,
        other => panic!("expected rows, got {other:?}"),
    };
    rows.into_iter()
        .map(|r| {
            r.into_iter()
                .map(|c| c.unwrap_or_else(|| "NULL".into()))
                .collect()
        })
        .collect()
}

async fn q(e: &DurableEngine, sql: &str) -> Vec<Vec<String>> {
    cells(run(e, &mut Session::default(), sql).await.unwrap())
}

async fn x(e: &DurableEngine, sql: &str) {
    run(e, &mut Session::default(), sql).await.unwrap();
}

fn msg(r: ProtocolResult<QueryResult>) -> String {
    r.unwrap_err().to_string()
}

async fn seeded() -> DurableEngine {
    let e = DurableEngine::in_memory();
    x(
        &e,
        "CREATE TABLE w (id INTEGER PRIMARY KEY, n VARCHAR, q INTEGER)",
    )
    .await;
    for (id, n, q) in [(1, "a", 10), (2, "b", 20), (3, "c", 30), (4, "d", 40)] {
        x(
            &e,
            &format!("INSERT INTO w (id, n, q) VALUES ({id}, '{n}', {q})"),
        )
        .await;
    }
    e
}

#[tokio::test]
async fn select_applies_where_order_limit_and_aggregates() {
    let e = seeded().await;
    assert_eq!(
        q(&e, "SELECT id, n FROM w WHERE id = 2").await,
        vec![vec!["2", "b"]]
    );
    assert_eq!(
        q(&e, "SELECT id FROM w WHERE q > 25 ORDER BY id")
            .await
            .len(),
        2
    );
    assert_eq!(
        q(
            &e,
            "SELECT id FROM w WHERE id IN (2, 3) ORDER BY id DESC LIMIT 1"
        )
        .await,
        vec![vec!["3"]]
    );
    assert_eq!(
        q(&e, "SELECT id FROM w ORDER BY id LIMIT 2 OFFSET 1").await,
        vec![vec!["2"], vec!["3"]]
    );
    assert_eq!(q(&e, "SELECT count(*) AS c FROM w").await, vec![vec!["4"]]);
    assert_eq!(
        q(&e, "SELECT sum(q), min(q), max(q), avg(q) FROM w").await,
        vec![vec!["100", "10", "40", "25"]]
    );
    assert_eq!(
        q(&e, "SELECT count(*) FROM w WHERE q > 100").await,
        vec![vec!["0"]]
    );
}

#[tokio::test]
async fn group_by_having_distinct_and_order_by_alias() {
    let e = DurableEngine::in_memory();
    x(
        &e,
        "CREATE TABLE s (id SERIAL PRIMARY KEY, g VARCHAR, v INTEGER)",
    )
    .await;
    x(
        &e,
        "INSERT INTO s (g, v) VALUES ('x', 1), ('x', 2), ('y', 5), ('z', 5)",
    )
    .await;
    assert_eq!(
        q(
            &e,
            "SELECT g, sum(v) AS t FROM s GROUP BY g ORDER BY t DESC, g"
        )
        .await,
        vec![vec!["y", "5"], vec!["z", "5"], vec!["x", "3"]]
    );
    assert_eq!(
        q(&e, "SELECT g FROM s GROUP BY g HAVING count(*) > 1").await,
        vec![vec!["x"]]
    );
    assert_eq!(
        q(&e, "SELECT DISTINCT v FROM s ORDER BY v").await,
        vec![vec!["1"], vec!["2"], vec!["5"]]
    );
    assert_eq!(
        q(&e, "SELECT coalesce(sum(v), 0) FROM s WHERE g = 'none'").await,
        vec![vec!["0"]]
    );
}

#[tokio::test]
async fn serial_and_returning() {
    let e = DurableEngine::in_memory();
    x(&e, "CREATE TABLE r (id SERIAL PRIMARY KEY, name VARCHAR NOT NULL, created VARCHAR DEFAULT 'now')").await;
    let res = run(
        &e,
        &mut Session::default(),
        "INSERT INTO r (name) VALUES ('a') RETURNING id",
    )
    .await
    .unwrap();
    match &res {
        QueryResult::Returning { tag, columns, rows } => {
            assert_eq!(tag, "INSERT 0 1");
            assert_eq!(columns[0], ("id".to_string(), 23));
            assert_eq!(rows, &vec![vec![Some("1".to_string())]]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        cells(
            run(
                &e,
                &mut Session::default(),
                "INSERT INTO r (name) VALUES ('b'), ('c') RETURNING id, name"
            )
            .await
            .unwrap()
        ),
        vec![vec!["2", "b"], vec!["3", "c"]]
    );
    assert_eq!(
        q(&e, "SELECT created FROM r WHERE id = 1").await,
        vec![vec!["now"]]
    );
    assert_eq!(
        cells(
            run(
                &e,
                &mut Session::default(),
                "UPDATE r SET name = 'z' WHERE id = 2 RETURNING name"
            )
            .await
            .unwrap()
        ),
        vec![vec!["z"]]
    );
    assert_eq!(
        cells(
            run(
                &e,
                &mut Session::default(),
                "DELETE FROM r WHERE id = 3 RETURNING id"
            )
            .await
            .unwrap()
        ),
        vec![vec!["3"]]
    );
    assert_eq!(q(&e, "SELECT count(*) FROM r").await, vec![vec!["2"]]);
}

#[tokio::test]
async fn text_that_looks_like_a_number_stays_text() {
    let e = DurableEngine::in_memory();
    x(
        &e,
        "CREATE TABLE t (id SERIAL PRIMARY KEY, dayofweek VARCHAR, flag VARCHAR)",
    )
    .await;
    x(&e, "INSERT INTO t (dayofweek, flag) VALUES ('0', 'null')").await;
    let r = run(&e, &mut Session::default(), "SELECT dayofweek, flag FROM t")
        .await
        .unwrap();
    match r {
        QueryResult::Typed { columns, rows } => {
            assert_eq!(
                columns[0].1, 1043,
                "varchar column is reported as varchar, not integer"
            );
            assert_eq!(
                rows,
                vec![vec![Some("0".to_string()), Some("null".to_string())]]
            );
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn constraints_are_enforced_with_sqlstates() {
    let e = seeded().await;
    let dup = msg(run(
        &e,
        &mut Session::default(),
        "INSERT INTO w (id, n, q) VALUES (1, 'x', 1)",
    )
    .await);
    assert!(dup.contains("[23505]") && dup.contains("w_pkey"), "{dup}");
    let nn = msg(run(
        &e,
        &mut Session::default(),
        "INSERT INTO w (n, q) VALUES ('x', 1)",
    )
    .await);
    assert!(nn.contains("[23502]"), "{nn}");
    let missing = msg(run(&e, &mut Session::default(), "SELECT * FROM nope").await);
    assert!(missing.contains("[42P01]"), "{missing}");
    x(
        &e,
        "CREATE TABLE u (id SERIAL PRIMARY KEY, email VARCHAR UNIQUE)",
    )
    .await;
    x(&e, "INSERT INTO u (email) VALUES ('a'), (NULL), (NULL)").await;
    let dup = msg(run(
        &e,
        &mut Session::default(),
        "INSERT INTO u (email) VALUES ('a')",
    )
    .await);
    assert!(dup.contains("[23505]"), "{dup}");
}

#[tokio::test]
async fn on_conflict_do_nothing_and_do_update() {
    let e = DurableEngine::in_memory();
    x(&e, "CREATE TABLE k (id INTEGER PRIMARY KEY, v INTEGER)").await;
    x(&e, "INSERT INTO k VALUES (1, 10)").await;
    x(&e, "INSERT INTO k VALUES (1, 99) ON CONFLICT DO NOTHING").await;
    assert_eq!(q(&e, "SELECT v FROM k").await, vec![vec!["10"]]);
    x(
        &e,
        "INSERT INTO k VALUES (1, 99) ON CONFLICT (id) DO UPDATE SET v = excluded.v + 1",
    )
    .await;
    assert_eq!(q(&e, "SELECT v FROM k").await, vec![vec!["100"]]);
}

#[tokio::test]
async fn joins_and_subqueries() {
    let e = DurableEngine::in_memory();
    x(&e, "CREATE TABLE p (id INTEGER PRIMARY KEY, name VARCHAR)").await;
    x(
        &e,
        "CREATE TABLE o (id INTEGER PRIMARY KEY, p_id INTEGER, amt INTEGER)",
    )
    .await;
    x(&e, "INSERT INTO p VALUES (1, 'ann'), (2, 'bob'), (3, 'cy')").await;
    x(
        &e,
        "INSERT INTO o VALUES (10, 1, 5), (11, 1, 7), (12, 2, 1)",
    )
    .await;
    assert_eq!(
        q(
            &e,
            "SELECT p.name, o.amt FROM p JOIN o ON o.p_id = p.id ORDER BY o.id"
        )
        .await,
        vec![vec!["ann", "5"], vec!["ann", "7"], vec!["bob", "1"]]
    );
    assert_eq!(q(&e, "SELECT p.name, count(o.id) FROM p LEFT JOIN o ON o.p_id = p.id GROUP BY p.name ORDER BY p.name").await, vec![vec!["ann", "2"], vec!["bob", "1"], vec!["cy", "0"]]);
    assert_eq!(
        q(
            &e,
            "SELECT name FROM p WHERE id IN (SELECT p_id FROM o WHERE amt > 4) ORDER BY id"
        )
        .await,
        vec![vec!["ann"]]
    );
    assert_eq!(
        q(
            &e,
            "SELECT name FROM p WHERE EXISTS (SELECT 1 FROM o WHERE o.p_id = p.id AND o.amt = 1)"
        )
        .await,
        vec![vec!["bob"]]
    );
    assert_eq!(
        q(
            &e,
            "SELECT name, (SELECT sum(amt) FROM o WHERE o.p_id = p.id) FROM p ORDER BY id"
        )
        .await,
        vec![vec!["ann", "12"], vec!["bob", "1"], vec!["cy", "NULL"]]
    );
}

#[tokio::test]
async fn with_clause_and_set_operations() {
    let e = seeded().await;
    assert_eq!(
        q(
            &e,
            "WITH big AS (SELECT id FROM w WHERE q > 20) SELECT count(*) FROM big"
        )
        .await,
        vec![vec!["2"]]
    );
    assert_eq!(
        q(
            &e,
            "SELECT id FROM w WHERE id < 3 UNION SELECT id FROM w WHERE id > 3 ORDER BY id"
        )
        .await,
        vec![vec!["1"], vec!["2"], vec!["4"]]
    );
    assert_eq!(
        q(&e, "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 4) SELECT sum(i) FROM n").await,
        vec![vec!["10"]]
    );
}

#[tokio::test]
async fn transactions_commit_and_roll_back() {
    let e = seeded().await;
    let mut s = Session::default();
    run(&e, &mut s, "BEGIN").await.unwrap();
    run(&e, &mut s, "INSERT INTO w VALUES (5, 'e', 50)")
        .await
        .unwrap();
    run(&e, &mut s, "UPDATE w SET q = 0 WHERE id = 1")
        .await
        .unwrap();
    run(&e, &mut s, "DELETE FROM w WHERE id = 2").await.unwrap();
    assert_eq!(
        cells(run(&e, &mut s, "SELECT count(*) FROM w").await.unwrap()),
        vec![vec!["4"]]
    );
    run(&e, &mut s, "ROLLBACK").await.unwrap();
    assert_eq!(
        q(&e, "SELECT id, q FROM w ORDER BY id").await,
        vec![
            vec!["1", "10"],
            vec!["2", "20"],
            vec!["3", "30"],
            vec!["4", "40"]
        ]
    );
    // unique index is back in step after the rollback
    x(&e, "INSERT INTO w VALUES (5, 'e', 50)").await;

    run(&e, &mut s, "BEGIN").await.unwrap();
    run(&e, &mut s, "INSERT INTO w VALUES (6, 'f', 60)")
        .await
        .unwrap();
    assert!(run(&e, &mut s, "INSERT INTO w VALUES (6, 'dup', 0)")
        .await
        .is_err());
    assert_eq!(s.txn_state(), Some(true));
    assert!(msg(run(&e, &mut s, "SELECT 1").await).contains("[25P02]"));
    run(&e, &mut s, "COMMIT").await.unwrap();
    assert_eq!(
        q(&e, "SELECT count(*) FROM w WHERE id = 6").await,
        vec![vec!["0"]],
        "a failed transaction commits nothing"
    );
}

#[tokio::test]
async fn savepoints_recover_from_a_failed_statement() {
    let e = seeded().await;
    let mut s = Session::default();
    run(&e, &mut s, "BEGIN").await.unwrap();
    run(&e, &mut s, "INSERT INTO w VALUES (5, 'e', 50)")
        .await
        .unwrap();
    run(&e, &mut s, "SAVEPOINT sp").await.unwrap();
    run(&e, &mut s, "INSERT INTO w VALUES (6, 'f', 60)")
        .await
        .unwrap();
    assert!(run(&e, &mut s, "INSERT INTO w VALUES (6, 'dup', 0)")
        .await
        .is_err());
    assert_eq!(s.txn_state(), Some(true));
    run(&e, &mut s, "ROLLBACK TO SAVEPOINT sp").await.unwrap();
    assert_eq!(
        s.txn_state(),
        Some(false),
        "rolling back to a savepoint clears the failed state"
    );
    assert_eq!(
        cells(run(&e, &mut s, "SELECT count(*) FROM w").await.unwrap()),
        vec![vec!["5"]],
        "work before the savepoint is kept, work after it is undone"
    );
    run(&e, &mut s, "INSERT INTO w VALUES (7, 'g', 70)")
        .await
        .unwrap();
    run(&e, &mut s, "RELEASE SAVEPOINT sp").await.unwrap();
    run(&e, &mut s, "COMMIT").await.unwrap();
    assert_eq!(
        q(&e, "SELECT id FROM w WHERE id > 4 ORDER BY id").await,
        vec![vec!["5"], vec!["7"]]
    );
    assert!(msg(run(&e, &mut Session::default(), "SAVEPOINT x").await).contains("[25P01]"));
}

#[tokio::test]
async fn a_dropped_open_transaction_is_undone() {
    let e = seeded().await;
    {
        let mut s = Session::default();
        run(&e, &mut s, "BEGIN").await.unwrap();
        run(&e, &mut s, "INSERT INTO w VALUES (9, 'x', 9)")
            .await
            .unwrap();
        run(&e, &mut s, "DELETE FROM w WHERE id = 1").await.unwrap();
        // the client disconnects: the session is dropped without COMMIT or ROLLBACK
    }
    assert_eq!(
        q(&e, "SELECT id FROM w ORDER BY id").await,
        vec![vec!["1"], vec!["2"], vec!["3"], vec!["4"]]
    );
    // and the engine is usable again: the gate was released
    x(&e, "INSERT INTO w VALUES (9, 'x', 9)").await;
    // committed work is not undone when its session ends
    {
        let mut s = Session::default();
        run(&e, &mut s, "BEGIN").await.unwrap();
        run(&e, &mut s, "INSERT INTO w VALUES (10, 'y', 10)")
            .await
            .unwrap();
        run(&e, &mut s, "COMMIT").await.unwrap();
    }
    assert_eq!(q(&e, "SELECT count(*) FROM w").await, vec![vec!["6"]]);
}

#[tokio::test]
async fn is_null_and_distinct_from() {
    let e = DurableEngine::in_memory();
    x(
        &e,
        "CREATE TABLE n (id INTEGER PRIMARY KEY, a INTEGER, b INTEGER)",
    )
    .await;
    x(
        &e,
        "INSERT INTO n VALUES (1, 1, 1), (2, NULL, 1), (3, NULL, NULL), (4, 2, NULL)",
    )
    .await;
    assert_eq!(
        q(&e, "SELECT id FROM n WHERE a IS NULL ORDER BY id").await,
        vec![vec!["2"], vec!["3"]]
    );
    assert_eq!(
        q(&e, "SELECT id FROM n WHERE a IS NOT NULL AND b IS NULL").await,
        vec![vec!["4"]]
    );
    assert_eq!(
        q(
            &e,
            "SELECT id FROM n WHERE a IS NOT DISTINCT FROM b ORDER BY id"
        )
        .await,
        vec![vec!["1"], vec!["3"]]
    );
    assert_eq!(
        q(&e, "SELECT id FROM n WHERE a = b").await,
        vec![vec!["1"]],
        "NULL = NULL is not true"
    );
    assert_eq!(
        q(&e, "SELECT id FROM n WHERE a <> b").await,
        Vec::<Vec<String>>::new(),
        "comparisons with NULL are not true"
    );
}

#[tokio::test]
async fn ddl_rolls_back_and_alter_table_reshapes_rows() {
    let e = seeded().await;
    let mut s = Session::default();
    run(&e, &mut s, "BEGIN").await.unwrap();
    run(&e, &mut s, "CREATE TABLE tmp (id INTEGER)")
        .await
        .unwrap();
    run(&e, &mut s, "DROP TABLE w").await.unwrap();
    run(&e, &mut s, "ROLLBACK").await.unwrap();
    assert_eq!(q(&e, "SELECT count(*) FROM w").await, vec![vec!["4"]]);
    assert!(msg(run(&e, &mut Session::default(), "SELECT * FROM tmp").await).contains("[42P01]"));

    x(&e, "ALTER TABLE w ADD COLUMN note VARCHAR DEFAULT 'n/a'").await;
    assert_eq!(
        q(&e, "SELECT note FROM w WHERE id = 1").await,
        vec![vec!["n/a"]]
    );
    x(&e, "ALTER TABLE w DROP COLUMN n").await;
    assert_eq!(
        q(&e, "SELECT id, q, note FROM w WHERE id = 2").await,
        vec![vec!["2", "20", "n/a"]]
    );
    x(&e, "CREATE UNIQUE INDEX w_q ON w (q)").await;
    assert!(msg(run(
        &e,
        &mut Session::default(),
        "INSERT INTO w (id, q) VALUES (9, 10)"
    )
    .await)
    .contains("[23505]"));
    x(&e, "TRUNCATE TABLE w").await;
    assert_eq!(q(&e, "SELECT count(*) FROM w").await, vec![vec!["0"]]);
}

#[cfg(feature = "storage-rocksdb")]
mod durability {
    use super::*;

    fn open(dir: &std::path::Path) -> DurableEngine {
        DurableEngine::open(Box::new(RocksPersist::open(dir).unwrap())).unwrap()
    }

    #[tokio::test]
    async fn data_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        {
            let e = open(dir.path());
            x(
                &e,
                "CREATE TABLE d (id SERIAL PRIMARY KEY, name VARCHAR UNIQUE, n INTEGER)",
            )
            .await;
            x(
                &e,
                "INSERT INTO d (name, n) VALUES ('a', 1), ('b', 2), ('c', 3)",
            )
            .await;
            x(&e, "UPDATE d SET n = 20 WHERE name = 'b'").await;
            x(&e, "DELETE FROM d WHERE name = 'c'").await;
            x(&e, "ALTER TABLE d ADD COLUMN extra VARCHAR DEFAULT 'x'").await;
        }
        let e = open(dir.path());
        assert_eq!(
            q(&e, "SELECT id, name, n, extra FROM d ORDER BY id").await,
            vec![vec!["1", "a", "1", "x"], vec!["2", "b", "20", "x"]]
        );
        // the unique index and the sequence were rebuilt: no duplicates, no reused ids
        assert!(run(
            &e,
            &mut Session::default(),
            "INSERT INTO d (name) VALUES ('a')"
        )
        .await
        .is_err());
        let r = run(
            &e,
            &mut Session::default(),
            "INSERT INTO d (name) VALUES ('d') RETURNING id",
        )
        .await
        .unwrap();
        let id: i64 = cells(r)[0][0].parse().unwrap();
        assert!(
            id > 3,
            "sequence must not reuse ids handed out before the restart, got {id}"
        );
    }

    #[tokio::test]
    async fn rolled_back_and_uncommitted_work_is_not_durable() {
        let dir = tempfile::tempdir().unwrap();
        {
            let e = open(dir.path());
            x(&e, "CREATE TABLE d (id INTEGER PRIMARY KEY)").await;
            x(&e, "INSERT INTO d VALUES (1)").await;
            let mut s = Session::default();
            run(&e, &mut s, "BEGIN").await.unwrap();
            run(&e, &mut s, "INSERT INTO d VALUES (2)").await.unwrap();
            run(&e, &mut s, "ROLLBACK").await.unwrap();
            let mut s2 = Session::default();
            run(&e, &mut s2, "BEGIN").await.unwrap();
            run(&e, &mut s2, "INSERT INTO d VALUES (3)").await.unwrap();
            // process "crashes" here: s2 is dropped without COMMIT
        }
        let e = open(dir.path());
        assert_eq!(q(&e, "SELECT id FROM d ORDER BY id").await, vec![vec!["1"]]);
    }

    #[tokio::test]
    async fn dropped_and_recreated_tables_do_not_mix_rows() {
        let dir = tempfile::tempdir().unwrap();
        {
            let e = open(dir.path());
            x(&e, "CREATE TABLE d (id INTEGER PRIMARY KEY, v VARCHAR)").await;
            x(&e, "INSERT INTO d VALUES (1, 'old')").await;
            x(&e, "DROP TABLE d").await;
            x(&e, "CREATE TABLE d (id INTEGER PRIMARY KEY, v VARCHAR)").await;
            x(&e, "INSERT INTO d VALUES (2, 'new')").await;
        }
        let e = open(dir.path());
        assert_eq!(q(&e, "SELECT id, v FROM d").await, vec![vec!["2", "new"]]);
    }
}

mod concurrency {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    async fn counter() -> Arc<DurableEngine> {
        let e = Arc::new(DurableEngine::in_memory());
        x(&e, "CREATE TABLE c (id INTEGER PRIMARY KEY, n INTEGER)").await;
        x(&e, "INSERT INTO c VALUES (1, 0)").await;
        e
    }

    #[tokio::test]
    async fn readers_are_not_blocked_by_an_open_writer_and_see_only_committed_data() {
        let e = counter().await;
        let mut writer = Session::default();
        run(&e, &mut writer, "BEGIN").await.unwrap();
        run(&e, &mut writer, "UPDATE c SET n = 5 WHERE id = 1")
            .await
            .unwrap();
        // the writer holds the gate; a reader still answers at once, from the committed state
        let read = tokio::time::timeout(Duration::from_secs(2), async {
            q(&e, "SELECT n FROM c").await
        })
        .await;
        assert_eq!(
            read.expect("a reader must not wait for a writer"),
            vec![vec!["0"]]
        );
        // the writer sees its own change
        assert_eq!(
            cells(run(&e, &mut writer, "SELECT n FROM c").await.unwrap()),
            vec![vec!["5"]]
        );
        run(&e, &mut writer, "COMMIT").await.unwrap();
        assert_eq!(q(&e, "SELECT n FROM c").await, vec![vec!["5"]]);
    }

    #[tokio::test]
    async fn a_second_writer_waits_for_the_first() {
        let e = counter().await;
        let mut a = Session::default();
        run(&e, &mut a, "BEGIN").await.unwrap();
        run(&e, &mut a, "UPDATE c SET n = n + 1 WHERE id = 1")
            .await
            .unwrap();
        let e2 = Arc::clone(&e);
        let b = tokio::spawn(async move {
            let mut s = Session::default();
            run(&e2, &mut s, "UPDATE c SET n = n + 10 WHERE id = 1")
                .await
                .unwrap();
        });
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(
            !b.is_finished(),
            "the second writer must wait for the first to finish"
        );
        run(&e, &mut a, "COMMIT").await.unwrap();
        b.await.unwrap();
        assert_eq!(
            q(&e, "SELECT n FROM c").await,
            vec![vec!["11"]],
            "neither update is lost"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_increments_lose_nothing() {
        let e = counter().await;
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let e = Arc::clone(&e);
            tasks.push(tokio::spawn(async move {
                for _ in 0..25 {
                    let mut s = Session::default();
                    run(&e, &mut s, "BEGIN").await.unwrap();
                    run(&e, &mut s, "UPDATE c SET n = n + 1 WHERE id = 1")
                        .await
                        .unwrap();
                    run(&e, &mut s, "COMMIT").await.unwrap();
                }
            }));
        }
        for t in tasks {
            t.await.unwrap();
        }
        assert_eq!(q(&e, "SELECT n FROM c").await, vec![vec!["200"]]);
    }

    #[tokio::test]
    async fn read_committed_sees_new_commits_between_statements() {
        let e = counter().await;
        let mut r = Session::default();
        run(&e, &mut r, "BEGIN").await.unwrap();
        assert_eq!(
            cells(run(&e, &mut r, "SELECT n FROM c").await.unwrap()),
            vec![vec!["0"]]
        );
        x(&e, "UPDATE c SET n = 7 WHERE id = 1").await;
        assert_eq!(
            cells(run(&e, &mut r, "SELECT n FROM c").await.unwrap()),
            vec![vec!["7"]]
        );
        run(&e, &mut r, "COMMIT").await.unwrap();
    }

    #[tokio::test]
    async fn repeatable_read_keeps_its_snapshot_and_refuses_a_stale_write() {
        let e = counter().await;
        let mut r = Session::default();
        run(&e, &mut r, "BEGIN ISOLATION LEVEL REPEATABLE READ")
            .await
            .unwrap();
        assert_eq!(
            cells(run(&e, &mut r, "SELECT n FROM c").await.unwrap()),
            vec![vec!["0"]]
        );
        x(&e, "UPDATE c SET n = 7 WHERE id = 1").await;
        assert_eq!(
            cells(run(&e, &mut r, "SELECT n FROM c").await.unwrap()),
            vec![vec!["0"]],
            "the snapshot does not move"
        );
        let stale = msg(run(&e, &mut r, "UPDATE c SET n = 1 WHERE id = 1").await);
        assert!(stale.contains("[40001]"), "{stale}");
        run(&e, &mut r, "ROLLBACK").await.unwrap();
        assert_eq!(q(&e, "SELECT n FROM c").await, vec![vec!["7"]]);
        // a fresh repeatable-read transaction can write
        let mut r2 = Session::default();
        run(&e, &mut r2, "BEGIN ISOLATION LEVEL REPEATABLE READ")
            .await
            .unwrap();
        run(&e, &mut r2, "UPDATE c SET n = 8 WHERE id = 1")
            .await
            .unwrap();
        run(&e, &mut r2, "COMMIT").await.unwrap();
        assert_eq!(q(&e, "SELECT n FROM c").await, vec![vec!["8"]]);
    }

    #[tokio::test]
    async fn a_session_dropped_mid_transaction_releases_the_writer() {
        let e = counter().await;
        {
            let mut s = Session::default();
            run(&e, &mut s, "BEGIN").await.unwrap();
            run(&e, &mut s, "UPDATE c SET n = 99 WHERE id = 1")
                .await
                .unwrap();
        }
        let done = tokio::time::timeout(Duration::from_secs(2), async {
            x(&e, "UPDATE c SET n = 1 WHERE id = 1").await
        })
        .await;
        assert!(
            done.is_ok(),
            "the gate must be free after the session is dropped"
        );
        assert_eq!(q(&e, "SELECT n FROM c").await, vec![vec!["1"]]);
    }
}

mod replication_unit {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A store that records every committed batch, as a replicated store would ship it.
    #[derive(Default)]
    struct Capture(Arc<Mutex<Vec<Vec<Op>>>>);

    #[async_trait::async_trait]
    impl Persist for Capture {
        async fn commit(&self, ops: &[Op]) -> Result<(), String> {
            self.0.lock().unwrap().push(ops.to_vec());
            Ok(())
        }
        fn load(&self) -> Result<Loaded, String> {
            Ok(Loaded::default())
        }
    }

    #[tokio::test]
    async fn replaying_the_committed_ops_reproduces_the_database() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let a = DurableEngine::open(Box::new(Capture(Arc::clone(&log)))).unwrap();
        x(
            &a,
            "CREATE TABLE p (id SERIAL PRIMARY KEY, name VARCHAR UNIQUE, n INTEGER)",
        )
        .await;
        x(
            &a,
            "CREATE TABLE o (id SERIAL PRIMARY KEY, p_id INTEGER, amt INTEGER)",
        )
        .await;
        x(
            &a,
            "INSERT INTO p (name, n) VALUES ('a', 1), ('b', 2), ('c', 3)",
        )
        .await;
        x(
            &a,
            "INSERT INTO o (p_id, amt) VALUES (1, 10), (1, 20), (2, 5)",
        )
        .await;
        x(&a, "UPDATE p SET n = n * 10 WHERE name <> 'a'").await;
        x(&a, "DELETE FROM o WHERE amt = 5").await;
        x(&a, "ALTER TABLE p ADD COLUMN note VARCHAR DEFAULT 'x'").await;
        x(&a, "ALTER TABLE o DROP COLUMN amt").await;
        x(&a, "CREATE INDEX o_p ON o (p_id)").await;
        let mut s = Session::default();
        run(&a, &mut s, "BEGIN").await.unwrap();
        run(&a, &mut s, "INSERT INTO p (name) VALUES ('d')")
            .await
            .unwrap();
        run(&a, &mut s, "TRUNCATE TABLE o").await.unwrap();
        run(&a, &mut s, "COMMIT").await.unwrap();
        x(&a, "DROP TABLE o").await;
        x(&a, "CREATE TABLE o (id SERIAL PRIMARY KEY, label VARCHAR)").await;
        x(&a, "INSERT INTO o (label) VALUES ('fresh')").await;

        let b = DurableEngine::in_memory();
        for batch in log.lock().unwrap().iter() {
            b.apply_replicated(batch).unwrap();
        }
        for sql in [
            "SELECT id, name, n, note FROM p ORDER BY id",
            "SELECT id, label FROM o ORDER BY id",
        ] {
            assert_eq!(q(&a, sql).await, q(&b, sql).await, "{sql}");
        }
        // the replica enforces the same unique index and, once it takes over, continues the sequences
        assert!(run(
            &b,
            &mut Session::default(),
            "INSERT INTO p (name) VALUES ('a')"
        )
        .await
        .is_err());
        b.became_leader();
        let next_a = cells(
            run(
                &a,
                &mut Session::default(),
                "INSERT INTO p (name) VALUES ('e') RETURNING id",
            )
            .await
            .unwrap(),
        );
        let next_b = cells(
            run(
                &b,
                &mut Session::default(),
                "INSERT INTO p (name) VALUES ('f') RETURNING id",
            )
            .await
            .unwrap(),
        );
        let (ia, ib): (i64, i64) = (next_a[0][0].parse().unwrap(), next_b[0][0].parse().unwrap());
        assert!(
            ib > 4,
            "a promoted replica must not reuse ids, got {ib} (leader handed out {ia})"
        );
    }
}

mod foreign_keys {
    use super::*;

    async fn family() -> DurableEngine {
        let e = DurableEngine::in_memory();
        x(&e, "CREATE TABLE p (id SERIAL PRIMARY KEY, name VARCHAR)").await;
        x(
            &e,
            "CREATE TABLE c (id SERIAL PRIMARY KEY, p_id INTEGER, note VARCHAR)",
        )
        .await;
        x(&e, "ALTER TABLE c ADD CONSTRAINT c_p_id_fkey FOREIGN KEY (p_id) REFERENCES p (id) ON DELETE CASCADE ON UPDATE CASCADE").await;
        x(&e, "INSERT INTO p (name) VALUES ('a'), ('b')").await;
        x(
            &e,
            "INSERT INTO c (p_id, note) VALUES (1, 'x'), (1, 'y'), (2, 'z'), (NULL, 'orphan')",
        )
        .await;
        e
    }

    #[tokio::test]
    async fn a_child_must_reference_an_existing_parent() {
        let e = family().await;
        let bad = msg(run(
            &e,
            &mut Session::default(),
            "INSERT INTO c (p_id) VALUES (99)",
        )
        .await);
        assert!(
            bad.contains("[23503]") && bad.contains("c_p_id_fkey"),
            "{bad}"
        );
        let bad = msg(run(
            &e,
            &mut Session::default(),
            "UPDATE c SET p_id = 99 WHERE note = 'x'",
        )
        .await);
        assert!(bad.contains("[23503]"), "{bad}");
        assert_eq!(
            q(&e, "SELECT count(*) FROM c").await,
            vec![vec!["4"]],
            "NULL satisfies the key and a failed statement changes nothing"
        );
    }

    #[tokio::test]
    async fn adding_a_key_checks_existing_rows() {
        let e = DurableEngine::in_memory();
        x(&e, "CREATE TABLE p (id INTEGER PRIMARY KEY)").await;
        x(&e, "CREATE TABLE c (id INTEGER PRIMARY KEY, p_id INTEGER)").await;
        x(&e, "INSERT INTO p VALUES (1)").await;
        x(&e, "INSERT INTO c VALUES (1, 1), (2, 2)").await;
        let bad = msg(run(
            &e,
            &mut Session::default(),
            "ALTER TABLE c ADD CONSTRAINT k FOREIGN KEY (p_id) REFERENCES p (id)",
        )
        .await);
        assert!(bad.contains("[23503]"), "{bad}");
        x(&e, "DELETE FROM c WHERE id = 2").await;
        x(
            &e,
            "ALTER TABLE c ADD CONSTRAINT k FOREIGN KEY (p_id) REFERENCES p (id)",
        )
        .await;
        assert!(
            run(&e, &mut Session::default(), "INSERT INTO c VALUES (3, 7)")
                .await
                .is_err()
        );
        // a key must point at a unique column
        x(&e, "CREATE TABLE loose (id INTEGER, v INTEGER)").await;
        let bad = msg(run(
            &e,
            &mut Session::default(),
            "ALTER TABLE c ADD CONSTRAINT k2 FOREIGN KEY (p_id) REFERENCES loose (v)",
        )
        .await);
        assert!(bad.contains("[42830]"), "{bad}");
    }

    #[tokio::test]
    async fn delete_and_update_follow_the_declared_actions() {
        let e = family().await;
        x(&e, "UPDATE p SET id = 10 WHERE id = 2").await;
        assert_eq!(
            q(&e, "SELECT p_id FROM c WHERE note = 'z'").await,
            vec![vec!["10"]],
            "ON UPDATE CASCADE"
        );
        x(&e, "DELETE FROM p WHERE id = 1").await;
        assert_eq!(
            q(&e, "SELECT note FROM c ORDER BY id").await,
            vec![vec!["z"], vec!["orphan"]],
            "ON DELETE CASCADE"
        );

        let e = DurableEngine::in_memory();
        x(&e, "CREATE TABLE p (id INTEGER PRIMARY KEY)").await;
        x(&e, "CREATE TABLE n (id INTEGER PRIMARY KEY, p_id INTEGER REFERENCES p (id) ON DELETE SET NULL)").await;
        x(&e, "CREATE TABLE r (id INTEGER PRIMARY KEY, p_id INTEGER REFERENCES p (id) ON DELETE RESTRICT)").await;
        x(&e, "INSERT INTO p VALUES (1), (2)").await;
        x(&e, "INSERT INTO n VALUES (1, 1)").await;
        x(&e, "INSERT INTO r VALUES (1, 2)").await;
        x(&e, "DELETE FROM p WHERE id = 1").await;
        assert_eq!(
            q(&e, "SELECT p_id FROM n").await,
            vec![vec!["NULL"]],
            "ON DELETE SET NULL"
        );
        let bad = msg(run(&e, &mut Session::default(), "DELETE FROM p WHERE id = 2").await);
        assert!(bad.contains("[23503]"), "{bad}");
        assert_eq!(
            q(&e, "SELECT count(*) FROM p").await,
            vec![vec!["1"]],
            "RESTRICT leaves the parent in place"
        );
    }

    #[tokio::test]
    async fn cascades_reach_grandchildren_and_self_references_work() {
        let e = DurableEngine::in_memory();
        x(&e, "CREATE TABLE a (id INTEGER PRIMARY KEY)").await;
        x(&e, "CREATE TABLE b (id INTEGER PRIMARY KEY, a_id INTEGER REFERENCES a (id) ON DELETE CASCADE)").await;
        x(&e, "CREATE TABLE c (id INTEGER PRIMARY KEY, b_id INTEGER REFERENCES b (id) ON DELETE CASCADE)").await;
        x(&e, "INSERT INTO a VALUES (1)").await;
        x(&e, "INSERT INTO b VALUES (1, 1), (2, 1)").await;
        x(&e, "INSERT INTO c VALUES (1, 1), (2, 2)").await;
        x(&e, "DELETE FROM a WHERE id = 1").await;
        assert_eq!(q(&e, "SELECT count(*) FROM c").await, vec![vec!["0"]]);

        x(&e, "CREATE TABLE tree (id INTEGER PRIMARY KEY, parent INTEGER REFERENCES tree (id) ON DELETE CASCADE)").await;
        x(&e, "INSERT INTO tree VALUES (1, 1)").await; // a row may reference itself
        x(&e, "INSERT INTO tree VALUES (2, 1), (3, 2)").await;
        x(&e, "DELETE FROM tree WHERE id = 1").await;
        assert_eq!(q(&e, "SELECT count(*) FROM tree").await, vec![vec!["0"]]);
    }

    #[tokio::test]
    async fn dropping_or_truncating_a_referenced_table_needs_cascade() {
        let e = family().await;
        let bad = msg(run(&e, &mut Session::default(), "DROP TABLE p").await);
        assert!(bad.contains("[2BP01]"), "{bad}");
        let bad = msg(run(&e, &mut Session::default(), "TRUNCATE TABLE p").await);
        assert!(bad.contains("[0A000]"), "{bad}");
        x(&e, "TRUNCATE TABLE p CASCADE").await;
        assert_eq!(q(&e, "SELECT count(*) FROM c").await, vec![vec!["0"]]);
        x(&e, "DROP TABLE p CASCADE").await;
        x(&e, "INSERT INTO c (p_id) VALUES (123)").await; // the key is gone with its parent
    }

    #[tokio::test]
    async fn a_replica_reaches_the_same_state_through_cascades() {
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        struct Capture(std::sync::Arc<std::sync::Mutex<Vec<Vec<Op>>>>);
        #[async_trait::async_trait]
        impl Persist for Capture {
            async fn commit(&self, ops: &[Op]) -> Result<(), String> {
                self.0.lock().unwrap().push(ops.to_vec());
                Ok(())
            }
            fn load(&self) -> Result<Loaded, String> {
                Ok(Loaded::default())
            }
        }
        let a = DurableEngine::open(Box::new(Capture(std::sync::Arc::clone(&log)))).unwrap();
        x(&a, "CREATE TABLE p (id SERIAL PRIMARY KEY, name VARCHAR)").await;
        x(&a, "CREATE TABLE c (id SERIAL PRIMARY KEY, p_id INTEGER REFERENCES p (id) ON DELETE CASCADE)").await;
        x(&a, "INSERT INTO p (name) VALUES ('a'), ('b')").await;
        x(&a, "INSERT INTO c (p_id) VALUES (1), (1), (2)").await;
        x(&a, "DELETE FROM p WHERE id = 1").await;
        let b = DurableEngine::in_memory();
        for batch in log.lock().unwrap().iter() {
            b.apply_replicated(batch).unwrap();
        }
        assert_eq!(
            q(&b, "SELECT id, p_id FROM c ORDER BY id").await,
            q(&a, "SELECT id, p_id FROM c ORDER BY id").await
        );
        assert!(
            run(
                &b,
                &mut Session::default(),
                "INSERT INTO c (p_id) VALUES (77)"
            )
            .await
            .is_err(),
            "the replica enforces the key too"
        );
    }
}

mod catalog {
    use super::*;

    #[tokio::test]
    async fn information_schema_lists_tables_and_columns_as_symphony_asks() {
        let e = DurableEngine::in_memory();
        x(
            &e,
            "CREATE TABLE res_partner (id SERIAL PRIMARY KEY, name VARCHAR NOT NULL, note TEXT)",
        )
        .await;
        x(
            &e,
            "CREATE TABLE res_users (id SERIAL PRIMARY KEY, login VARCHAR)",
        )
        .await;
        // the exact query Symphony sends to PostgreSQL to learn what exists
        let got = q(&e, "SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = current_schema() ORDER BY table_name, ordinal_position").await;
        assert_eq!(
            got,
            vec![
                vec!["res_partner", "id"],
                vec!["res_partner", "name"],
                vec!["res_partner", "note"],
                vec!["res_users", "id"],
                vec!["res_users", "login"]
            ]
        );
        assert_eq!(q(&e, "SELECT column_name, is_nullable, data_type FROM information_schema.columns WHERE table_name = 'res_partner' ORDER BY ordinal_position").await,
            vec![vec!["id", "NO", "integer"], vec!["name", "NO", "character varying"], vec!["note", "YES", "text"]]);
        assert_eq!(q(&e, "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' ORDER BY table_name").await, vec![vec!["res_partner"], vec!["res_users"]]);
        assert_eq!(
            q(&e, "SELECT current_schema(), current_database()").await,
            vec![vec!["public", "orbit"]]
        );
        x(&e, "ALTER TABLE res_partner ADD COLUMN extra VARCHAR").await;
        assert_eq!(
            q(
                &e,
                "SELECT count(*) FROM information_schema.columns WHERE table_name = 'res_partner'"
            )
            .await,
            vec![vec!["4"]]
        );
    }
}

#[cfg(feature = "storage-rocksdb")]
mod backup {
    use super::*;

    #[tokio::test]
    async fn a_backup_opens_as_the_database_it_was_taken_from() {
        let dir = tempfile::tempdir().unwrap();
        let e = DurableEngine::open(Box::new(
            RocksPersist::open(dir.path().join("live")).unwrap(),
        ))
        .unwrap();
        x(
            &e,
            "CREATE TABLE b (id SERIAL PRIMARY KEY, v VARCHAR UNIQUE)",
        )
        .await;
        x(&e, "INSERT INTO b (v) VALUES ('one'), ('two')").await;
        let dest = dir.path().join("copy");
        e.backup(&dest).unwrap();
        x(&e, "INSERT INTO b (v) VALUES ('after the backup')").await;
        assert!(
            e.backup(&dest).is_err(),
            "an existing destination is never overwritten"
        );

        let restored = DurableEngine::open(Box::new(RocksPersist::open(&dest).unwrap())).unwrap();
        assert_eq!(
            q(&restored, "SELECT id, v FROM b ORDER BY id").await,
            vec![vec!["1", "one"], vec!["2", "two"]]
        );
        assert!(
            run(
                &restored,
                &mut Session::default(),
                "INSERT INTO b (v) VALUES ('one')"
            )
            .await
            .is_err(),
            "indexes come back too"
        );
    }
}

mod more_sql {
    use super::*;

    async fn tables() -> DurableEngine {
        let e = DurableEngine::in_memory();
        x(
            &e,
            "CREATE TABLE a (id INTEGER PRIMARY KEY, x INTEGER, y INTEGER, tag VARCHAR)",
        )
        .await;
        x(
            &e,
            "CREATE TABLE b (id INTEGER PRIMARY KEY, a_id INTEGER, v INTEGER)",
        )
        .await;
        x(
            &e,
            "INSERT INTO a VALUES (1, 0, 0, 'p'), (2, 0, 0, 'q'), (3, 0, 0, 'r')",
        )
        .await;
        x(
            &e,
            "INSERT INTO b VALUES (10, 1, 100), (11, 2, 200), (12, 2, 300)",
        )
        .await;
        e
    }

    #[tokio::test]
    async fn update_from_changes_rows_using_a_joined_table() {
        let e = tables().await;
        x(
            &e,
            "UPDATE a SET x = b.v FROM b WHERE b.a_id = a.id AND b.v < 300",
        )
        .await;
        assert_eq!(
            q(&e, "SELECT id, x FROM a ORDER BY id").await,
            vec![vec!["1", "100"], vec!["2", "200"], vec!["3", "0"]],
            "rows with no match are left alone"
        );
        let r = run(
            &e,
            &mut Session::default(),
            "UPDATE a SET y = 1 FROM b WHERE b.a_id = a.id RETURNING a.id",
        )
        .await
        .unwrap();
        assert_eq!(
            cells(r).len(),
            2,
            "each target row is updated once however many rows match"
        );
    }

    #[tokio::test]
    async fn delete_using_removes_rows_that_match_a_joined_table() {
        let e = tables().await;
        x(
            &e,
            "DELETE FROM a USING b WHERE b.a_id = a.id AND b.v = 200",
        )
        .await;
        assert_eq!(
            q(&e, "SELECT id FROM a ORDER BY id").await,
            vec![vec!["1"], vec!["3"]]
        );
    }

    #[tokio::test]
    async fn multi_column_set_takes_a_row_or_a_subquery() {
        let e = tables().await;
        x(&e, "UPDATE a SET (x, y) = (7, 8) WHERE id = 1").await;
        assert_eq!(
            q(&e, "SELECT x, y FROM a WHERE id = 1").await,
            vec![vec!["7", "8"]]
        );
        x(
            &e,
            "UPDATE a SET (x, y) = (SELECT v, a_id FROM b WHERE id = 11) WHERE id = 2",
        )
        .await;
        assert_eq!(
            q(&e, "SELECT x, y FROM a WHERE id = 2").await,
            vec![vec!["200", "2"]]
        );
        assert!(
            msg(run(&e, &mut Session::default(), "UPDATE a SET (x, y) = (1)").await)
                .contains("[42601]")
        );
    }

    #[tokio::test]
    async fn alter_column_changes_nullability_default_and_type() {
        let e = tables().await;
        x(&e, "ALTER TABLE a ALTER COLUMN tag SET NOT NULL").await;
        assert!(
            msg(run(&e, &mut Session::default(), "INSERT INTO a (id) VALUES (9)").await)
                .contains("[23502]")
        );
        x(&e, "ALTER TABLE a ALTER COLUMN tag DROP NOT NULL").await;
        x(&e, "ALTER TABLE a ALTER COLUMN tag SET DEFAULT 'none'").await;
        x(&e, "INSERT INTO a (id) VALUES (9)").await;
        assert_eq!(
            q(&e, "SELECT tag FROM a WHERE id = 9").await,
            vec![vec!["none"]]
        );
        x(&e, "ALTER TABLE a ALTER COLUMN tag DROP DEFAULT").await;
        x(&e, "INSERT INTO a (id) VALUES (10)").await;
        assert_eq!(
            q(&e, "SELECT tag FROM a WHERE id = 10").await,
            vec![vec!["NULL"]]
        );
        x(&e, "ALTER TABLE a ALTER COLUMN x TYPE BIGINT").await;
        assert_eq!(
            q(&e, "SELECT count(*) FROM a WHERE x = 0").await,
            vec![vec!["3"]],
            "the converted column keeps its values; the two new rows have NULL"
        );
        // with rows holding NULLs, SET NOT NULL is refused
        assert!(msg(run(
            &e,
            &mut Session::default(),
            "ALTER TABLE a ALTER COLUMN tag SET NOT NULL"
        )
        .await)
        .contains("[23502]"));
        assert!(msg(run(
            &e,
            &mut Session::default(),
            "ALTER TABLE a ALTER COLUMN id DROP NOT NULL"
        )
        .await)
        .contains("[42P16]"));
    }
}

mod windows {
    use super::*;

    async fn sales() -> DurableEngine {
        let e = DurableEngine::in_memory();
        x(
            &e,
            "CREATE TABLE s (id INTEGER PRIMARY KEY, region VARCHAR, amt INTEGER)",
        )
        .await;
        x(
            &e,
            "INSERT INTO s VALUES (1,'n',10),(2,'n',20),(3,'n',20),(4,'s',5),(5,'s',15)",
        )
        .await;
        e
    }

    #[tokio::test]
    async fn ranking_functions_respect_partitions_and_ties() {
        let e = sales().await;
        assert_eq!(
            q(&e, "SELECT id, row_number() OVER (PARTITION BY region ORDER BY amt DESC, id) AS rn, rank() OVER (PARTITION BY region ORDER BY amt DESC) AS rk, dense_rank() OVER (PARTITION BY region ORDER BY amt DESC) AS dr FROM s ORDER BY id").await,
            vec![vec!["1", "3", "3", "2"], vec!["2", "1", "1", "1"], vec!["3", "2", "1", "1"], vec!["4", "2", "2", "2"], vec!["5", "1", "1", "1"]]
        );
    }

    #[tokio::test]
    async fn aggregates_over_a_window_use_the_default_and_explicit_frames() {
        let e = sales().await;
        // no ORDER BY: the whole partition; with ORDER BY: a running total that includes ties
        assert_eq!(
            q(
                &e,
                "SELECT id, sum(amt) OVER (PARTITION BY region) FROM s ORDER BY id"
            )
            .await,
            vec![
                vec!["1", "50"],
                vec!["2", "50"],
                vec!["3", "50"],
                vec!["4", "20"],
                vec!["5", "20"]
            ]
        );
        assert_eq!(
            q(
                &e,
                "SELECT id, sum(amt) OVER (PARTITION BY region ORDER BY amt) FROM s ORDER BY id"
            )
            .await,
            vec![
                vec!["1", "10"],
                vec!["2", "50"],
                vec!["3", "50"],
                vec!["4", "5"],
                vec!["5", "20"]
            ]
        );
        assert_eq!(q(&e, "SELECT id, sum(amt) OVER (ORDER BY id ROWS BETWEEN 1 PRECEDING AND CURRENT ROW) FROM s ORDER BY id").await,
            vec![vec!["1", "10"], vec!["2", "30"], vec!["3", "40"], vec!["4", "25"], vec!["5", "20"]]);
        assert_eq!(
            q(&e, "SELECT count(*) OVER () FROM s LIMIT 1").await,
            vec![vec!["5"]]
        );
    }

    #[tokio::test]
    async fn lag_lead_first_and_last_value() {
        let e = sales().await;
        assert_eq!(
            q(&e, "SELECT id, lag(amt) OVER (ORDER BY id), lead(amt, 2, -1) OVER (ORDER BY id), first_value(amt) OVER (PARTITION BY region ORDER BY id) FROM s ORDER BY id").await,
            vec![vec!["1", "NULL", "20", "10"], vec!["2", "10", "5", "10"], vec!["3", "20", "15", "10"], vec!["4", "20", "-1", "5"], vec!["5", "5", "-1", "5"]]
        );
    }

    #[tokio::test]
    async fn a_window_value_can_be_used_inside_an_expression_and_ordered_by() {
        let e = sales().await;
        assert_eq!(q(&e, "SELECT id, amt * 100 / sum(amt) OVER () AS pct FROM s ORDER BY pct DESC, id LIMIT 2").await, vec![vec!["2", "28"], vec!["3", "28"]], "20 * 100 / 70 is integer division");
        assert_eq!(
            q(
                &e,
                "SELECT id FROM s WHERE amt > 5 ORDER BY row_number() OVER (ORDER BY amt DESC, id)"
            )
            .await,
            vec![vec!["2"], vec!["3"], vec!["5"], vec!["1"]]
        );
    }
}

mod arithmetic {
    use super::*;

    #[tokio::test]
    async fn numbers_of_different_types_combine_the_way_postgresql_does() {
        let e = DurableEngine::in_memory();
        x(
            &e,
            "CREATE TABLE n (i INTEGER, b BIGINT, d NUMERIC(12,2), f DOUBLE PRECISION)",
        )
        .await;
        x(&e, "INSERT INTO n VALUES (7, 3000000000, 10.50, 2.5)").await;
        assert_eq!(
            q(&e, "SELECT i / 2, i % 4, i * 3 FROM n").await,
            vec![vec!["3", "3", "21"]],
            "integer division truncates"
        );
        assert_eq!(
            q(&e, "SELECT b + i, b / i FROM n").await,
            vec![vec!["3000000007", "428571428"]]
        );
        assert_eq!(
            q(&e, "SELECT d * 2, d + i, d - 0.5 FROM n").await,
            vec![vec!["21.00", "17.50", "10.00"]]
        );
        assert_eq!(
            q(&e, "SELECT i / d FROM n").await,
            vec![vec!["0.6666666666666667"]]
        );
        assert_eq!(
            q(&e, "SELECT f * 2, f + i, d * f FROM n").await,
            vec![vec!["5", "9.5", "26.25"]]
        );
        let r = run(&e, &mut Session::default(), "SELECT i FROM n")
            .await
            .unwrap();
        assert!(matches!(r, QueryResult::Typed { .. }));
        assert!(
            msg(run(&e, &mut Session::default(), "SELECT i / 0 FROM n").await).contains("[22012]")
        );
        assert!(
            msg(run(&e, &mut Session::default(), "SELECT 2147483647 + 1").await)
                .contains("[22003]")
        );
        assert_eq!(
            q(&e, "SELECT 2147483647::bigint + 1").await,
            vec![vec!["2147483648"]]
        );
    }
}

#[tokio::test]
async fn big_integer_literals_and_numeric_scale_are_kept() {
    let e = DurableEngine::in_memory();
    x(&e, "CREATE TABLE big (b BIGINT, d NUMERIC(8,2))").await;
    x(&e, "INSERT INTO big VALUES (3000000000, 10.5), (9223372036854775807, 0.005), (-5000000000, 1234.567)").await;
    assert_eq!(
        q(&e, "SELECT b, d FROM big ORDER BY b").await,
        vec![
            vec!["-5000000000", "1234.57"],
            vec!["3000000000", "10.50"],
            vec!["9223372036854775807", "0.01"]
        ]
    );
    assert!(msg(run(
        &e,
        &mut Session::default(),
        "INSERT INTO big (d) VALUES (1234567.1)"
    )
    .await)
    .contains("numeric field overflow"));
}
