use orbit_server::OrbitServer;
use redis::Commands;
use tokio::time::sleep;

async fn cleanup_lingering_instances() {
    let _ = std::process::Command::new("killall")
        .arg("orbit-server")
        .output();
    sleep(std::time::Duration::from_millis(500)).await;
}

async fn wait_for_port(port: u16, max_wait_secs: u64) -> bool {
    let start = std::time::Instant::now();
    let max_duration = std::time::Duration::from_secs(max_wait_secs);

    while start.elapsed() < max_duration {
        if std::net::TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            return true;
        }
        sleep(std::time::Duration::from_millis(200)).await;
    }
    false
}

#[tokio::test]
#[ignore = "slow integration test - starts full server"]
async fn test_list_commands() {
    cleanup_lingering_instances().await;

    // Start server using OrbitServer API
    let mut server = OrbitServer::builder()
        .with_redis_enabled(true)
        .with_redis_port(6379)
        .with_postgres_enabled(false)
        .build()
        .await
        .expect("Failed to create server");

    // Start server in background
    tokio::spawn(async move {
        if let Err(e) = server.start().await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for Redis protocol
    assert!(
        wait_for_port(6379, 15).await,
        "Redis port 6379 not listening"
    );

    // Connect with redis client
    let client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
    let mut con = client.get_connection().unwrap();

    // Test LPUSH and LPOP
    let _: () = con.del("mylist").unwrap();
    let len: isize = con.lpush("mylist", "world").unwrap();
    assert_eq!(len, 1);
    let len: isize = con.lpush("mylist", "hello").unwrap();
    assert_eq!(len, 2);

    let val: String = con.lpop("mylist", None).unwrap();
    assert_eq!(val, "hello");
    let val: String = con.lpop("mylist", None).unwrap();
    assert_eq!(val, "world");
    let val: Option<String> = con.lpop("mylist", None).unwrap();
    assert!(val.is_none());

    // Test RPUSH and RPOP
    let _: () = con.del("mylist").unwrap();
    let len: isize = con.rpush("mylist", "hello").unwrap();
    assert_eq!(len, 1);
    let len: isize = con.rpush("mylist", "world").unwrap();
    assert_eq!(len, 2);

    let val: String = con.rpop("mylist", None).unwrap();
    assert_eq!(val, "world");
    let val: String = con.rpop("mylist", None).unwrap();
    assert_eq!(val, "hello");

    // Test LRANGE
    let _: () = con.del("mylist").unwrap();
    let _: () = con.rpush("mylist", &["one", "two", "three"]).unwrap();
    let list: Vec<String> = con.lrange("mylist", 0, 0).unwrap();
    assert_eq!(list, vec!["one"]);
    let list: Vec<String> = con.lrange("mylist", -3, 2).unwrap();
    assert_eq!(list, vec!["one", "two", "three"]);

    // Test LLEN
    let len: isize = con.llen("mylist").unwrap();
    assert_eq!(len, 3);

    // Test LINDEX
    let val: String = con.lindex("mylist", 1).unwrap();
    assert_eq!(val, "two");
    let val: Option<String> = con.lindex("mylist", 100).unwrap();
    assert!(val.is_none());

    // Test LSET
    let _: () = con.lset("mylist", 1, "four").unwrap();
    let val: String = con.lindex("mylist", 1).unwrap();
    assert_eq!(val, "four");

    // Test LREM
    let _: () = con.rpush("mylist", "four").unwrap();
    let removed: isize = con.lrem("mylist", 1, "four").unwrap();
    assert_eq!(removed, 1);
    let len: isize = con.llen("mylist").unwrap();
    assert_eq!(len, 3);

    // Test LTRIM
    let _: () = con.del("mylist").unwrap();
    let _: () = con.rpush("mylist", &["one", "two", "three"]).unwrap();
    let _: () = con.ltrim("mylist", 1, -1).unwrap();
    let list: Vec<String> = con.lrange("mylist", 0, -1).unwrap();
    assert_eq!(list, vec!["two", "three"]);

    // Cleanup
    cleanup_lingering_instances().await;
}
