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
