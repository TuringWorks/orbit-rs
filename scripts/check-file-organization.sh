#!/bin/bash

# File Organization Checker
# This script checks that files are placed in the correct directories according to project rules

set -e

# Colors for output
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

# Counters
violations=0
warnings=0

echo "🔍 Checking file organization rules..."

# Function to report violations
report_violation() {
    local file="$1"
    local rule="$2"
    echo -e "${RED}❌ VIOLATION:${NC} $file"
    echo -e "   Rule: $rule"
    violations=$((violations + 1))
}

# Function to report warnings
report_warning() {
    local file="$1" 
    local suggestion="$2"
    echo -e "${YELLOW}⚠️  WARNING:${NC} $file"
    echo -e "   Suggestion: $suggestion"
    warnings=$((warnings + 1))
}

# Check documentation files
echo "📚 Checking documentation file placement..."
find . -name "*.md" \
    -not -path "./docs/*" \
    -not -path "./.git/*" \
    -not -path "./target/*" \
    -not -path "./orbit-benchmarks/*" \
    -not -path "./orbit-desktop/node_modules/*" \
    -not -path "./.github/ISSUE_TEMPLATE/*" \
    -not -path "*/node_modules/*" | while read -r file; do
    # Allow README.md files in package roots and project root
    basename=$(basename "$file")
    if [[ "$basename" == "README.md" ]]; then
        # Check if it's in a valid location (project root or package root)
        dir=$(dirname "$file")
        if [[ "$dir" =~ ^\./[a-z-]+$ ]] || [[ "$dir" == "." ]]; then
            continue  # Valid package README.md
        fi
    fi
    
    case "$file" in
        ./DEVELOPMENT.md|./CONTRIBUTING.md|./CHANGELOG.md|./ARCHITECTURE.md)
            report_violation "$file" "Documentation files must be in docs/ directory"
            ;;
        ./*.md)
            if [[ "$basename" != "README.md" ]]; then
                report_violation "$file" "Documentation files must be in docs/ directory"
            fi
            ;;
        ./*/doc*.md|./*/DESIGN.md|./*/ARCHITECTURE.md)
            report_warning "$file" "Consider moving to docs/ directory for better organization"
            ;;
    esac
done

# Check benchmark files
echo "🏃‍♂️ Checking benchmark file placement..."

# Check for benchmark files outside orbit-benchmarks
find . -name "*benchmark*" -type f \
    -not -path "./orbit-benchmarks/*" \
    -not -path "./.git/*" \
    -not -path "./target/*" \
    -not -path "*/node_modules/*" | while read -r file; do
    case "$file" in
        # Allow benchmark references in documentation and scripts
        *.md|*.sh|*.py|*.yml|*.yaml)
            continue
            ;;
        # Rust benchmark files should be in orbit-benchmarks
        *.rs)
            report_violation "$file" "Benchmark code must be in orbit-benchmarks/ package"
            ;;
        # Other potential benchmark files
        *)
            report_warning "$file" "Benchmark-related file should probably be in orbit-benchmarks/"
            ;;
    esac
done

# Check for performance/bench directories outside orbit-benchmarks
find . -type d \( -name "bench" -o -name "benches" -o -name "benchmark" -o -name "benchmarks" -o -name "performance" \) \
    -not -path "./orbit-benchmarks/*" \
    -not -path "./.git/*" \
    -not -path "./target/*" \
    -not -path "*/node_modules/*" | while read -r dir; do
    report_warning "$dir" "Benchmark directories should be in orbit-benchmarks/ package"
done

# Check for misplaced script files
echo "📜 Checking script file placement..."
find . -name "*.sh" \
    -not -path "./scripts/*" \
    -not -path "./orbit-benchmarks/scripts/*" \
    -not -path "./.github/*" \
    -not -path "./.git/*" \
    -not -path "./target/*" \
    -not -path "*/node_modules/*" | while read -r file; do
    case "$file" in
        ./tests/*)
            continue  # Test scripts are OK in tests/
            ;;
        ./*/*)
            report_warning "$file" "Consider moving to scripts/ or appropriate package scripts/ directory"
            ;;
    esac
done

# Summary
echo
echo "📊 File Organization Check Summary:"
if [ $violations -eq 0 ] && [ $warnings -eq 0 ]; then
    echo -e "${GREEN}✅ All files are properly organized!${NC}"
    exit 0
elif [ $violations -eq 0 ]; then
    echo -e "${YELLOW}⚠️  Found $warnings warnings (suggestions for improvement)${NC}"
    echo "These are not blocking, but consider addressing them for better organization."
    exit 0
else
    echo -e "${RED}❌ Found $violations violations and $warnings warnings${NC}"
    echo
    echo "Please fix the violations by moving files to appropriate locations:"
    echo "  📚 Documentation files → docs/"
    echo "  🏃‍♂️ Benchmark files → orbit-benchmarks/"
    echo "  📜 Scripts → scripts/ or package-specific scripts/"
    echo
    echo "Use 'git mv' to preserve file history when moving files."
    echo "See docs/PROJECT_ORGANIZATION_RULES.md for detailed guidelines."
    exit 1
fi