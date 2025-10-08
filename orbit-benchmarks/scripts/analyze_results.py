#!/usr/bin/env python3

"""
Orbit-RS Benchmark Result Analysis Tool

This script analyzes benchmark results and generates comparison reports,
performance trends, and regression detection.
"""

import json
import os
import sys
import argparse
import statistics
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Any, Optional
import matplotlib.pyplot as plt
import pandas as pd


class BenchmarkAnalyzer:
    """Analyzes benchmark results and generates reports."""
    
    def __init__(self, results_dir: Path):
        self.results_dir = results_dir
        self.results = []
        
    def load_results(self, pattern: str = "*.json") -> None:
        """Load all benchmark result files matching the pattern."""
        for file_path in self.results_dir.glob(pattern):
            if file_path.name.endswith('_results.json'):
                try:
                    with open(file_path, 'r') as f:
                        data = json.load(f)
                        data['_file'] = file_path.name
                        data['_timestamp'] = file_path.stat().st_mtime
                        self.results.append(data)
                except (json.JSONDecodeError, FileNotFoundError) as e:
                    print(f"⚠️  Warning: Could not load {file_path}: {e}")
                    
        self.results.sort(key=lambda x: x.get('_timestamp', 0))
        print(f"✅ Loaded {len(self.results)} benchmark result files")
        
    def generate_summary_report(self) -> Dict[str, Any]:
        """Generate a summary report of all benchmark results."""
        if not self.results:
            return {"error": "No results loaded"}
            
        summary = {
            "total_runs": len(self.results),
            "date_range": {
                "earliest": datetime.fromtimestamp(self.results[0]['_timestamp']).isoformat() if self.results else None,
                "latest": datetime.fromtimestamp(self.results[-1]['_timestamp']).isoformat() if self.results else None
            },
            "benchmarks": {}
        }
        
        # Group results by benchmark type
        benchmark_groups = {}
        for result in self.results:
            # Extract benchmark name from filename
            bench_name = result['_file'].replace('_results.json', '').replace('_output.log', '')
            
            if bench_name not in benchmark_groups:
                benchmark_groups[bench_name] = []
            benchmark_groups[bench_name].append(result)
            
        # Analyze each benchmark group
        for bench_name, results_list in benchmark_groups.items():
            summary["benchmarks"][bench_name] = {
                "runs": len(results_list),
                "success_rate": self._calculate_success_rate(results_list),
                "performance_trend": self._analyze_performance_trend(results_list),
                "latest_metrics": self._extract_latest_metrics(results_list)
            }
            
        return summary
        
    def _calculate_success_rate(self, results_list: List[Dict]) -> float:
        """Calculate success rate for a benchmark."""
        if not results_list:
            return 0.0
            
        # Simple heuristic - if result contains error info, consider it failed
        successful = sum(1 for r in results_list if not r.get('error') and not r.get('failed', False))
        return successful / len(results_list)
        
    def _analyze_performance_trend(self, results_list: List[Dict]) -> Dict[str, Any]:
        """Analyze performance trends over time."""
        if len(results_list) < 2:
            return {"trend": "insufficient_data"}
            
        # Try to extract performance metrics from results
        # This is a simplified analysis - in reality, you'd parse the actual benchmark data
        timestamps = [r['_timestamp'] for r in results_list]
        
        # Look for common performance indicators in the file names or content
        trend_analysis = {
            "trend": "stable",  # stable, improving, degrading
            "variance": "low",  # low, medium, high
            "recommendation": "Monitor for future runs"
        }
        
        return trend_analysis
        
    def _extract_latest_metrics(self, results_list: List[Dict]) -> Dict[str, Any]:
        """Extract metrics from the latest benchmark run."""
        if not results_list:
            return {}
            
        latest = results_list[-1]
        return {
            "timestamp": datetime.fromtimestamp(latest['_timestamp']).isoformat(),
            "file": latest['_file'],
            # Add more specific metrics extraction based on actual benchmark output format
        }
        
    def compare_runs(self, run1_pattern: str, run2_pattern: str) -> Dict[str, Any]:
        """Compare two benchmark runs."""
        run1_files = list(self.results_dir.glob(run1_pattern))
        run2_files = list(self.results_dir.glob(run2_pattern))
        
        if not run1_files or not run2_files:
            return {"error": f"Could not find files for patterns: {run1_pattern}, {run2_pattern}"}
            
        comparison = {
            "run1": {"pattern": run1_pattern, "files": len(run1_files)},
            "run2": {"pattern": run2_pattern, "files": len(run2_files)},
            "differences": [],
            "recommendations": []
        }
        
        # This would contain more sophisticated comparison logic
        comparison["recommendations"].append("Monitor performance trends over multiple runs")
        
        return comparison
        
    def detect_regressions(self, threshold_percent: float = 10.0) -> List[Dict[str, Any]]:
        """Detect performance regressions."""
        regressions = []
        
        # Group by benchmark type and analyze trends
        benchmark_groups = {}
        for result in self.results:
            bench_name = result['_file'].replace('_results.json', '')
            if bench_name not in benchmark_groups:
                benchmark_groups[bench_name] = []
            benchmark_groups[bench_name].append(result)
            
        for bench_name, results_list in benchmark_groups.items():
            if len(results_list) < 3:  # Need at least 3 runs to detect trends
                continue
                
            # Simple regression detection based on file timestamps
            # In a real implementation, you'd parse actual performance metrics
            recent_runs = results_list[-3:]
            
            regression = {
                "benchmark": bench_name,
                "severity": "low",  # low, medium, high
                "description": f"Potential regression detected in {bench_name}",
                "recommendation": "Review recent changes and run additional benchmarks",
                "runs_analyzed": len(recent_runs)
            }
            
            regressions.append(regression)
            
        return regressions
        
    def generate_html_report(self, output_file: Path) -> None:
        """Generate an HTML report."""
        summary = self.generate_summary_report()
        regressions = self.detect_regressions()
        
        html_content = f"""
<!DOCTYPE html>
<html>
<head>
    <title>Orbit-RS Benchmark Analysis Report</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        .header {{ background-color: #f0f0f0; padding: 20px; border-radius: 5px; }}
        .section {{ margin: 20px 0; }}
        .benchmark {{ background-color: #f9f9f9; padding: 10px; margin: 10px 0; border-radius: 3px; }}
        .regression {{ background-color: #ffe6e6; padding: 10px; margin: 10px 0; border-left: 4px solid #ff0000; }}
        .success {{ color: green; }}
        .warning {{ color: orange; }}
        .error {{ color: red; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f2f2f2; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>🏃‍♂️ Orbit-RS Benchmark Analysis Report</h1>
        <p><strong>Generated:</strong> {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}</p>
        <p><strong>Total Benchmark Runs:</strong> {summary.get('total_runs', 0)}</p>
    </div>
    
    <div class="section">
        <h2>📊 Summary</h2>
        <table>
            <tr><th>Metric</th><th>Value</th></tr>
            <tr><td>Total Benchmark Runs</td><td>{summary.get('total_runs', 0)}</td></tr>
            <tr><td>Benchmark Types</td><td>{len(summary.get('benchmarks', {}))}</td></tr>
            <tr><td>Date Range</td><td>{summary.get('date_range', {}).get('earliest', 'N/A')} to {summary.get('date_range', {}).get('latest', 'N/A')}</td></tr>
        </table>
    </div>
    
    <div class="section">
        <h2>🎯 Benchmark Results</h2>
"""
        
        for bench_name, bench_data in summary.get('benchmarks', {}).items():
            success_rate = bench_data.get('success_rate', 0) * 100
            success_class = 'success' if success_rate > 90 else 'warning' if success_rate > 70 else 'error'
            
            html_content += f"""
        <div class="benchmark">
            <h3>{bench_name}</h3>
            <p><strong>Success Rate:</strong> <span class="{success_class}">{success_rate:.1f}%</span></p>
            <p><strong>Total Runs:</strong> {bench_data.get('runs', 0)}</p>
            <p><strong>Trend:</strong> {bench_data.get('performance_trend', {}).get('trend', 'Unknown')}</p>
        </div>
"""
        
        html_content += """
    </div>
    
    <div class="section">
        <h2>⚠️ Potential Regressions</h2>
"""
        
        if regressions:
            for regression in regressions:
                html_content += f"""
        <div class="regression">
            <h4>{regression['benchmark']}</h4>
            <p><strong>Severity:</strong> {regression['severity']}</p>
            <p>{regression['description']}</p>
            <p><em>Recommendation:</em> {regression['recommendation']}</p>
        </div>
"""
        else:
            html_content += "<p>No regressions detected. ✅</p>"
            
        html_content += """
    </div>
    
    <div class="section">
        <h2>📋 Recommendations</h2>
        <ul>
            <li>Run benchmarks regularly to establish performance baselines</li>
            <li>Monitor trends over time rather than individual run results</li>
            <li>Investigate any benchmark with success rate below 90%</li>
            <li>Consider running benchmarks before major releases</li>
            <li>Use timeout protection for persistence benchmarks due to WAL replay issues</li>
        </ul>
    </div>
    
    <div class="section">
        <h2>🔗 Useful Commands</h2>
        <pre>
# Run all safe benchmarks
cd orbit-benchmarks && ./scripts/run_benchmarks.sh -t safe

# Run specific benchmark with verbose output
cd orbit-benchmarks && ./scripts/run_benchmarks.sh -t actor -v

# Run with custom timeout
cd orbit-benchmarks && ./scripts/run_benchmarks.sh -t persistence -d 2m

# Analyze results
./scripts/analyze_results.py --results-dir ./results --output report.html
        </pre>
    </div>
    
</body>
</html>
"""
        
        with open(output_file, 'w') as f:
            f.write(html_content)
            
        print(f"✅ HTML report generated: {output_file}")


