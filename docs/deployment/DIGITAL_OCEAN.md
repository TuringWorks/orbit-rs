---
layout: default
title: Digital Ocean Deployment Guide
category: deployment
---

## Digital Ocean Deployment Guide

This guide provides comprehensive instructions for deploying Orbit-RS to Digital Ocean using droplets with optional GPU support and Digital Ocean Spaces for object storage.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Infrastructure Setup](#infrastructure-setup)
- [Digital Ocean Spaces Configuration](#digital-ocean-spaces-configuration)
- [GPU Droplets](#gpu-droplets)
- [Deployment Methods](#deployment-methods)
- [Monitoring and Observability](#monitoring-and-observability)
- [Cost Optimization](#cost-optimization)
- [Troubleshooting](#troubleshooting)
- [Best Practices](#best-practices)

## Overview

Orbit-RS supports deployment to Digital Ocean using:

- **Standard Droplets**: For general compute workloads
- **GPU Droplets**: For AI/ML and compute-intensive workloads (H100, A100, V100)
- **Digital Ocean Spaces**: S3-compatible object storage with CDN support
- **Load Balancers**: For high availability and traffic distribution
- **VPC**: For network isolation and security
- **Auto-scaling**: Based on CPU, memory, and GPU utilization

### Architecture Diagram

```text
┌─────────────────┐    ┌─────────────────┐
│   Load Balancer │────│   Firewall      │
└─────────────────┘    └─────────────────┘
          │                       │
          ▼                       ▼
┌─────────────────┐    ┌─────────────────┐
│ Standard        │    │ GPU Droplets    │
│ Droplets        │    │ (H100/A100)     │
│ - orbit-server  │    │ - orbit-compute │
│ - orbit-client  │    │ - ML workloads  │
│ - orbit-operator│    │                 │
└─────────────────┘    └─────────────────┘
          │                       │
          └───────────────────────┘
                      │
                      ▼
            ┌─────────────────┐
            │ Digital Ocean   │
            │ Spaces          │
            │ (Object Storage)│
            └─────────────────┘
```

## Prerequisites

### Digital Ocean Account Setup

1. **Create a Digital Ocean Account**
   - Sign up at [digitalocean.com](https://digitalocean.com)
   - Add a payment method
   - Verify your email

2. **Generate API Token**

   ```bash
   # Go to Digital Ocean Control Panel > API > Tokens
   # Generate a new Personal Access Token with read/write permissions
   export DO_API_TOKEN="your_api_token_here"
   ```

3. **Create SSH Key**

   ```bash
   # Generate SSH key pair
   ssh-keygen -t ed25519 -C "your_email@example.com" -f ~/.ssh/orbit-rs-do
   
   # Add public key to Digital Ocean
   # Control Panel > Settings > Security > SSH Keys
   # Copy content of ~/.ssh/orbit-rs-do.pub
   
   # Get the SSH key ID
   doctl compute ssh-key list
   export DO_SSH_KEY_ID="your_ssh_key_id"
   ```

### Local Environment

1. **Install Required Tools**

   ```bash
   # Install doctl (Digital Ocean CLI)
   # macOS
   brew install doctl
   
   # Linux
   snap install doctl
   
   # Or download from: https://github.com/digitalocean/doctl/releases
   
   # Authenticate doctl
   doctl auth init
   ```

2. **Install Docker**

   ```bash
   # Required for local container operations
   curl -fsSL https://get.docker.com | sh
   ```

3. **Clone Repository**

   ```bash
   git clone https://github.com/your-org/orbit-rs.git
   cd orbit-rs
   ```

## Quick Start

### 1. Environment Variables

Create a `.env` file with your Digital Ocean credentials:

```bash
# Digital Ocean API Configuration
DO_API_TOKEN="your_api_token_here"
DO_SSH_KEY_ID="your_ssh_key_id"

# Digital Ocean Spaces Configuration  
DO_SPACES_NAME="orbit-rs-storage"
DO_SPACES_REGION="nyc3"
DO_SPACES_ACCESS_KEY="your_spaces_access_key"
DO_SPACES_SECRET_KEY="your_spaces_secret_key"

# Optional: Backup Spaces
DO_SPACES_BACKUP_NAME="orbit-rs-backup"
DO_SPACES_BACKUP_ACCESS_KEY="your_backup_access_key"
DO_SPACES_BACKUP_SECRET_KEY="your_backup_secret_key"

# Load Balancer Configuration
DO_LB_ID="will_be_set_after_creation"

# Monitoring
GRAFANA_ADMIN_PASSWORD="secure_password"
SLACK_WEBHOOK_URL="https://hooks.slack.com/services/..."
```

### 2. Quick Deployment

Using the GitHub Actions workflow:

```bash
# Trigger deployment via GitHub UI or API
gh workflow run digitalocean-deploy.yml \
  -f deployment_environment=staging \
  -f enable_gpu_droplets=true \
  -f droplet_count=3 \
  -f gpu_droplet_count=1
```

Or deploy manually using configuration files:

```bash
# Deploy using example configuration
cp deploy/examples/digitalocean-droplets-config.yaml deploy/config/production.yaml

# Edit the configuration
vim deploy/config/production.yaml

# Deploy using infrastructure-as-code tools (recommended)
# See "Deployment Methods" section below
```

## Infrastructure Setup

### 1. VPC and Networking

Create a Virtual Private Cloud for network isolation:

```bash
# Create VPC
VPC_ID=$(doctl compute vpc create \
  --name "orbit-rs-vpc" \
  --region "nyc3" \
  --ip-range "10.116.0.0/20" \
  --format ID --no-header)

echo "VPC_ID=${VPC_ID}"
```

### 2. Firewall Rules

```bash
# Create firewall
FIREWALL_ID=$(doctl compute firewall create \
  --name "orbit-rs-firewall" \
  --inbound-rules "protocol:tcp,ports:22,address:YOUR_IP/32 protocol:tcp,ports:50051,address:10.116.0.0/20 protocol:tcp,ports:8080,address:10.116.0.0/20" \
  --outbound-rules "protocol:tcp,ports:443,address:0.0.0.0/0 protocol:tcp,ports:80,address:0.0.0.0/0" \
  --format ID --no-header)

echo "FIREWALL_ID=${FIREWALL_ID}"
```

### 3. Load Balancer

```bash
# Create load balancer
LB_ID=$(doctl compute load-balancer create \
  --name "orbit-rs-lb" \
  --region "nyc3" \
  --size "lb-small" \
  --algorithm "round_robin" \
  --forwarding-rules "entry_protocol:http,entry_port:80,target_protocol:http,target_port:8080" \
  --health-check "protocol:http,port:8080,path:/health" \
  --format ID --no-header)

echo "LB_ID=${LB_ID}"
```

### 4. Standard Droplets

```bash
# Create standard compute droplets
for i in {1..3}; do
  DROPLET_ID=$(doctl compute droplet create "orbit-rs-${i}" \
    --image ubuntu-22-04-x64 \
    --size s-2vcpu-4gb \
    --region nyc3 \
    --vpc-uuid $VPC_ID \
    --user-data-file deploy/scripts/standard-droplet-init.sh \
    --ssh-keys $DO_SSH_KEY_ID \
    --tag-names "orbit-rs,standard-compute" \
    --wait \
    --format ID --no-header)
  
  echo "Created droplet: orbit-rs-${i} (ID: ${DROPLET_ID})"
  
  # Add to firewall and load balancer
  doctl compute firewall add-droplets $FIREWALL_ID --droplet-ids $DROPLET_ID
  doctl compute load-balancer add-droplets $LB_ID --droplet-ids $DROPLET_ID
done
```

### 5. GPU Droplets (Optional)

```bash
# Create GPU droplets (if needed)
GPU_DROPLET_ID=$(doctl compute droplet create "orbit-rs-gpu-1" \
  --image gpu-h100x1-base \
  --size gd-8vcpu-32gb-nvidia-h100x1 \
  --region nyc3 \
  --vpc-uuid $VPC_ID \
  --user-data-file deploy/scripts/gpu-droplet-init.sh \
  --ssh-keys $DO_SSH_KEY_ID \
  --tag-names "orbit-rs,gpu-compute,ml-workload" \
  --wait \
  --format ID --no-header)

echo "Created GPU droplet: orbit-rs-gpu-1 (ID: ${GPU_DROPLET_ID})"
```

## Digital Ocean Spaces Configuration

### 1. Create Spaces Bucket

```bash
# Create primary storage space
doctl spaces bucket create $DO_SPACES_NAME --region $DO_SPACES_REGION

# Create backup space (different region for DR)
doctl spaces bucket create $DO_SPACES_BACKUP_NAME --region sfo3
```

### 2. Configure CORS (if needed)

```bash
# Create CORS configuration
cat > cors-config.json << 'EOF'
{
  "CORSRules": [
    {
      "AllowedHeaders": ["*"],
      "AllowedMethods": ["GET", "POST", "PUT", "DELETE", "HEAD"],
      "AllowedOrigins": ["*"],
      "MaxAgeSeconds": 3000
    }
  ]
}
EOF

# Apply CORS configuration
doctl spaces bucket put-cors $DO_SPACES_NAME --config cors-config.json
```

### 3. CDN Configuration

```bash
# Enable CDN for your space via the Digital Ocean control panel
# The CDN endpoint will be: ${DO_SPACES_NAME}.${DO_SPACES_REGION}.cdn.digitaloceanspaces.com
```

### 4. Orbit-RS Configuration

Create a configuration file for Orbit-RS to use Digital Ocean Spaces:

```toml
# /opt/orbit-rs/config/orbit-server.toml
[storage]
provider = "digitalocean_spaces"

[storage.digitalocean_spaces]
endpoint = "nyc3.digitaloceanspaces.com"
region = "nyc3"
space_name = "orbit-rs-storage"
access_key_id = "${DO_SPACES_ACCESS_KEY}"
secret_access_key = "${DO_SPACES_SECRET_KEY}"
enable_ssl = true
enable_cdn = true
enable_encryption = true
prefix = "orbit-rs-production"

[storage.digitalocean_spaces.tags]
project = "orbit-rs"
environment = "production"
```

## GPU Droplets

### Available GPU Droplet Types

Digital Ocean offers several GPU droplet types:

| Size | vCPUs | RAM | GPU | GPU Memory | Storage | Price/hour |
|------|-------|-----|-----|------------|---------|------------|
| `gd-2vcpu-8gb-nvidia-rtx-4000` | 2 | 8 GB | RTX 4000 | 20 GB | 200 GB SSD | ~$0.50 |
| `gd-4vcpu-16gb-nvidia-rtx-5000` | 4 | 16 GB | RTX 5000 | 32 GB | 400 GB SSD | ~$1.00 |
| `gd-8vcpu-32gb-nvidia-h100x1` | 8 | 32 GB | H100 | 80 GB | 800 GB SSD | ~$3.00 |
| `gd-16vcpu-64gb-nvidia-h100x2` | 16 | 64 GB | 2x H100 | 160 GB | 1.6 TB SSD | ~$6.00 |

> **Note:** Prices are approximate and vary by region

### GPU Droplet Configuration

```yaml
# deploy/config/gpu-droplets.yaml
gpu_droplets:
  image: "gpu-h100x1-base"
  size: "gd-8vcpu-32gb-nvidia-h100x1"
  region: "nyc3"
  count: 2
  
  gpu_config:
    driver_version: "535.154.05"
    cuda_version: "12.2"
    cudnn_version: "8.9"
    enable_persistence_mode: true
    memory_clock: 1215
    graphics_clock: 1410
    power_limit: 350
    memory_fraction: 0.9

  monitoring:
    gpu_metrics: true
    temperature_threshold: 83
    power_threshold: 300
    memory_threshold: 80
    enable_nvml: true
```

### GPU Workload Deployment

```bash
# Deploy GPU-optimized containers
docker run -d \
  --name orbit-compute-gpu \
  --restart unless-stopped \
  --gpus all \
  -e NVIDIA_VISIBLE_DEVICES=all \
  -e CUDA_VISIBLE_DEVICES=0 \
  -e GPU_MEMORY_FRACTION=0.8 \
  ghcr.io/your-org/orbit-rs/orbit-compute:latest-gpu
```

## Deployment Methods

### Method 1: GitHub Actions (Recommended)

The automated deployment pipeline handles everything:

```yaml
# .github/workflows/digitalocean-deploy.yml is already configured
# Trigger deployment:
name: Deploy to Digital Ocean
on:
  workflow_dispatch:
    inputs:
      environment:
        type: choice
        options: [development, staging, production]
      enable_gpu:
        type: boolean
        default: false
```

### Method 2: Terraform (Infrastructure as Code)

```hcl
# terraform/digitalocean/main.tf
terraform {
  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "~> 2.0"
    }
  }
}

provider "digitalocean" {
  token = var.do_token
}

resource "digitalocean_vpc" "orbit_vpc" {
  name     = "orbit-rs-vpc"
  region   = var.region
  ip_range = "10.116.0.0/20"
}

resource "digitalocean_droplet" "orbit_servers" {
  count    = var.droplet_count
  image    = "ubuntu-22-04-x64"
  name     = "orbit-rs-${count.index + 1}"
  region   = var.region
  size     = "s-2vcpu-4gb"
  vpc_uuid = digitalocean_vpc.orbit_vpc.id
  
  user_data = file("${path.module}/scripts/droplet-init.sh")
  
  ssh_keys = [var.ssh_key_id]
  
  tags = ["orbit-rs", "standard-compute"]
}
```

### Method 3: Ansible Playbooks

```yaml
# ansible/digitalocean.yml
---
- name: Deploy Orbit-RS to Digital Ocean
  hosts: localhost
  vars:
    do_token: "{{ lookup('env', 'DO_API_TOKEN') }}"
    droplet_count: 3
    
  tasks:
    - name: Create VPC
      digital_ocean_vpc:
        api_token: "{{ do_token }}"
        name: "orbit-rs-vpc"
        region: "nyc3"
        ip_range: "10.116.0.0/20"
        state: present
      register: vpc_result
      
    - name: Create droplets
      digital_ocean_droplet:
        api_token: "{{ do_token }}"
        name: "orbit-rs-{{ item }}"
        size: "s-2vcpu-4gb"
        image: "ubuntu-22-04-x64"
        region: "nyc3"
        vpc_uuid: "{{ vpc_result.vpc.id }}"
        user_data: "{{ lookup('file', 'scripts/droplet-init.sh') }}"
        ssh_keys: ["{{ ssh_key_id }}"]
        tags: ["orbit-rs", "standard-compute"]
        state: present
      loop: "{{ range(1, droplet_count + 1) | list }}"
```

### Method 4: Docker Compose (Single Droplet)

For smaller deployments on a single droplet:

```yaml
# docker-compose.digitalocean.yml
version: '3.8'

services:
  orbit-server:
    image: ghcr.io/your-org/orbit-rs/orbit-server:latest
    ports:
      - "50051:50051"
      - "8080:8080"
      - "9090:9090"
    environment:
      - DEPLOYMENT_MODE=digital_ocean
      - DO_SPACES_NAME=${DO_SPACES_NAME}
      - DO_SPACES_REGION=${DO_SPACES_REGION}
      - DO_SPACES_ACCESS_KEY=${DO_SPACES_ACCESS_KEY}
      - DO_SPACES_SECRET_KEY=${DO_SPACES_SECRET_KEY}
    volumes:
      - orbit_data:/app/data
      - orbit_logs:/app/logs
    restart: unless-stopped

  orbit-client:
    image: ghcr.io/your-org/orbit-rs/orbit-client:latest
    depends_on:
      - orbit-server
    environment:
      - ORBIT_SERVER_URL=http://orbit-server:50051
    restart: unless-stopped

volumes:
  orbit_data:
  orbit_logs:
```

## Monitoring and Observability

### 1. Digital Ocean Monitoring

Digital Ocean provides built-in monitoring:

```bash
# Enable monitoring for droplets
doctl compute droplet-action enable-monitoring $DROPLET_ID
```

### 2. Prometheus and Grafana

Deploy monitoring stack:

```yaml
# monitoring/docker-compose.yml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_ADMIN_PASSWORD}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources

volumes:
  grafana_data:
```

### 3. GPU Monitoring

For GPU droplets, additional monitoring is available:

```yaml
# GPU monitoring with DCGM
services:
  dcgm-exporter:
    image: nvcr.io/nvidia/k8s/dcgm-exporter:3.1.8-3.1.5-ubuntu20.04
    runtime: nvidia
    environment:
      - NVIDIA_VISIBLE_DEVICES=all
    ports:
      - "9400:9400"
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: all
              capabilities: [gpu]
```

### 4. Log Aggregation

```bash
# Configure log forwarding to Digital Ocean's log aggregation service
# or use external services like Grafana Loki, ELK, etc.

# Example: Forward logs to external service
curl -X POST "https://api.digitalocean.com/v2/monitoring/logs/forward" \
  -H "Authorization: Bearer $DO_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "orbit-rs-logs",
    "config": {
      "endpoint": "your-log-service-endpoint",
      "format": "json"
    }
  }'
```

## Cost Optimization

### 1. Right-sizing Instances

Monitor resource usage and adjust droplet sizes:

```bash
# Get droplet metrics
doctl monitoring metrics droplet cpu \
  --start $(date -d '1 hour ago' --iso-8601) \
  --end $(date --iso-8601) \
  $DROPLET_ID

# Resize droplet if needed (requires shutdown)
doctl compute droplet-action resize $DROPLET_ID --size s-1vcpu-2gb
```

### 2. Reserved Instances

While Digital Ocean doesn't offer traditional reserved instances, consider:

- **Long-term discounts**: Contact sales for volume discounts
- **Scheduled scaling**: Scale down during off-hours

```bash
# Example: Automated scaling script
cat > scale-down.sh << 'EOF'
#!/bin/bash
# Scale down non-production environments during off-hours

if [[ $(date +%H) -gt 20 || $(date +%H) -lt 8 ]]; then
  # Scale down development droplets
  doctl compute droplet delete orbit-rs-dev-2 orbit-rs-dev-3 --force
fi
EOF
```

### 3. Storage Optimization

```bash
# Set up lifecycle policies for Spaces
cat > lifecycle-policy.json << 'EOF'
{
  "Rules": [
    {
      "ID": "archive-old-logs",
      "Status": "Enabled",
      "Filter": {
        "Prefix": "logs/"
      },
      "Transitions": [
        {
          "Days": 30,
          "StorageClass": "GLACIER"
        }
      ],
      "Expiration": {
        "Days": 365
      }
    }
  ]
}
EOF

# Apply lifecycle policy (when supported)
# doctl spaces bucket put-lifecycle $DO_SPACES_NAME --config lifecycle-policy.json
```

### 4. Auto-scaling Configuration

```yaml
# Auto-scaling based on metrics
auto_scaling:
  enabled: true
  
  standard:
    min_instances: 2
    max_instances: 10
    target_cpu_utilization: 70
    scale_up_cooldown: "5m"
    scale_down_cooldown: "10m"
    
  gpu:
    min_instances: 1
    max_instances: 4
    target_gpu_utilization: 80
    scale_up_cooldown: "10m"
    scale_down_cooldown: "20m"
```

## Troubleshooting

### Common Issues

#### 1. Droplet Creation Failed

```bash
# Check quota limits
doctl account get

# Check available sizes in region
doctl compute size list --region nyc3

# Check available images
doctl compute image list --public --type distribution
```

#### 2. GPU Droplet Issues

```bash
# SSH into GPU droplet and check GPU status
doctl compute ssh $GPU_DROPLET_ID --ssh-command "nvidia-smi"

# Check CUDA installation
doctl compute ssh $GPU_DROPLET_ID --ssh-command "nvcc --version"

# Check Docker GPU support
doctl compute ssh $GPU_DROPLET_ID --ssh-command "docker run --rm --gpus all nvidia/cuda:11.0-base nvidia-smi"
```

#### 3. Spaces Connectivity Issues

```bash
# Test Spaces connectivity
curl -I https://${DO_SPACES_REGION}.digitaloceanspaces.com/${DO_SPACES_NAME}/

# Check credentials
s3cmd --host=${DO_SPACES_REGION}.digitaloceanspaces.com \
      --access_key=${DO_SPACES_ACCESS_KEY} \
      --secret_key=${DO_SPACES_SECRET_KEY} \
      ls s3://${DO_SPACES_NAME}/
```

#### 4. Load Balancer Health Checks Failing

```bash
# Check droplet health endpoints
for droplet_ip in $(doctl compute droplet list --tag-name orbit-rs --format PublicIPv4 --no-header); do
  echo "Checking $droplet_ip"
  curl -f http://$droplet_ip:8080/health || echo "Health check failed"
done

# Check load balancer configuration
doctl compute load-balancer get $LB_ID
```

### Debug Commands

```bash
# View droplet console output
doctl compute droplet get $DROPLET_ID --format ID,Name,Status,PublicIPv4

# Check cloud-init logs
doctl compute ssh $DROPLET_ID --ssh-command "sudo cat /var/log/cloud-init-output.log"

# Monitor resource usage
doctl monitoring metrics droplet cpu \
  --start $(date -d '1 hour ago' --iso-8601) \
  --end $(date --iso-8601) \
  $DROPLET_ID
```

### Log Locations

```bash
# Droplet initialization logs
/var/log/orbit-droplet-init.log
/var/log/orbit-gpu-droplet-init.log

# Application logs
/opt/orbit-rs/logs/
/var/log/orbit-rs/

# System logs
/var/log/syslog
/var/log/cloud-init.log
```

## Best Practices

### 1. Security

- **Network Security**
  - Use VPC for network isolation
  - Restrict firewall rules to necessary ports only
  - Disable root SSH login
  - Use SSH keys instead of passwords
  - Keep systems updated

```bash
# Security hardening script
cat > harden.sh << 'EOF'
#!/bin/bash
# Disable root login
sed -i 's/PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config

# Disable password authentication
sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config

# Configure fail2ban
systemctl enable fail2ban
systemctl start fail2ban

# Update system
apt update && apt upgrade -y
EOF
```

- **Secrets Management**
  - Use environment variables for sensitive data
  - Consider external secret management (HashiCorp Vault)
  - Rotate credentials regularly

### 2. High Availability

```yaml
# Multi-region deployment
regions:
  primary: "nyc3"
  secondary: "sfo3"
  
load_balancer:
  algorithm: "round_robin"
  health_check:
    path: "/health"
    interval: 10
    timeout: 5
    unhealthy_threshold: 3

backup:
  enabled: true
  schedule: "0 2 * * *"  # Daily at 2 AM
  retention: "30d"
```

### 3. Performance Optimization

```bash
# Network optimization
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_congestion_control = bbr' >> /etc/sysctl.conf

# GPU optimization (for GPU droplets)
nvidia-smi -pm 1  # Enable persistence mode
nvidia-smi -ac 1215,1410  # Set memory and graphics clocks
```

### 4. Monitoring Strategy

```yaml
monitoring:
  metrics:
    - cpu_usage
    - memory_usage  
    - disk_usage
    - network_io
    - gpu_usage (for GPU droplets)
    - application_metrics
    
  alerts:
    - name: "high_cpu"
      condition: "cpu > 80%"
      duration: "5m"
    - name: "gpu_temp"
      condition: "gpu_temp > 85°C"
      duration: "2m"
      
  dashboards:
    - "infrastructure_overview"
    - "application_performance" 
    - "gpu_metrics"
```

### 5. Backup and Recovery

```bash
# Automated backup script
cat > backup.sh << 'EOF'
#!/bin/bash
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="orbit-rs-backup-${DATE}"

# Create snapshots of droplets
for droplet in $(doctl compute droplet list --tag-name orbit-rs --format ID --no-header); do
  doctl compute droplet-action snapshot $droplet --snapshot-name "${BACKUP_NAME}-droplet-${droplet}"
done

# Backup Spaces data to different region
s3cmd sync s3://${DO_SPACES_NAME}/ s3://${DO_SPACES_BACKUP_NAME}/backups/${DATE}/
EOF
```

### 6. CI/CD Integration

```yaml
# .github/workflows/deploy-production.yml
name: Deploy to Production
on:
  push:
    branches: [main]
    
env:
  DO_API_TOKEN: ${{ secrets.DO_API_TOKEN }}
  DO_SPACES_ACCESS_KEY: ${{ secrets.DO_SPACES_ACCESS_KEY }}
  DO_SPACES_SECRET_KEY: ${{ secrets.DO_SPACES_SECRET_KEY }}

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Deploy to Digital Ocean
        uses: ./.github/actions/digitalocean-deploy
        with:
          environment: production
          enable_gpu: true
          droplet_count: 5
```

---

## Next Steps

After successful deployment:

1. **Configure monitoring and alerting**
2. **Set up automated backups**
3. **Implement auto-scaling policies**
4. **Configure SSL/TLS certificates**
5. **Set up log aggregation**
6. **Perform load testing**
7. **Document runbooks for operations**

## Support

- **Digital Ocean Documentation**: <https://docs.digitalocean.com/>
- **Orbit-RS Documentation**: [Link to main docs]
- **Community Support**: [Link to community forum/Discord]
- **Professional Support**: [Contact information]

For issues specific to Digital Ocean deployment, please check the [troubleshooting section](#troubleshooting) or create an issue in the repository.
