#!/usr/bin/env python3
"""
Run Orbit-RS gRPC quick examples via grpcurl if available.

This script shells out to grpcurl to list services and perform health checks.
Install grpcurl: https://github.com/fullstorydev/grpcurl
"""

import shutil
import subprocess


def run(cmd):
    print("$", " ".join(cmd))
    try:
        out = subprocess.check_output(cmd, stderr=subprocess.STDOUT)
        print(out.decode("utf-8"))
    except subprocess.CalledProcessError as e:
        print(e.output.decode("utf-8"))


def main():
    print("=" * 60)
    print("Orbit ML Examples - gRPC Quick Examples")
    print("=" * 60)

    if shutil.which("grpcurl") is None:
        print("grpcurl not found. Install grpcurl to run gRPC quick examples.")
        return

    # List services
    run(["grpcurl", "-plaintext", "localhost:50051", "list"]) 

    # Health check
    run([
        "grpcurl", "-plaintext", "-d", '{"service":"orbit-server"}',
        "localhost:50051", "orbit.shared.HealthService/Check",
    ])

    # Transaction health check
    run([
        "grpcurl", "-plaintext", "-d", '{"node_id":"node-1"}',
        "localhost:50051", "orbit.transactions.TransactionService/TransactionHealthCheck",
    ])

    print("=" * 60)
    print("gRPC quick examples completed.")


if __name__ == "__main__":
    main()

