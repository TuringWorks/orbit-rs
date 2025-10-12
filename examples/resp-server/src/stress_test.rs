//! Stress and performance test for RESP server
//! Tests concurrent connections and command throughput

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Test statistics
#[derive(Debug, Default)]
pub struct StressTestStats {
    pub total_commands: usize,
    pub successful_commands: usize,
    pub failed_commands: usize,
    pub total_time: Duration,
    pub commands_per_second: f64,
}

fn main() -> std::io::Result<()> {
    println!("🚀 RESP Server Stress Test");
    println!("==========================");

    // Give server time to start
    thread::sleep(Duration::from_millis(500));

    // Test configurations
    let num_connections = 10;
    let commands_per_connection = 100;

    println!("📊 Test Configuration:");
    println!("   • Concurrent connections: {}", num_connections);
    println!("   • Commands per connection: {}", commands_per_connection);
    println!(
        "   • Total commands: {}",
        num_connections * commands_per_connection
    );
    println!();

    let stats = Arc::new(Mutex::new(StressTestStats::default()));
    let mut handles = Vec::new();

    let start_time = Instant::now();

    // Spawn concurrent test threads
    for i in 0..num_connections {
        let stats_clone = Arc::clone(&stats);

        let handle =
            thread::spawn(move || run_connection_test(i, commands_per_connection, stats_clone));

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        let _ = handle.join();
    }

    let total_time = start_time.elapsed();

    // Update final stats
    {
        let mut stats = stats.lock().unwrap();
        stats.total_time = total_time;
        stats.commands_per_second = stats.successful_commands as f64 / total_time.as_secs_f64();
    }

    // Print results
    let final_stats = stats.lock().unwrap();
    print_results(&final_stats);

    Ok(())
}

fn run_connection_test(
    connection_id: usize,
    num_commands: usize,
    stats: Arc<Mutex<StressTestStats>>,
) {
    println!("🔌 Starting connection {} test...", connection_id);

    let mut successful = 0;
    let mut failed = 0;

    match TcpStream::connect("127.0.0.1:6379") {
        Ok(mut stream) => {
            stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
            stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

            for i in 0..num_commands {
                if test_command(&mut stream, connection_id, i) {
                    successful += 1;
                } else {
                    failed += 1;
                }
            }

            println!(
                "✅ Connection {} completed: {} successful, {} failed",
                connection_id, successful, failed
            );
        }
        Err(e) => {
            println!("❌ Connection {} failed to connect: {}", connection_id, e);
            failed = num_commands;
        }
    }

    // Update global stats
    {
        let mut stats = stats.lock().unwrap();
        stats.total_commands += num_commands;
        stats.successful_commands += successful;
        stats.failed_commands += failed;
    }
}

fn test_command(stream: &mut TcpStream, connection_id: usize, command_id: usize) -> bool {
    // Alternate between different commands for variety
    match command_id % 6 {
        0 => send_command(stream, &["PING"]).is_ok(),
        1 => {
            let key = format!("key{}_{}", connection_id, command_id);
            send_command(stream, &["SET", &key, "value"]).is_ok()
        }
        2 => {
            let key = format!("key{}_{}", connection_id, command_id.saturating_sub(1));
            send_command(stream, &["GET", &key]).is_ok()
        }
        3 => send_command(stream, &["ECHO", "stress test"]).is_ok(),
        4 => send_command(stream, &["INFO"]).is_ok(),
        5 => {
            let key = format!("key{}_{}", connection_id, command_id);
            send_command(stream, &["EXISTS", &key]).is_ok()
        }
        _ => send_command(stream, &["PING"]).is_ok(),
    }
}

fn send_command(stream: &mut TcpStream, command: &[&str]) -> std::io::Result<String> {
    // Build RESP command
    let mut resp_cmd = format!("*{}\r\n", command.len());
    for arg in command {
        resp_cmd.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
    }

    // Send command
    stream.write_all(resp_cmd.as_bytes())?;
    stream.flush()?;

    // Read response
    let mut buffer = vec![0; 1024];
    let n = stream.read(&mut buffer)?;
    Ok(String::from_utf8_lossy(&buffer[..n]).to_string())
}

fn print_results(stats: &StressTestStats) {
    println!();
    println!("📈 Stress Test Results:");
    println!("========================");
    println!("📊 Commands:");
    println!("   • Total:      {}", stats.total_commands);
    println!(
        "   • Successful: {} ({:.1}%)",
        stats.successful_commands,
        (stats.successful_commands as f64 / stats.total_commands as f64) * 100.0
    );
    println!(
        "   • Failed:     {} ({:.1}%)",
        stats.failed_commands,
        (stats.failed_commands as f64 / stats.total_commands as f64) * 100.0
    );
    println!();

    println!("⏱️ Performance:");
    println!(
        "   • Total time:     {:.2}s",
        stats.total_time.as_secs_f64()
    );
    println!("   • Commands/sec:   {:.2}", stats.commands_per_second);
    println!(
        "   • Avg latency:    {:.2}ms",
        (stats.total_time.as_millis() as f64) / (stats.total_commands as f64)
    );
    println!();

    // Performance assessment
    if stats.commands_per_second > 1000.0 {
        println!("🚀 Performance: Excellent (>1000 cmd/s)");
    } else if stats.commands_per_second > 500.0 {
        println!("✅ Performance: Good (>500 cmd/s)");
    } else if stats.commands_per_second > 100.0 {
        println!("⚠️ Performance: Fair (>100 cmd/s)");
    } else {
        println!("❌ Performance: Poor (<100 cmd/s)");
    }

    // Success rate assessment
    let success_rate = (stats.successful_commands as f64 / stats.total_commands as f64) * 100.0;
    if success_rate > 95.0 {
        println!("✅ Reliability: Excellent (>95% success)");
    } else if success_rate > 90.0 {
        println!("⚠️ Reliability: Good (>90% success)");
    } else {
        println!("❌ Reliability: Poor (<90% success)");
    }
}
