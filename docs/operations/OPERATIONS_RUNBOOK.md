# Orbit-RS Operations Runbook

## Quick Reference

### Emergency Scripts

- `/usr/local/bin/orbit-emergency-response.sh` - First response
- `/usr/local/bin/orbit-data-recovery.sh` - Data recovery
- `/usr/local/bin/orbit-disk-cleanup.sh` - Disk space recovery

### Health Endpoints

- `GET /health` - Basic health check
- `GET /metrics` - Prometheus metrics
- `GET /ready` - Readiness probe

### Key Commands

```bash

# Check service status
systemctl status orbit-rs

# View logs
journalctl -u orbit-rs -f

# Backend detection
orbit-cli detect-backend --data-dir=/var/lib/orbit

# Force snapshot
orbit-cli snapshot create --output=/tmp/emergency.json

# WAL replay
orbit-cli replay-wal --data-dir=/var/lib/orbit
```

## Common Scenarios

### 1. Service Won't Start

```bash

# Check logs for errors
journalctl -u orbit-rs --since "5 minutes ago"

# Check disk space
df -h /var/lib/orbit

# Validate configuration
orbit-cli validate-config --config=/etc/orbit/config.json

# Try safe mode
systemctl start orbit-rs --config=/etc/orbit/safe-mode.json
```

### 2. High Latency

```bash

# Check current performance
curl -s localhost:8080/metrics | grep latency

# Check system resources
top -p $(pgrep orbit-rs)

# Force garbage collection
orbit-cli gc-trigger

# Reduce cache if LSM
orbit-cli reduce-cache --block-cache-mb=128
```

### 3. Data Corruption

```bash

# Stop service
systemctl stop orbit-rs

# Backup corrupt data
cp -r /var/lib/orbit /backup/corrupt-$(date +%s)

# Restore from latest backup
/usr/local/bin/orbit-data-recovery.sh

# Validate recovery
orbit-cli validate --data-dir=/var/lib/orbit
```

## Configuration Switching

### Environment Variables

```bash

# Switch to COW B+ Tree
export ORBIT_PERSISTENCE_BACKEND=cow
systemctl restart orbit-rs

# Switch to LSM-Tree  
export ORBIT_PERSISTENCE_BACKEND=lsm
systemctl restart orbit-rs

# Switch to RocksDB
export ORBIT_PERSISTENCE_BACKEND=rocksdb
systemctl restart orbit-rs
```

### Performance Tuning

```bash

# Low memory configuration
cat > /tmp/low-memory.json << 'EOF'
{
  "backend": "CowBTree",
  "cow_config": {
    "max_keys_per_node": 32,
    "wal_buffer_size": 65536
  }
}
EOF

# High performance configuration
cat > /tmp/high-perf.json << 'EOF'
{
  "backend": "CowBTree", 
  "cow_config": {
    "max_keys_per_node": 256,
    "wal_buffer_size": 4194304
  }
}
EOF
```

## Monitoring Alerts

### Critical (P0)

- Service down > 30s
- Disk usage > 95%
- Memory usage > 95%
- Error rate > 1%

### Warning (P1)

- Write latency > 100ms
- Read latency > 50ms
- Disk usage > 85%
- Memory usage > 80%

## Backup & Recovery

### Daily Backup

```bash

# Automated via cron: 0 2 * * *
/usr/local/bin/orbit-backup.sh

# Manual backup
orbit-cli backup --output=/backup/manual-$(date +%s).tar.gz
```

### Recovery Process

1. Stop service: `systemctl stop orbit-rs`
2. Backup corrupt data: `cp -r /var/lib/orbit /backup/corrupt/`
3. Run recovery: `/usr/local/bin/orbit-data-recovery.sh`
4. Validate: `orbit-cli validate --data-dir=/var/lib/orbit`
5. Start service: `systemctl start orbit-rs`

## Performance Baselines

| Backend | Write (μs) | Read (μs) | Memory (MB) |
|---------|------------|-----------|-------------|
| COW B+ Tree | 41 | <1 | 256 |
| LSM-Tree | 38 | 0.3 | 512 |
| RocksDB | 53 | 19 | 1024 |

## Emergency Contacts

- **Primary**: DevOps (+1-555-0100)
- **Secondary**: SRE (+1-555-0101)  
- **Escalation**: Eng Manager (+1-555-0102)
