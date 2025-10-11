---
layout: default
title: Kubernetes Persistence Guide for Orbit-RS
category: documentation
---

# Kubernetes Persistence Guide for Orbit-RS

This guide explains how to configure Orbit-RS persistence backends when deploying on Kubernetes, using either the raw manifests, the Helm chart, or the Operator (CRDs).

## Quick Summary

- Local backends (LSM-Tree, RocksDB, COW B+Tree, Memory with disk backup) use StatefulSet with PVCs
- Cloud backends (S3, Azure, GCP) can use Deployment (no PVCs), and mount secrets for credentials
- You choose one backend per deployment via environment variable OR configuration

## Backend Selection

Set the persistence backend using one of these methods:

- Environment variable: ORBIT_PERSISTENCE_BACKEND
- In TOML config: [server] persistence_backend = "lsm_tree"

Supported values:
- memory, cow_btree, lsm_tree, rocksdb, s3, azure, gcp

## Manifests (k8s/)

Use the enhanced files:
- 01-configmap-enhanced.yaml: Adds persistence config sections and envsubst
- 03-statefulset-enhanced.yaml: Adds env vars and secrets for all backends

Examples:

- LSM-Tree (local SSD):
  - Set ORBIT_PERSISTENCE_BACKEND=lsm_tree
  - Ensure PVC with SSD StorageClass
  - Mount /app/data

- RocksDB (local SSD):
  - Set ORBIT_PERSISTENCE_BACKEND=rocksdb
  - Increase CPU/memory and SSD IOPS

- S3 (cloud):
  - Set ORBIT_PERSISTENCE_BACKEND=s3
  - Provide S3 secrets in orbit-server-secrets
  - You may use Deployment instead of StatefulSet

## Helm Chart (helm/orbit-rs)

To add cloud/local backend support via values:

values.yaml overrides example:

```
orbitServer:
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "rocksdb"
    - name: ORBIT_ROCKSDB_DATA_DIR
      value: "/app/data/rocksdb"

persistence:
  enabled: true
  storageClass: "gp3"
  accessMode: ReadWriteOnce
  size: 100Gi
```

For S3:
```
orbitServer:
  env:
    - name: ORBIT_PERSISTENCE_BACKEND
      value: "s3"
    - name: ORBIT_S3_ENDPOINT
      value: "https://s3.amazonaws.com"
    - name: ORBIT_S3_REGION
      value: "us-west-2"
    - name: ORBIT_S3_BUCKET
      value: "my-orbit"
    - name: ORBIT_S3_PREFIX
      value: "orbit"
    - name: ORBIT_S3_ENABLE_SSL
      value: "true"
```
Also create a secret named orbit-server-secrets with keys s3-access-key-id and s3-secret-access-key.

## Operator (CRDs)

The OrbitCluster CRD now supports a persistence section:

```
apiVersion: orbit.turingworks.com/v1
kind: OrbitCluster
metadata:
  name: demo
spec:
  replicas: 3
  image:
    repository: orbit-rs/orbit-server
    tag: latest
  persistence:
    backend: lsm_tree        # memory | cow_btree | lsm_tree | rocksdb | s3 | azure | gcp
    local:
      backend_type: lsm_tree
      data_dir: /app/data/lsm_tree
      enable_compression: true
      write_buffer_size: 134217728
      cache_size: 268435456
    # For cloud:
    # cloud:
    #   provider: s3
    #   endpoint: https://s3.amazonaws.com
    #   region: us-west-2
    #   bucket: orbit-production-state
    #   prefix: orbit
    #   connection_timeout: 30
    #   retry_count: 3
    #   enable_ssl: true
    #   credentials_secret: orbit-server-secrets
```

The operator should map this to env vars and volumes for the underlying StatefulSet/Deployment.

## Best Practices

- Local backends:
  - Use SSD-backed StorageClass
  - Pin pods across nodes (anti-affinity) and use ReadWriteOnce
  - Set appropriate resource limits, especially CPU for LSM compaction and RocksDB background jobs

- Cloud backends:
  - Use Deployment (no PVCs) for stateless pods
  - Store credentials in Secrets, mount or inject via env vars
  - Configure retries and timeouts conservatively

- Security:
  - Never inline secrets; use Kubernetes Secrets
  - Rotate credentials regularly

- Observability:
  - Enable Prometheus scraping (already present via annotations)
  - Watch persistence metrics to tune performance

## Migration Notes

- Existing Helm chart already supports persistence PVCs; just inject ORBIT_PERSISTENCE_BACKEND and backend-specific env vars via values.yaml.
- For cloud, consider adding a boolean like persistence.enabled=false and deploy the cloud Deployment variant.

## Files Added

- k8s/01-configmap-enhanced.yaml
- k8s/03-statefulset-enhanced.yaml

These provide ready-made templates to deploy with any supported backend.
