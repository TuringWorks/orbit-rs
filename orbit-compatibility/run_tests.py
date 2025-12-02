import os
import sys
import argparse
import subprocess
import time

# Define the protocols and their associated check scripts
# Paths are relative to the script location (orbit-compatibility/)
PROTOCOLS = {
    "postgresql": [
        "postgresql/compatibility_check.py",
        "postgresql/pgvector_check.py",
        "postgresql/timescale_check.py",
        "postgresql/pg18_check.py"
    ],
    "aql": ["aql/compatibility_check.py"],
    "cql": ["cql/compatibility_check.py"],
    "neo4j": ["neo4j/compatibility_check.py"],
    "mongodb": ["mongodb/compatibility_check.py"],
    "mysql": ["mysql/compatibility_check.py"],
    "redis": ["redis/compatibility_check.py"],
    "orbitql": ["orbitql/compatibility_check.py"],
}

def run_script(script_path):
    """Runs a single python script and returns True if successful."""
    full_path = os.path.join(os.path.dirname(__file__), script_path)
    if not os.path.exists(full_path):
        print(f"  [WARN] Script not found: {script_path}")
        return False

    print(f"  Running {script_path}...")
    start_time = time.time()
    try:
        # Run the script and capture output. 
        # We print output only if it fails or if verbose (optional, keeping simple for now)
        # Actually, user probably wants to see the output of the checks as they run.
        # Let's pipe stdout/stderr to the console.
        result = subprocess.run(
            [sys.executable, full_path],
            check=False # We handle return code manually
        )
        duration = time.time() - start_time
        
        if result.returncode == 0:
            print(f"  [PASS] {script_path} ({duration:.2f}s)")
            return True
        else:
            print(f"  [FAIL] {script_path} (Exit Code: {result.returncode})")
            return False
    except Exception as e:
        print(f"  [ERROR] Failed to execute {script_path}: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description="Run Orbit compatibility checks.")
    parser.add_argument(
        "--protocols", 
        nargs="+", 
        choices=list(PROTOCOLS.keys()) + ["all"],
        default=["all"],
        help="List of protocols to test (default: all)"
    )
    args = parser.parse_args()

    selected_protocols = args.protocols
    if "all" in selected_protocols:
        selected_protocols = list(PROTOCOLS.keys())

    print(f"Starting Orbit Compatibility Tests for: {', '.join(selected_protocols)}")
    print("=" * 60)

    results = {}
    total_scripts = 0
    passed_scripts = 0

    for protocol in selected_protocols:
        print(f"\nTesting Protocol: {protocol.upper()}")
        print("-" * 30)
        
        scripts = PROTOCOLS[protocol]
        protocol_passed = True
        
        for script in scripts:
            total_scripts += 1
            if run_script(script):
                passed_scripts += 1
            else:
                protocol_passed = False
        
        results[protocol] = "PASS" if protocol_passed else "FAIL"

    print("\n" + "=" * 60)
    print("TEST SUMMARY")
    print("=" * 60)
    
    for protocol, status in results.items():
        print(f"{protocol.ljust(15)}: {status}")
    
    print("-" * 60)
    print(f"Total Scripts: {total_scripts}")
    print(f"Passed:        {passed_scripts}")
    print(f"Failed:        {total_scripts - passed_scripts}")
    
    if passed_scripts == total_scripts:
        print("\nALL TESTS PASSED")
        sys.exit(0)
    else:
        print("\nSOME TESTS FAILED")
        sys.exit(1)

if __name__ == "__main__":
    main()
