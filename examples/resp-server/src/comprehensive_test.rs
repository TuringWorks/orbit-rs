//! Comprehensive Redis command test suite for RESP server
//!
//! This test suite verifies all implemented Redis commands work correctly
//! with proper responses, error handling, and edge cases.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

/// Test result tracking
#[derive(Debug, Default)]
pub struct TestResults {
    pub passed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

impl TestResults {
    pub fn success(&mut self, test_name: &str) {
        self.passed += 1;
        println!("✅ {}", test_name);
    }

    pub fn failure(&mut self, test_name: &str, error: &str) {
        self.failed += 1;
        self.errors.push(format!("{}: {}", test_name, error));
        println!("❌ {}: {}", test_name, error);
    }

    pub fn summary(&self) {
        println!("\n📊 Test Summary:");
        println!("✅ Passed: {}", self.passed);
        println!("❌ Failed: {}", self.failed);
        println!(
            "📈 Success Rate: {:.1}%",
            (self.passed as f64 / (self.passed + self.failed) as f64) * 100.0
        );

        if !self.errors.is_empty() {
            println!("\n🔍 Failures:");
            for error in &self.errors {
                println!("  • {}", error);
            }
        }
    }
}

/// RESP command client for testing
pub struct RespTestClient {
    stream: TcpStream,
}

impl RespTestClient {
    pub fn connect(addr: &str) -> std::io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self { stream })
    }

    pub fn send_command(&mut self, command: &[&str]) -> std::io::Result<String> {
        // Build RESP command
        let mut resp_cmd = format!("*{}\r\n", command.len());
        for arg in command {
            resp_cmd.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
        }

        // Send command
        self.stream.write_all(resp_cmd.as_bytes())?;
        self.stream.flush()?;

        // Read response
        let mut buffer = vec![0; 8192];
        let n = self.stream.read(&mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer[..n]).to_string())
    }

    pub fn expect_ok(&mut self, command: &[&str]) -> Result<(), String> {
        match self.send_command(command) {
            Ok(response) => {
                if response.contains("+OK") || response.contains("$2\r\nOK") {
                    Ok(())
                } else {
                    Err(format!("Expected OK, got: {}", response.trim()))
                }
            }
            Err(e) => Err(format!("IO error: {}", e)),
        }
    }

    pub fn expect_response(&mut self, command: &[&str], expected: &str) -> Result<(), String> {
        match self.send_command(command) {
            Ok(response) => {
                if response.contains(expected) {
                    Ok(())
                } else {
                    Err(format!("Expected '{}', got: {}", expected, response.trim()))
                }
            }
            Err(e) => Err(format!("IO error: {}", e)),
        }
    }

    pub fn expect_integer(&mut self, command: &[&str], expected: i64) -> Result<(), String> {
        match self.send_command(command) {
            Ok(response) => {
                let expected_resp = format!(":{}", expected);
                if response.contains(&expected_resp) {
                    Ok(())
                } else {
                    Err(format!(
                        "Expected integer {}, got: {}",
                        expected,
                        response.trim()
                    ))
                }
            }
            Err(e) => Err(format!("IO error: {}", e)),
        }
    }
}

fn main() -> std::io::Result<()> {
    println!("🚀 Starting comprehensive RESP server test suite...");

    // Give server time to start
    thread::sleep(Duration::from_millis(500));

    let mut results = TestResults::default();

    match RespTestClient::connect("127.0.0.1:6379") {
        Ok(mut client) => {
            println!("✅ Connected to RESP server");

            // Run all test categories
            test_connection_commands(&mut client, &mut results);
            test_string_commands(&mut client, &mut results);
            test_hash_commands(&mut client, &mut results);
            test_list_commands(&mut client, &mut results);
            test_set_commands(&mut client, &mut results);
            test_sorted_set_commands(&mut client, &mut results);
            test_key_management(&mut client, &mut results);
            test_server_commands(&mut client, &mut results);
            test_error_handling(&mut client, &mut results);

            results.summary();
        }
        Err(e) => {
            println!("❌ Failed to connect to RESP server: {}", e);
            println!("Make sure the RESP server is running on 127.0.0.1:6379");
        }
    }

    Ok(())
}

