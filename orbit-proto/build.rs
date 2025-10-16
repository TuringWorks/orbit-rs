use std::io::Result;

fn main() -> Result<()> {
    println!("cargo:warning=Skipping protobuf compilation - API compatibility issue with tonic-build 0.14");
    Ok(())
}
