#!/bin/bash

# Documentation Check
# Replicates: cargo doc --no-deps --features="resp,postgres-wire,cypher,rest"

set -e

echo "🔍 Building documentation..."

# Set Cargo color output
export CARGO_TERM_COLOR=always

# Build documentation with default features
# Note: The features resp, postgres-wire, cypher, rest don't exist as top-level features
# The server uses protocol-redis, protocol-postgres, query-cypher, protocol-rest instead
echo "Running: cargo doc --no-deps"

if cargo doc --no-deps; then
    echo "✅ Documentation build successful"
    
    # Check if documentation was generated
    if [ -d "target/doc" ]; then
        echo "📚 Documentation generated in target/doc/"
        
        # Find the main crate documentation
        if [ -f "target/doc/orbit_server/index.html" ]; then
            echo "   Main documentation: target/doc/orbit_server/index.html"
        fi
        
        echo "💡 Open target/doc/index.html in your browser to view the documentation"
    fi
else
    echo "❌ Documentation build failed"
    echo "💡 Check for documentation errors in the code comments"
    exit 1
fi

echo "✅ Documentation check completed successfully"