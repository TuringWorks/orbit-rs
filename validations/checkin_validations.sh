cd ..
cargo build
cargo test --workspace --verbose
cargo clippy --all-targets -- -D warnings
cargo install cargo-deny --locked
cargo deny check
cargo tarpaulin --verbose --workspace --timeout 120 --out Xml --output-dir ./coverage
cargo llvm-cov --all-features --workspace

