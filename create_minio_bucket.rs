use opendal::{Operator, services::S3};

#[tokio::main]
async fn main() -> Result\u003c(), Box\u003cdyn std::error::Error\u003e\u003e {
    // Configure S3 service for MinIO
    let mut builder = S3::default();
    builder
        .endpoint("http://localhost:9000")
        .access_key_id("minioadmin")
        .secret_access_key("minioadmin")
        .bucket("orbit-cold-storage")
        .region("us-east-1");

    let op = Operator::new(builder)?.finish();
    
    // Try to create a test file to ensure bucket exists
    op.write("test.txt", "test").await?;
    println!("Successfully created/accessed bucket orbit-cold-storage");
    
    // Clean up test file
    op.delete("test.txt").await?;
    
    Ok(())
}
