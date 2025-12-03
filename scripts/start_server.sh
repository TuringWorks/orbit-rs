#!/bin/bash
cd /Users/ravindraboddipalli/sources/git/orbit-rs
./target/release/orbit-server --data-dir ./test-data --postgres-port 5433 --bind 127.0.0.1 --redis-port 6380 --mysql-port 3307 --cql-port 9043 \u0026