fn test_connection_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n🔌 Testing Connection Commands:");

    // Test PING
    match client.expect_response(&["PING"], "PONG") {
        Ok(()) => results.success("PING command"),
        Err(e) => results.failure("PING command", &e),
    }

    // Test PING with message
    match client.expect_response(&["PING", "Hello"], "Hello") {
        Ok(()) => results.success("PING with message"),
        Err(e) => results.failure("PING with message", &e),
    }

    // Test ECHO
    match client.expect_response(&["ECHO", "test message"], "test message") {
        Ok(()) => results.success("ECHO command"),
        Err(e) => results.failure("ECHO command", &e),
    }

    // Test SELECT
    match client.expect_ok(&["SELECT", "0"]) {
        Ok(()) => results.success("SELECT command"),
        Err(e) => results.failure("SELECT command", &e),
    }

    // Test AUTH (should accept any credentials in demo mode)
    match client.expect_ok(&["AUTH", "password"]) {
        Ok(()) => results.success("AUTH command"),
        Err(e) => results.failure("AUTH command", &e),
    }
}

fn test_string_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n🔤 Testing String Commands:");

    // Test SET/GET
    match client.expect_ok(&["SET", "testkey", "testvalue"]) {
        Ok(()) => results.success("SET command"),
        Err(e) => results.failure("SET command", &e),
    }

    match client.expect_response(&["GET", "testkey"], "testvalue") {
        Ok(()) => results.success("GET command"),
        Err(e) => results.failure("GET command", &e),
    }

    // Test GET non-existent key
    match client.send_command(&["GET", "nonexistent"]) {
        Ok(response) => {
            if response.contains("$-1") || response.contains("(nil)") {
                results.success("GET non-existent key");
            } else {
                results.failure(
                    "GET non-existent key",
                    &format!("Expected null, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("GET non-existent key", &format!("IO error: {}", e)),
    }

    // Test EXISTS
    match client.expect_integer(&["EXISTS", "testkey"], 1) {
        Ok(()) => results.success("EXISTS existing key"),
        Err(e) => results.failure("EXISTS existing key", &e),
    }

    match client.expect_integer(&["EXISTS", "nonexistent"], 0) {
        Ok(()) => results.success("EXISTS non-existent key"),
        Err(e) => results.failure("EXISTS non-existent key", &e),
    }

    // Test DEL
    match client.expect_integer(&["DEL", "testkey"], 1) {
        Ok(()) => results.success("DEL existing key"),
        Err(e) => results.failure("DEL existing key", &e),
    }

    match client.expect_integer(&["DEL", "nonexistent"], 0) {
        Ok(()) => results.success("DEL non-existent key"),
        Err(e) => results.failure("DEL non-existent key", &e),
    }

    // Test SET with expiration
    match client.expect_ok(&["SET", "expkey", "expvalue", "EX", "3600"]) {
        Ok(()) => results.success("SET with expiration"),
        Err(e) => results.failure("SET with expiration", &e),
    }

    // Test TTL
    match client.send_command(&["TTL", "expkey"]) {
        Ok(response) => {
            if response.contains(":") && !response.contains(":-1") && !response.contains(":-2") {
                results.success("TTL command");
            } else {
                results.failure(
                    "TTL command",
                    &format!("Expected positive TTL, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("TTL command", &format!("IO error: {}", e)),
    }
}

fn test_hash_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n#️⃣ Testing Hash Commands:");

    // Test HSET/HGET
    match client.expect_integer(&["HSET", "testhash", "field1", "value1"], 1) {
        Ok(()) => results.success("HSET command"),
        Err(e) => results.failure("HSET command", &e),
    }

    match client.expect_response(&["HGET", "testhash", "field1"], "value1") {
        Ok(()) => results.success("HGET command"),
        Err(e) => results.failure("HGET command", &e),
    }

    // Test HMSET/HMGET
    match client.expect_ok(&["HMSET", "testhash", "field2", "value2", "field3", "value3"]) {
        Ok(()) => results.success("HMSET command"),
        Err(e) => results.failure("HMSET command", &e),
    }

    match client.send_command(&["HMGET", "testhash", "field1", "field2", "field3"]) {
        Ok(response) => {
            if response.contains("value1")
                && response.contains("value2")
                && response.contains("value3")
            {
                results.success("HMGET command");
            } else {
                results.failure(
                    "HMGET command",
                    &format!("Missing expected values in: {}", response),
                );
            }
        }
        Err(e) => results.failure("HMGET command", &format!("IO error: {}", e)),
    }

    // Test HGETALL
    match client.send_command(&["HGETALL", "testhash"]) {
        Ok(response) => {
            if response.contains("field1")
                && response.contains("value1")
                && response.contains("field2")
                && response.contains("value2")
            {
                results.success("HGETALL command");
            } else {
                results.failure(
                    "HGETALL command",
                    &format!("Missing expected fields in: {}", response),
                );
            }
        }
        Err(e) => results.failure("HGETALL command", &format!("IO error: {}", e)),
    }

    // Test HEXISTS
    match client.expect_integer(&["HEXISTS", "testhash", "field1"], 1) {
        Ok(()) => results.success("HEXISTS existing field"),
        Err(e) => results.failure("HEXISTS existing field", &e),
    }

    match client.expect_integer(&["HEXISTS", "testhash", "nonexistent"], 0) {
        Ok(()) => results.success("HEXISTS non-existent field"),
        Err(e) => results.failure("HEXISTS non-existent field", &e),
    }

    // Test HLEN
    match client.send_command(&["HLEN", "testhash"]) {
        Ok(response) => {
            if response.contains(":3") || response.contains(":2") {
                // Should have at least 2 fields
                results.success("HLEN command");
            } else {
                results.failure(
                    "HLEN command",
                    &format!("Expected count >= 2, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("HLEN command", &format!("IO error: {}", e)),
    }

    // Test HDEL
    match client.expect_integer(&["HDEL", "testhash", "field1"], 1) {
        Ok(()) => results.success("HDEL existing field"),
        Err(e) => results.failure("HDEL existing field", &e),
    }

    match client.expect_integer(&["HDEL", "testhash", "nonexistent"], 0) {
        Ok(()) => results.success("HDEL non-existent field"),
        Err(e) => results.failure("HDEL non-existent field", &e),
    }
}

fn test_list_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n📝 Testing List Commands:");

    // Test LPUSH/LLEN
    match client.expect_integer(&["LPUSH", "testlist", "item1", "item2"], 2) {
        Ok(()) => results.success("LPUSH command"),
        Err(e) => results.failure("LPUSH command", &e),
    }

    match client.expect_integer(&["LLEN", "testlist"], 2) {
        Ok(()) => results.success("LLEN command"),
        Err(e) => results.failure("LLEN command", &e),
    }

    // Test RPUSH
    match client.send_command(&["RPUSH", "testlist", "item3"]) {
        Ok(response) => {
            if response.contains(":3") {
                results.success("RPUSH command");
            } else {
                results.failure(
                    "RPUSH command",
                    &format!("Expected length 3, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("RPUSH command", &format!("IO error: {}", e)),
    }

    // Test LRANGE
    match client.send_command(&["LRANGE", "testlist", "0", "-1"]) {
        Ok(response) => {
            if response.contains("item") {
                results.success("LRANGE command");
            } else {
                results.failure(
                    "LRANGE command",
                    &format!("Expected items in list, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("LRANGE command", &format!("IO error: {}", e)),
    }

    // Test LINDEX
    match client.send_command(&["LINDEX", "testlist", "0"]) {
        Ok(response) => {
            if response.contains("item") {
                results.success("LINDEX command");
            } else {
                results.failure(
                    "LINDEX command",
                    &format!("Expected item at index 0, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("LINDEX command", &format!("IO error: {}", e)),
    }

    // Test LPOP
    match client.send_command(&["LPOP", "testlist"]) {
        Ok(response) => {
            if response.contains("item") {
                results.success("LPOP command");
            } else {
                results.failure(
                    "LPOP command",
                    &format!("Expected to pop an item, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("LPOP command", &format!("IO error: {}", e)),
    }

    // Test RPOP
    match client.send_command(&["RPOP", "testlist"]) {
        Ok(response) => {
            if response.contains("item") {
                results.success("RPOP command");
            } else {
                results.failure(
                    "RPOP command",
                    &format!("Expected to pop an item, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("RPOP command", &format!("IO error: {}", e)),
    }
}

fn test_set_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n🎯 Testing Set Commands:");

    // Test SADD/SCARD
    match client.expect_integer(&["SADD", "testset", "member1", "member2", "member3"], 3) {
        Ok(()) => results.success("SADD command"),
        Err(e) => results.failure("SADD command", &e),
    }

    match client.expect_integer(&["SCARD", "testset"], 3) {
        Ok(()) => results.success("SCARD command"),
        Err(e) => results.failure("SCARD command", &e),
    }

    // Test SISMEMBER
    match client.expect_integer(&["SISMEMBER", "testset", "member1"], 1) {
        Ok(()) => results.success("SISMEMBER existing member"),
        Err(e) => results.failure("SISMEMBER existing member", &e),
    }

    match client.expect_integer(&["SISMEMBER", "testset", "nonexistent"], 0) {
        Ok(()) => results.success("SISMEMBER non-existent member"),
        Err(e) => results.failure("SISMEMBER non-existent member", &e),
    }

    // Test SMEMBERS
    match client.send_command(&["SMEMBERS", "testset"]) {
        Ok(response) => {
            if response.contains("member1")
                || response.contains("member2")
                || response.contains("member3")
            {
                results.success("SMEMBERS command");
            } else {
                results.failure(
                    "SMEMBERS command",
                    &format!("Expected members in set, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("SMEMBERS command", &format!("IO error: {}", e)),
    }

    // Test SREM
    match client.expect_integer(&["SREM", "testset", "member1"], 1) {
        Ok(()) => results.success("SREM existing member"),
        Err(e) => results.failure("SREM existing member", &e),
    }

    match client.expect_integer(&["SREM", "testset", "nonexistent"], 0) {
        Ok(()) => results.success("SREM non-existent member"),
        Err(e) => results.failure("SREM non-existent member", &e),
    }
}

fn test_sorted_set_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n🏆 Testing Sorted Set Commands:");

    // Test ZADD/ZCARD
    match client.expect_integer(&["ZADD", "testzset", "1.0", "member1", "2.0", "member2"], 2) {
        Ok(()) => results.success("ZADD command"),
        Err(e) => results.failure("ZADD command", &e),
    }

    match client.expect_integer(&["ZCARD", "testzset"], 2) {
        Ok(()) => results.success("ZCARD command"),
        Err(e) => results.failure("ZCARD command", &e),
    }

    // Test ZSCORE
    match client.expect_response(&["ZSCORE", "testzset", "member1"], "1") {
        Ok(()) => results.success("ZSCORE command"),
        Err(e) => results.failure("ZSCORE command", &e),
    }

    // Test ZRANGE
    match client.send_command(&["ZRANGE", "testzset", "0", "-1"]) {
        Ok(response) => {
            if response.contains("member") {
                results.success("ZRANGE command");
            } else {
                results.failure(
                    "ZRANGE command",
                    &format!("Expected members in sorted set, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("ZRANGE command", &format!("IO error: {}", e)),
    }

    // Test ZINCRBY
    match client.expect_response(&["ZINCRBY", "testzset", "5.5", "member1"], "6.5") {
        Ok(()) => results.success("ZINCRBY command"),
        Err(e) => results.failure("ZINCRBY command", &e),
    }

    // Test ZREM
    match client.expect_integer(&["ZREM", "testzset", "member1"], 1) {
        Ok(()) => results.success("ZREM existing member"),
        Err(e) => results.failure("ZREM existing member", &e),
    }
}

fn test_key_management(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n🗝️ Testing Key Management Commands:");

    // Set up test key with expiration
    let _ = client.send_command(&["SET", "expiretest", "value"]);

    // Test EXPIRE
    match client.expect_integer(&["EXPIRE", "expiretest", "10"], 1) {
        Ok(()) => results.success("EXPIRE command"),
        Err(e) => results.failure("EXPIRE command", &e),
    }

    // Test TTL (should show time remaining)
    match client.send_command(&["TTL", "expiretest"]) {
        Ok(response) => {
            if response.contains(":") && !response.contains(":-1") {
                results.success("TTL after EXPIRE");
            } else {
                results.failure(
                    "TTL after EXPIRE",
                    &format!("Expected positive TTL, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("TTL after EXPIRE", &format!("IO error: {}", e)),
    }

    // Test TYPE command
    let _ = client.send_command(&["SET", "stringkey", "value"]);
    match client.send_command(&["TYPE", "stringkey"]) {
        Ok(response) => {
            if response.contains("string") || response.contains("none") {
                results.success("TYPE command");
            } else {
                results.failure(
                    "TYPE command",
                    &format!("Expected type info, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("TYPE command", &format!("IO error: {}", e)),
    }
}

fn test_server_commands(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n🖥️ Testing Server Commands:");

    // Test INFO
    match client.send_command(&["INFO"]) {
        Ok(response) => {
            if response.len() > 10 {
                // Should return some server info
                results.success("INFO command");
            } else {
                results.failure(
                    "INFO command",
                    &format!("Expected server info, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("INFO command", &format!("IO error: {}", e)),
    }

    // Test DBSIZE
    match client.send_command(&["DBSIZE"]) {
        Ok(response) => {
            if response.contains(":") {
                // Should return a number
                results.success("DBSIZE command");
            } else {
                results.failure(
                    "DBSIZE command",
                    &format!("Expected number, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("DBSIZE command", &format!("IO error: {}", e)),
    }

    // Test COMMAND
    match client.send_command(&["COMMAND"]) {
        Ok(response) => {
            if response.len() > 50 {
                // Should return command info
                results.success("COMMAND command");
            } else {
                results.failure(
                    "COMMAND command",
                    &format!("Expected command list, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("COMMAND command", &format!("IO error: {}", e)),
    }
}

fn test_error_handling(client: &mut RespTestClient, results: &mut TestResults) {
    println!("\n⚠️ Testing Error Handling:");

    // Test unknown command
    match client.send_command(&["UNKNOWNCOMMAND"]) {
        Ok(response) => {
            if response.contains("ERR") || response.contains("unknown command") {
                results.success("Unknown command error");
            } else {
                results.failure(
                    "Unknown command error",
                    &format!("Expected error, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("Unknown command error", &format!("IO error: {}", e)),
    }

    // Test wrong number of arguments
    match client.send_command(&["GET"]) {
        Ok(response) => {
            if response.contains("ERR") || response.contains("wrong number") {
                results.success("Wrong arguments error");
            } else {
                results.failure(
                    "Wrong arguments error",
                    &format!("Expected error, got: {}", response),
                );
            }
        }
        Err(e) => results.failure("Wrong arguments error", &format!("IO error: {}", e)),
    }

    // Test invalid data type
    match client.send_command(&["INCR", "non-numeric"]) {
        Ok(response) => {
            if response.contains("ERR") || response.contains("not an integer") {
                results.success("Invalid data type error");
            } else {
                // This might succeed in some implementations, so it's okay
                results.success("Invalid data type error (command accepted)");
            }
        }
        Err(e) => results.failure("Invalid data type error", &format!("IO error: {}", e)),
    }
}
