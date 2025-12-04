import os
import sys
import argparse
import subprocess
import time
import socket
import signal

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

SERVER_PROCESS = None

def wait_for_port(port, host='localhost', timeout=600.0):
    """Wait until a port accepts connections."""
    start_time = time.time()
    while True:
        try:
            with socket.create_connection((host, port), timeout=1):
                return True
        except (OSError, ConnectionRefusedError):
            if time.time() - start_time >= timeout:
                return False
            time.sleep(0.5)

def start_server():
    """Starts the Orbit server using the startup script."""
    global SERVER_PROCESS
    print("Starting Orbit Server (this may take a while if building)...")
    
    # Path to the startup script relative to this script
    script_dir = os.path.dirname(os.path.abspath(__file__))
    root_dir = os.path.dirname(script_dir)
    startup_script = os.path.join(root_dir, "scripts", "start-multiprotocol-server.sh")
    
    if not os.path.exists(startup_script):
        print(f"  [ERROR] Startup script not found: {startup_script}")
        return False 

    try:
        # Open log file (this log file will not be used by subprocess.Popen if stdout/stderr are None)
        log_file = open(os.path.join(root_dir, "server_startup.log"), "w")
        
        # Start the server in a separate process group so we can kill it and its children
        SERVER_PROCESS = subprocess.Popen(
            [startup_script],
            cwd=root_dir,
            stdout=log_file,
            stderr=subprocess.STDOUT,
            preexec_fn=os.setsid
        )
        
        # Wait for key ports to be ready (Postgres: 5432, Redis: 6379)
        print("  Waiting for server to be ready (logs in server_startup.log)...", end="", flush=True)
        if wait_for_port(5432) and wait_for_port(6379):
            print(" READY")
            return True
        else:
            print(" TIMEOUT")
            stop_server()
            return False
            
    except Exception as e:
        print(f"  [ERROR] Failed to start server: {e}")
        return False

def stop_server():
    """Stops the Orbit server."""
    global SERVER_PROCESS
    if SERVER_PROCESS:
        print("Stopping Orbit Server...")
        try:
            os.killpg(os.getpgid(SERVER_PROCESS.pid), signal.SIGTERM)
            SERVER_PROCESS.wait(timeout=10)
        except Exception as e:
            print(f"  [WARN] Error stopping server: {e}")
        SERVER_PROCESS = None

def install_dependencies(protocol):
    """Installs dependencies for a given protocol."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    req_file = os.path.join(script_dir, protocol, "requirements.txt")
    
    if os.path.exists(req_file):
        print(f"  Installing dependencies for {protocol}...", end="", flush=True)
        try:
            subprocess.check_call(
                [sys.executable, "-m", "pip", "install", "-r", req_file],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL
            )
            print(" DONE")
            return True
        except subprocess.CalledProcessError:
            print(" FAIL")
            return False
    return True

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
    parser.add_argument(
        "--start-server",
        action="store_true",
        help="Start the Orbit server before running tests"
    )
    parser.add_argument(
        "--skip-install",
        action="store_true",
        help="Skip automatic dependency installation"
    )
    args = parser.parse_args()

    if args.start_server:
        if not start_server():
            print("Failed to start server. Aborting tests.")
            sys.exit(1)




    try:
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
            
            if not args.skip_install:
                install_dependencies(protocol)
            
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
            
    finally:
        if args.start_server:
            stop_server()

if __name__ == "__main__":
    main()
