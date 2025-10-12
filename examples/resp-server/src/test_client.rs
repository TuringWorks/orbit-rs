//! Simple RESP client test to verify server functionality

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    println!("🧪 Testing RESP server connection...");

    // Give the server a moment to start
    std::thread::sleep(Duration::from_millis(100));

    // Connect to the RESP server
    match TcpStream::connect("127.0.0.1:6379") {
        Ok(mut stream) => {
            println!("✅ Connected to RESP server");

            // Test PING command
            test_ping(&mut stream)?;

            // Test ECHO command
            test_echo(&mut stream)?;

            // Test SET/GET commands
            test_set_get(&mut stream)?;

            println!("✅ All tests passed!");
        }
        Err(e) => {
            println!("❌ Failed to connect to RESP server: {}", e);
            println!("Make sure the RESP server is running on 127.0.0.1:6379");
        }
    }

    Ok(())
}

fn test_ping(stream: &mut TcpStream) -> std::io::Result<()> {
    println!("📤 Sending PING command...");

    // Send PING command in RESP format
    stream.write_all(b"*1\r\n$4\r\nPING\r\n")?;
    stream.flush()?;

    // Read response
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer)?;
    let response = String::from_utf8_lossy(&buffer[..n]);

    println!("📥 Response: {:?}", response.trim());

    if response.contains("PONG") || response.starts_with("+") {
        println!("✅ PING test passed");
    } else {
        println!("❌ PING test failed");
    }

    Ok(())
}

fn test_echo(stream: &mut TcpStream) -> std::io::Result<()> {
    println!("📤 Sending ECHO command...");

    // Send ECHO "Hello" command in RESP format
    stream.write_all(b"*2\r\n$4\r\nECHO\r\n$5\r\nHello\r\n")?;
    stream.flush()?;

    // Read response
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer)?;
    let response = String::from_utf8_lossy(&buffer[..n]);

    println!("📥 Response: {:?}", response.trim());

    if response.contains("Hello") {
        println!("✅ ECHO test passed");
    } else {
        println!("❌ ECHO test failed");
    }

    Ok(())
}

fn test_set_get(stream: &mut TcpStream) -> std::io::Result<()> {
    println!("📤 Sending SET command...");

    // Send SET mykey "Hello World" command in RESP format
    stream.write_all(b"*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$11\r\nHello World\r\n")?;
    stream.flush()?;

    // Read response
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer)?;
    let set_response = String::from_utf8_lossy(&buffer[..n]);

    println!("📥 SET Response: {:?}", set_response.trim());

    println!("📤 Sending GET command...");

    // Send GET mykey command in RESP format
    stream.write_all(b"*2\r\n$3\r\nGET\r\n$5\r\nmykey\r\n")?;
    stream.flush()?;

    // Read response
    let mut buffer = [0; 1024];
    let n = stream.read(&mut buffer)?;
    let get_response = String::from_utf8_lossy(&buffer[..n]);

    println!("📥 GET Response: {:?}", get_response.trim());

    if get_response.contains("Hello World") {
        println!("✅ SET/GET test passed");
    } else {
        println!("❌ SET/GET test failed");
    }

    Ok(())
}
