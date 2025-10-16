fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:warning=Skipping protobuf compilation - API compatibility issue with tonic-build 0.14");
    Ok(())
}
