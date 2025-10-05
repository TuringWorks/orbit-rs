# Multi-stage build for Orbit-RS
FROM rust:1.75-bookworm as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 orbit

# Set working directory
WORKDIR /usr/src/orbit-rs

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY orbit-util/Cargo.toml ./orbit-util/
COPY orbit-shared/Cargo.toml ./orbit-shared/
COPY orbit-proto/Cargo.toml ./orbit-proto/
COPY orbit-client/Cargo.toml ./orbit-client/
COPY orbit-server/Cargo.toml ./orbit-server/
COPY orbit-server-etcd/Cargo.toml ./orbit-server-etcd/
COPY orbit-server-prometheus/Cargo.toml ./orbit-server-prometheus/
COPY orbit-client-spring/Cargo.toml ./orbit-client-spring/
COPY orbit-protocols/Cargo.toml ./orbit-protocols/
COPY orbit-operator/Cargo.toml ./orbit-operator/
COPY orbit-application/Cargo.toml ./orbit-application/
COPY orbit-benchmarks/Cargo.toml ./orbit-benchmarks/
COPY tests/Cargo.toml ./tests/
COPY examples/hello-world/Cargo.toml ./examples/hello-world/
COPY examples/distributed-counter/Cargo.toml ./examples/distributed-counter/
COPY examples/distributed-transactions/Cargo.toml ./examples/distributed-transactions/
COPY examples/resp-server/Cargo.toml ./examples/resp-server/
COPY examples/vector-store/Cargo.toml ./examples/vector-store/
COPY examples/pgvector-store/Cargo.toml ./examples/pgvector-store/
COPY examples/saga-example/Cargo.toml ./examples/saga-example/

# Create dummy source files to cache dependencies
RUN mkdir -p orbit-util/src orbit-shared/src orbit-proto/src orbit-client/src \
    orbit-server/src orbit-server-etcd/src orbit-server-prometheus/src \
    orbit-client-spring/src orbit-protocols/src orbit-operator/src \
    orbit-application/src orbit-benchmarks/src tests/src \
    orbit-proto/proto examples/hello-world/src examples/distributed-counter/src \
    examples/distributed-transactions/src examples/resp-server/src \
    examples/vector-store/src examples/pgvector-store/src examples/saga-example/src

# Create minimal main.rs files
RUN echo "fn main() {}" > orbit-server/src/main.rs && \
    echo "fn main() {}" > examples/hello-world/src/main.rs && \
    echo "fn main() {}" > examples/distributed-counter/src/main.rs && \
    echo "fn main() {}" > examples/distributed-transactions/src/main.rs && \
    echo "fn main() {}" > examples/resp-server/src/main.rs && \
    echo "fn main() {}" > examples/vector-store/src/main.rs && \
    echo "fn main() {}" > examples/pgvector-store/src/main.rs && \
    echo "fn main() {}" > examples/saga-example/src/main.rs

# Create minimal lib.rs files
RUN echo "" > orbit-util/src/lib.rs && \
    echo "" > orbit-shared/src/lib.rs && \
    echo "" > orbit-proto/src/lib.rs && \
    echo "" > orbit-client/src/lib.rs && \
    echo "" > orbit-server/src/lib.rs && \
    echo "" > orbit-server-etcd/src/lib.rs && \
    echo "" > orbit-server-prometheus/src/lib.rs && \
    echo "" > orbit-client-spring/src/lib.rs && \
    echo "" > orbit-protocols/src/lib.rs && \
    echo "" > orbit-operator/src/lib.rs && \
    echo "" > orbit-application/src/lib.rs && \
    echo "" > orbit-benchmarks/src/lib.rs && \
    echo "" > tests/src/lib.rs

# Copy proto files for build
COPY orbit-proto/proto/ ./orbit-proto/proto/
COPY orbit-proto/build.rs ./orbit-proto/

# Build dependencies only
RUN cargo build --release --bin orbit-server

# Copy actual source code
COPY . .

# Build the actual application
RUN touch orbit-server/src/main.rs && \
    cargo build --release --bin orbit-server

# Runtime stage
FROM debian:bookworm-slim as runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 orbit

# Create directories for application
RUN mkdir -p /app/data /app/logs /app/config && \
    chown -R orbit:orbit /app

# Copy binary from builder stage
COPY --from=builder /usr/src/orbit-rs/target/release/orbit-server /app/orbit-server

# Copy configuration files (with fallback handling)
COPY deploy/config/ /app/config/
# Also copy example configs for reference
COPY config/ /app/config/reference/

# Set ownership
RUN chown -R orbit:orbit /app

# Switch to non-root user
USER orbit

# Set working directory
WORKDIR /app

# Expose ports
EXPOSE 50051 8080 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Set entrypoint
ENTRYPOINT ["/app/orbit-server"]

# Default command
CMD ["--config", "/app/config/orbit-server.toml"]