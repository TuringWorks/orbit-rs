#!/usr/bin/env python3
"""
Simple script to create MinIO bucket using boto3-compatible library
"""
import sys

try:
    from minio import Minio
    from minio.error import S3Error
    
    # Create MinIO client
    client = Minio(
        "localhost:9000",
        access_key="minioadmin",
        secret_key="minioadmin",
        secure=False
    )
    
    bucket_name = "orbit-cold-storage"
    
    # Check if bucket exists
    if client.bucket_exists(bucket_name):
        print(f"✓ Bucket '{bucket_name}' already exists")
    else:
        # Create bucket
        client.make_bucket(bucket_name)
        print(f"✓ Created bucket '{bucket_name}'")
    
    # Test write
    test_data = b"test data"
    client.put_object(bucket_name, "test.txt", 
                     io.BytesIO(test_data), len(test_data))
    print("✓ Successfully wrote test file")
    
    # Clean up
    client.remove_object(bucket_name, "test.txt")
    print("✓ Test complete")
    
except ImportError:
    print("Error: minio package not installed")
    print("Install with: pip install minio")
    sys.exit(1)
except S3Error as e:
    print(f"Error: {e}")
    sys.exit(1)
