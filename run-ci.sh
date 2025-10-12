#!/bin/bash

# Simple launcher for manual CI/CD script
# This provides convenient shortcuts for common CI/CD operations

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Default to running the full CI/CD pipeline
exec "${SCRIPT_DIR}/scripts/manual-ci-cd.sh" "$@"