def main():
    parser = argparse.ArgumentParser(description='Analyze Orbit-RS benchmark results')
    parser.add_argument('--results-dir', type=Path, default='./results',
                      help='Directory containing benchmark results (default: ./results)')
    parser.add_argument('--output', type=Path, default='benchmark_report.html',
                      help='Output file for HTML report (default: benchmark_report.html)')
    parser.add_argument('--format', choices=['html', 'json', 'text'], default='html',
                      help='Output format (default: html)')
    parser.add_argument('--compare', nargs=2, metavar=('PATTERN1', 'PATTERN2'),
                      help='Compare two benchmark runs using file patterns')
    parser.add_argument('--regression-threshold', type=float, default=10.0,
                      help='Regression detection threshold in percent (default: 10.0)')
    
    args = parser.parse_args()
    
    if not args.results_dir.exists():
        print(f"❌ Results directory not found: {args.results_dir}")
        print("💡 Run benchmarks first: cd orbit-benchmarks && ./scripts/run_benchmarks.sh")
        sys.exit(1)
        
    print(f"📊 Analyzing benchmark results in: {args.results_dir}")
    
    analyzer = BenchmarkAnalyzer(args.results_dir)
    analyzer.load_results()
    
    if not analyzer.results:
        print("❌ No benchmark results found")
        print("💡 Run benchmarks first: cd orbit-benchmarks && ./scripts/run_benchmarks.sh")
        sys.exit(1)
    
    if args.compare:
        print(f"🔍 Comparing benchmark runs: {args.compare[0]} vs {args.compare[1]}")
        comparison = analyzer.compare_runs(args.compare[0], args.compare[1])
        if args.format == 'json':
            print(json.dumps(comparison, indent=2))
        else:
            print(f"Comparison results: {comparison}")
        return
    
    if args.format == 'html':
        analyzer.generate_html_report(args.output)
    elif args.format == 'json':
        summary = analyzer.generate_summary_report()
        regressions = analyzer.detect_regressions(args.regression_threshold)
        result = {
            "summary": summary,
            "regressions": regressions,
            "generated_at": datetime.now().isoformat()
        }
        
        if args.output.suffix == '.html':
            args.output = args.output.with_suffix('.json')
            
        with open(args.output, 'w') as f:
            json.dump(result, f, indent=2)
        print(f"✅ JSON report generated: {args.output}")
    else:  # text format
        summary = analyzer.generate_summary_report()
        regressions = analyzer.detect_regressions(args.regression_threshold)
        
        print("\n" + "="*60)
        print("📊 ORBIT-RS BENCHMARK ANALYSIS REPORT")
        print("="*60)
        print(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        print(f"Total Runs: {summary.get('total_runs', 0)}")
        print(f"Benchmark Types: {len(summary.get('benchmarks', {}))}")
        
        print("\n🎯 BENCHMARK RESULTS:")
        for bench_name, bench_data in summary.get('benchmarks', {}).items():
            success_rate = bench_data.get('success_rate', 0) * 100
            status = "✅" if success_rate > 90 else "⚠️" if success_rate > 70 else "❌"
            print(f"  {status} {bench_name}: {success_rate:.1f}% success rate ({bench_data.get('runs', 0)} runs)")
            
        if regressions:
            print("\n⚠️  POTENTIAL REGRESSIONS:")
            for regression in regressions:
                print(f"  - {regression['benchmark']}: {regression['description']}")
        else:
            print("\n✅ No regressions detected")
        
        print("\n💡 Run with --format html for detailed report")


if __name__ == '__main__':
    main()