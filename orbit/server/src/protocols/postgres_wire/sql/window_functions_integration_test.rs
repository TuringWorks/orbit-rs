#[cfg(test)]
mod tests {
    use crate::protocols::postgres_wire::sql::{SqlEngine, UnifiedExecutionResult};

    #[tokio::test]
    async fn test_window_frames_rows() {
        let mut engine = SqlEngine::new_traditional();

        engine
            .execute("CREATE TABLE frame_test (id INT, val INT)")
            .await
            .unwrap();
        // Insert: 1, 10; 2, 20; 3, 30; 4, 40; 5, 50
        engine
            .execute("INSERT INTO frame_test VALUES (1, 10), (2, 20), (3, 30), (4, 40), (5, 50)")
            .await
            .unwrap();

        // ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING
        let sql = "
            SELECT id, val, SUM(val) OVER (
                ORDER BY id 
                ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING
            ) as frame_sum
            FROM frame_test
            ORDER BY id
        ";

        let result = engine.execute(sql).await;
        match result {
            Ok(UnifiedExecutionResult::Select { rows, .. }) => {
                // Expected sums: 30, 60, 90, 120, 90
                // Note: rows are strings in UnifiedExecutionResult if defaulting to string format
                assert_eq!(rows.len(), 5);

                // Helper to get val at col index 2
                let get_sum =
                    |r: &Vec<Option<String>>| r[2].as_ref().unwrap().parse::<i64>().unwrap();

                assert_eq!(get_sum(&rows[0]), 30, "Row 1 sum mismatch");
                assert_eq!(get_sum(&rows[1]), 60, "Row 2 sum mismatch");
                assert_eq!(get_sum(&rows[2]), 90, "Row 3 sum mismatch");
                assert_eq!(get_sum(&rows[3]), 120, "Row 4 sum mismatch");
                assert_eq!(get_sum(&rows[4]), 90, "Row 5 sum mismatch");

                println!("ROWS frame test passed!");
            }
            Ok(res) => panic!("Unexpected result type: {:?}", res),
            Err(e) => panic!("Query failed: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_window_frames_range() {
        let mut engine = SqlEngine::new_traditional();

        engine
            .execute("CREATE TABLE range_test (id INT, val INT)")
            .await
            .unwrap();
        engine
            .execute("INSERT INTO range_test VALUES (1, 10), (1, 20), (2, 30)")
            .await
            .unwrap();

        // RANGE BETWEEN CURRENT ROW AND CURRENT ROW
        // Should include peers with same ORDER BY key (id)
        let sql = "
            SELECT id, val, SUM(val) OVER (
                ORDER BY id 
                RANGE BETWEEN CURRENT ROW AND CURRENT ROW
            ) as frame_sum
            FROM range_test
            ORDER BY id
        ";

        let result = engine.execute(sql).await;
        match result {
            Ok(UnifiedExecutionResult::Select { rows, .. }) => {
                // Expected:
                // id=1, val=10 -> sum=30
                // id=1, val=20 -> sum=30
                // id=2, val=30 -> sum=30

                assert_eq!(rows.len(), 3);
                let get_sum =
                    |r: &Vec<Option<String>>| r[2].as_ref().unwrap().parse::<i64>().unwrap();

                // Order of id=1 rows might vary if stable sort or insertion order preserved.
                // Sum should be 30 for all.
                assert_eq!(get_sum(&rows[0]), 30, "Row 1 sum mismatch");
                assert_eq!(get_sum(&rows[1]), 30, "Row 2 sum mismatch");
                assert_eq!(get_sum(&rows[2]), 30, "Row 3 sum mismatch");

                println!("RANGE frame test passed!");
            }
            Ok(res) => panic!("Unexpected result type: {:?}", res),
            Err(e) => panic!("Query failed: {:?}", e),
        }
    }
}
