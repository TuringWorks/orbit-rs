fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/transaction.proto")?;
    tonic_build::compile_protos("proto/consensus.proto")?;
    Ok(())
}
