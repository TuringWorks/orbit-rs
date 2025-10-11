#!/bin/bash

# Add Jekyll Frontmatter to Documentation Files
# This script adds proper Jekyll frontmatter to .md files to fix title duplication issues

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo "🔧 Adding Jekyll frontmatter to documentation files..."

# Function to extract title from first H1 heading
extract_title() {
    local file="$1"
    head -20 "$file" | grep "^# " | head -1 | sed 's/^# //' || echo "$(basename "$file" .md)"
}

# Function to generate category based on file path
generate_category() {
    local file="$1"
    case "$file" in
        */architecture/*) echo "architecture" ;;
        */development/*) echo "development" ;;
        */deployment/*) echo "deployment" ;;
        */protocols/*) echo "protocols" ;;
        */rfcs/*) echo "rfcs" ;;
        */features/*) echo "features" ;;
        */github/*) echo "github" ;;
        */examples/*) echo "examples" ;;
        */planning/*) echo "planning" ;;
        */wip/*) echo "wip" ;;
        */backlog/*) echo "backlog" ;;
        */issues/*) echo "issues" ;;
        */verification/*) echo "verification" ;;
        */guidelines/*) echo "guidelines" ;;
        *) echo "documentation" ;;
    esac
}

# Function to add frontmatter to a file
add_frontmatter() {
    local file="$1"
    local title="$2"
    local category="$3"
    
    # Create temporary file
    local temp_file=$(mktemp)
    
    # Add frontmatter
    cat > "$temp_file" << EOF
---
layout: default
title: $title
category: $category
---

EOF
    
    # Append original content
    cat "$file" >> "$temp_file"
    
    # Replace original file
    mv "$temp_file" "$file"
    
    echo -e "${GREEN}✅ Added frontmatter to: $file${NC}"
}

# Counter for processed files
processed=0

# Process all .md files without frontmatter
find docs -name "*.md" | while read file; do
    # Skip files that already have frontmatter
    if head -1 "$file" | grep -q "^---"; then
        continue
    fi
    
    # Skip README files in subdirectories 
    if basename "$file" | grep -q "^README.md$"; then
        continue
    fi
    
    # Extract title from first H1 heading
    title=$(extract_title "$file")
    
    # Generate category based on file path
    category=$(generate_category "$file")
    
    # Add frontmatter
    add_frontmatter "$file" "$title" "$category"
    
    processed=$((processed + 1))
done

echo
echo -e "${GREEN}🎉 Completed adding Jekyll frontmatter!${NC}"
echo "This should resolve title duplication issues in the documentation site."
echo
echo "Next steps:"
echo "1. Review the changes with 'git diff'"
echo "2. Test the documentation site locally if possible"
echo "3. Commit and push the changes"