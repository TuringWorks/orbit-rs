#!/usr/bin/env python3
"""
Code Complexity Analysis Tool for Orbit Rust Codebase
Analyzes code complexity, duplications, and maintainability metrics
"""

import os
import re
import sys
from pathlib import Path
from collections import defaultdict, Counter
from typing import Dict, List, Tuple, NamedTuple
import hashlib


class ComplexityMetrics(NamedTuple):
    file_path: str
    lines_of_code: int
    cyclomatic_complexity: int
    nesting_depth: int
    function_count: int
    struct_count: int
    impl_count: int
    comment_ratio: float
    todo_count: int
    fixme_count: int
    clone_issues: int


class CodeAnalyzer:
    def __init__(self, root_dir: str):
        self.root_dir = Path(root_dir)
        self.results: List[ComplexityMetrics] = []
        self.code_hashes: Dict[str, List[str]] = defaultdict(list)
        
    def find_rust_files(self) -> List[Path]:
        """Find all Rust source files excluding target directories and tests"""
        rust_files = []
        for path in self.root_dir.rglob("*.rs"):
            # Skip target directories, generated files, and test-only directories
            if any(part in path.parts for part in ['target', 'tests', 'examples', 'demo']):
                continue
            if 'generated' in str(path) or 'build.rs' in str(path):
                continue
            rust_files.append(path)
        return rust_files
    
    def calculate_cyclomatic_complexity(self, content: str) -> int:
        """Calculate cyclomatic complexity based on decision points"""
        complexity = 1  # Base complexity
        
        # Control flow keywords that increase complexity
        control_keywords = [
            r'\bif\b', r'\belse\s+if\b', r'\bmatch\b', r'\bwhile\b', 
            r'\bfor\b', r'\bloop\b', r'\bcatch\b', r'\b\?\b',
            r'=>', r'\band\b', r'\bor\b', r'\b&&\b', r'\b\|\|\b'
        ]
        
        for keyword in control_keywords:
            matches = re.findall(keyword, content)
            complexity += len(matches)
        
        return complexity
    
    def calculate_nesting_depth(self, content: str) -> int:
        """Calculate maximum nesting depth"""
        max_depth = 0
        current_depth = 0
        
        lines = content.split('\n')
        for line in lines:
            stripped = line.strip()
            if not stripped or stripped.startswith('//'):
                continue
                
            # Count opening braces
            open_braces = line.count('{')
            close_braces = line.count('}')
            
            current_depth += open_braces - close_braces
            max_depth = max(max_depth, current_depth)
        
        return max_depth
    
    def count_functions_structs(self, content: str) -> Tuple[int, int, int]:
        """Count functions, structs, and impl blocks"""
        function_count = len(re.findall(r'\bfn\s+\w+', content))
        struct_count = len(re.findall(r'\bstruct\s+\w+', content))
        impl_count = len(re.findall(r'\bimpl\b', content))
        
        return function_count, struct_count, impl_count
    
    def calculate_comment_ratio(self, content: str) -> float:
        """Calculate ratio of comment lines to total lines"""
        lines = content.split('\n')
        comment_lines = 0
        code_lines = 0
        
        for line in lines:
            stripped = line.strip()
            if not stripped:
                continue
            elif stripped.startswith('//') or stripped.startswith('/*') or stripped.startswith('*'):
                comment_lines += 1
            else:
                code_lines += 1
        
        total_lines = comment_lines + code_lines
        return comment_lines / total_lines if total_lines > 0 else 0.0
    
    def count_technical_debt(self, content: str) -> Tuple[int, int]:
        """Count TODO and FIXME comments"""
        todo_count = len(re.findall(r'(?i)todo', content))
        fixme_count = len(re.findall(r'(?i)fixme', content))
        return todo_count, fixme_count
    
    def detect_code_clones(self, file_path: str, content: str) -> int:
        """Simple clone detection based on line hashes"""
        lines = content.split('\n')
        clone_issues = 0
        
        for i, line in enumerate(lines):
            stripped = line.strip()
            if len(stripped) > 30:  # Only consider substantial lines
                line_hash = hashlib.md5(stripped.encode()).hexdigest()
                if line_hash in self.code_hashes:
                    # Found potential clone
                    existing_files = self.code_hashes[line_hash]
                    if not any(str(file_path) in f for f in existing_files):
                        clone_issues += 1
                self.code_hashes[line_hash].append(f"{file_path}:{i+1}")
        
        return clone_issues
    
    def analyze_file(self, file_path: Path) -> ComplexityMetrics:
        """Analyze a single Rust file"""
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                content = f.read()
        except Exception as e:
            print(f"Error reading {file_path}: {e}")
            return ComplexityMetrics(
                str(file_path), 0, 0, 0, 0, 0, 0, 0.0, 0, 0, 0
            )
        
        lines_of_code = len([line for line in content.split('\n') if line.strip()])
        cyclomatic_complexity = self.calculate_cyclomatic_complexity(content)
        nesting_depth = self.calculate_nesting_depth(content)
        function_count, struct_count, impl_count = self.count_functions_structs(content)
        comment_ratio = self.calculate_comment_ratio(content)
        todo_count, fixme_count = self.count_technical_debt(content)
        clone_issues = self.detect_code_clones(file_path, content)
        
        return ComplexityMetrics(
            str(file_path.relative_to(self.root_dir)),
            lines_of_code,
            cyclomatic_complexity,
            nesting_depth,
            function_count,
            struct_count,
            impl_count,
            comment_ratio,
            todo_count,
            fixme_count,
            clone_issues
        )
    
    def analyze(self) -> None:
        """Analyze all Rust files in the codebase"""
        rust_files = self.find_rust_files()
        print(f"Analyzing {len(rust_files)} Rust files...")
        
        for file_path in rust_files:
            metrics = self.analyze_file(file_path)
            self.results.append(metrics)
    
    def generate_report(self) -> str:
        """Generate a comprehensive complexity report"""
        if not self.results:
            return "No files analyzed."
        
        # Sort by complexity and size metrics
        high_complexity = sorted(self.results, key=lambda x: x.cyclomatic_complexity, reverse=True)[:10]
        large_files = sorted(self.results, key=lambda x: x.lines_of_code, reverse=True)[:10]
        deep_nesting = sorted(self.results, key=lambda x: x.nesting_depth, reverse=True)[:10]
        high_clone_issues = sorted(self.results, key=lambda x: x.clone_issues, reverse=True)[:10]
        low_comment_ratio = sorted(self.results, key=lambda x: x.comment_ratio)[:10]
        high_technical_debt = sorted(self.results, key=lambda x: x.todo_count + x.fixme_count, reverse=True)[:10]
        
        # Calculate summary statistics
        total_files = len(self.results)
        total_loc = sum(r.lines_of_code for r in self.results)
        avg_complexity = sum(r.cyclomatic_complexity for r in self.results) / total_files
        avg_nesting = sum(r.nesting_depth for r in self.results) / total_files
        avg_comment_ratio = sum(r.comment_ratio for r in self.results) / total_files
        total_todos = sum(r.todo_count for r in self.results)
        total_fixmes = sum(r.fixme_count for r in self.results)
        total_functions = sum(r.function_count for r in self.results)
        total_structs = sum(r.struct_count for r in self.results)
        total_impls = sum(r.impl_count for r in self.results)
        
        report = f"""
# Orbit Rust Codebase Complexity Analysis Report

## Summary Statistics
- **Total Files Analyzed**: {total_files}
- **Total Lines of Code**: {total_loc:,}
- **Average Cyclomatic Complexity**: {avg_complexity:.2f}
- **Average Nesting Depth**: {avg_nesting:.2f}
- **Average Comment Ratio**: {avg_comment_ratio:.2%}
- **Total Functions**: {total_functions:,}
- **Total Structs**: {total_structs:,}
- **Total Impl Blocks**: {total_impls:,}
- **Technical Debt (TODO/FIXME)**: {total_todos + total_fixmes}

## Code Smells and Complexity Issues

### 🔴 Highest Complexity Files (Cyclomatic Complexity)
"""
        
        for i, metrics in enumerate(high_complexity, 1):
            report += f"{i}. **{metrics.file_path}** (Complexity: {metrics.cyclomatic_complexity}, LOC: {metrics.lines_of_code})\n"
        
        report += f"""
### 📏 Largest Files (Lines of Code)
"""
        
        for i, metrics in enumerate(large_files, 1):
            report += f"{i}. **{metrics.file_path}** ({metrics.lines_of_code:,} lines, {metrics.function_count} functions)\n"
        
        report += f"""
### 🌀 Deepest Nesting Files
"""
        
        for i, metrics in enumerate(deep_nesting, 1):
            report += f"{i}. **{metrics.file_path}** (Max depth: {metrics.nesting_depth}, Complexity: {metrics.cyclomatic_complexity})\n"
        
        report += f"""
### 📝 Files with Low Comment Ratios
"""
        
        for i, metrics in enumerate(low_comment_ratio, 1):
            if metrics.comment_ratio < 0.1:  # Less than 10% comments
                report += f"{i}. **{metrics.file_path}** (Comment ratio: {metrics.comment_ratio:.1%}, LOC: {metrics.lines_of_code})\n"
        
        report += f"""
### 🔧 Technical Debt Hotspots (TODO/FIXME)
"""
        
        for i, metrics in enumerate(high_technical_debt, 1):
            if metrics.todo_count + metrics.fixme_count > 0:
                report += f"{i}. **{metrics.file_path}** (TODO: {metrics.todo_count}, FIXME: {metrics.fixme_count})\n"
        
        report += f"""
### 🔄 Potential Code Duplication Issues
"""
        
        for i, metrics in enumerate(high_clone_issues, 1):
            if metrics.clone_issues > 0:
                report += f"{i}. **{metrics.file_path}** (Potential duplications: {metrics.clone_issues})\n"
        
        report += f"""
## Recommendations

### High Priority Issues:
1. **Refactor high complexity files** (Complexity > 50): Focus on breaking down large functions
2. **Split large files** (>1000 LOC): Consider modularizing into smaller, focused modules  
3. **Reduce deep nesting** (>5 levels): Extract nested logic into separate functions
4. **Add documentation** to files with low comment ratios (<10%)
5. **Address technical debt** by resolving TODO/FIXME comments

### Medium Priority:
1. **Review potential code duplications** and extract common functionality
2. **Improve test coverage** for complex modules
3. **Consider design patterns** for files with many impl blocks

### Maintainability Metrics:
- Files with complexity >20: {len([r for r in self.results if r.cyclomatic_complexity > 20])}
- Files with >500 LOC: {len([r for r in self.results if r.lines_of_code > 500])}
- Files with <5% comments: {len([r for r in self.results if r.comment_ratio < 0.05])}

### Overall Assessment:
"""
        
        # Provide overall assessment
        high_complexity_files = len([r for r in self.results if r.cyclomatic_complexity > 20])
        large_files_count = len([r for r in self.results if r.lines_of_code > 500])
        
        if high_complexity_files > total_files * 0.1:
            report += "⚠️  **High complexity concern**: More than 10% of files have high cyclomatic complexity.\n"
        if large_files_count > total_files * 0.2:
            report += "⚠️  **Large file concern**: More than 20% of files are quite large (>500 LOC).\n"
        if avg_comment_ratio < 0.1:
            report += "⚠️  **Documentation concern**: Average comment ratio is below 10%.\n"
        if total_todos + total_fixmes > total_files:
            report += "⚠️  **Technical debt concern**: High number of TODO/FIXME comments.\n"
        
        if all([
            high_complexity_files <= total_files * 0.1,
            large_files_count <= total_files * 0.2,
            avg_comment_ratio >= 0.1,
            total_todos + total_fixmes <= total_files
        ]):
            report += "✅ **Good maintainability**: The codebase shows good overall structure and maintainability.\n"
        
        return report


def main():
    if len(sys.argv) > 1:
        root_dir = sys.argv[1]
    else:
        root_dir = "."
    
    analyzer = CodeAnalyzer(root_dir)
    analyzer.analyze()
    report = analyzer.generate_report()
    print(report)


if __name__ == "__main__":
    main()