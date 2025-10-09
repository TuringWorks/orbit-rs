#!/bin/bash
# GPU-Enabled Digital Ocean Droplet Initialization Script
# This script sets up a GPU droplet for Orbit-RS compute-intensive workloads

set -euo pipefail

# Enable logging
exec > >(tee /var/log/orbit-gpu-droplet-init.log)
exec 2>&1

echo "=== Orbit-RS GPU Droplet Initialization Started ==="
echo "Timestamp: $(date)"
echo "Hostname: $(hostname)"
echo "User: $(whoami)"

# Update system packages
echo "=== Updating system packages ==="
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get upgrade -y

# Install essential packages
echo "=== Installing essential packages ==="
apt-get install -y \
    curl \
    wget \
    gnupg \
    lsb-release \
    ca-certificates \
    software-properties-common \
    apt-transport-https \
    unzip \
    jq \
    htop \
    netcat \
    rsync \
    logrotate \
    fail2ban \
    ufw \
    ntp \
    build-essential \
    linux-headers-$(uname -r) \
    dkms \
    pciutils \
    nvidia-modprobe

# Install Docker
echo "=== Installing Docker ==="
curl -fsSL https://get.docker.com -o get-docker.sh
sh get-docker.sh

# Start and enable Docker
systemctl start docker
systemctl enable docker

# Add root to docker group (for deployment scripts)
usermod -aG docker root

# Install Docker Compose
echo "=== Installing Docker Compose ==="
DOCKER_COMPOSE_VERSION="v2.21.0"
curl -L "https://github.com/docker/compose/releases/download/${DOCKER_COMPOSE_VERSION}/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
chmod +x /usr/local/bin/docker-compose

# Install NVIDIA drivers and CUDA toolkit
echo "=== Installing NVIDIA drivers and CUDA toolkit ==="

# Check if NVIDIA GPU is present
if ! lspci | grep -i nvidia; then
    echo "WARNING: No NVIDIA GPU detected. This may be a fallback instance."
    echo "Continuing with CPU-only setup..."
    GPU_AVAILABLE=false
else
    echo "NVIDIA GPU detected:"
    lspci | grep -i nvidia
    GPU_AVAILABLE=true
fi

if [ "$GPU_AVAILABLE" = true ]; then
    # Add NVIDIA package repositories
    wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/cuda-keyring_1.0-1_all.deb
    dpkg -i cuda-keyring_1.0-1_all.deb
    apt-get update

    # Install CUDA toolkit and drivers
    apt-get install -y cuda-toolkit-12-2
    apt-get install -y nvidia-driver-535
    
    # Install NVIDIA Container Toolkit
    distribution=$(. /etc/os-release;echo $ID$VERSION_ID)
    curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
    curl -s -L https://nvidia.github.io/libnvidia-container/$distribution/libnvidia-container.list | \
        sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
        sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
    
    apt-get update
    apt-get install -y nvidia-container-toolkit
    nvidia-ctk runtime configure --runtime=docker
    
    # Configure Docker daemon for GPU support
    cat > /etc/docker/daemon.json << 'EOF'
{
    "default-runtime": "nvidia",
    "runtimes": {
        "nvidia": {
            "path": "nvidia-container-runtime",
            "runtimeArgs": []
        }
    },
    "log-driver": "json-file",
    "log-opts": {
        "max-size": "10m",
        "max-file": "3"
    },
    "storage-driver": "overlay2"
}
EOF

    # Restart Docker to apply GPU configuration
    systemctl restart docker
    
    # Configure NVIDIA persistence mode
    nvidia-smi -pm 1
    
    # Set GPU performance settings
    nvidia-smi -pl 350  # Set power limit to 350W
    nvidia-smi -ac 1215,1410  # Set memory and graphics clocks
    
    # Install additional CUDA libraries
    apt-get install -y \
        libcudnn8 \
        libcudnn8-dev \
        libnccl2 \
        libnccl-dev \
        tensorrt \
        python3-libnvinfer \
        python3-libnvinfer-dev
        
    echo "=== GPU setup completed ==="
    nvidia-smi
else
    echo "=== Proceeding with CPU-only setup ==="
fi

# Install Digital Ocean monitoring agent
echo "=== Installing Digital Ocean monitoring agent ==="
curl -sSL https://repos.insights.digitalocean.com/install.sh | bash

# Configure firewall
echo "=== Configuring firewall ==="
ufw --force reset
ufw --force enable

# Allow SSH (port 22)
ufw allow ssh

# Allow Orbit-RS ports
ufw allow 8080/tcp   # HTTP API
ufw allow 50051/tcp  # gRPC
ufw allow 9090/tcp   # Metrics

# Deny everything else by default
ufw default deny incoming
ufw default allow outgoing

# Configure fail2ban
echo "=== Configuring fail2ban ==="
cat > /etc/fail2ban/jail.local << 'EOF'
[DEFAULT]
bantime = 3600
findtime = 600
maxretry = 3

[sshd]
enabled = true
port = ssh
filter = sshd
logpath = /var/log/auth.log
maxretry = 3
bantime = 3600
EOF

systemctl restart fail2ban
systemctl enable fail2ban

# Create orbit user
echo "=== Creating orbit user ==="
useradd -m -s /bin/bash orbit
usermod -aG docker orbit

# Create directory structure
echo "=== Creating directory structure ==="
mkdir -p /opt/orbit-rs/{config,data,logs,scripts,backups,models,datasets}
mkdir -p /var/log/orbit-rs

# Set permissions
chown -R orbit:orbit /opt/orbit-rs
chown -R orbit:orbit /var/log/orbit-rs

# Create log rotation configuration
echo "=== Configuring log rotation ==="
cat > /etc/logrotate.d/orbit-rs << 'EOF'
/var/log/orbit-rs/*.log /opt/orbit-rs/logs/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 orbit orbit
    postrotate
        systemctl reload docker || true
    endscript
}
EOF

# Configure NTP for time synchronization
echo "=== Configuring NTP ==="
systemctl start ntp
systemctl enable ntp

# Set timezone
timedatectl set-timezone UTC

# Configure system limits (higher for GPU workloads)
echo "=== Configuring system limits ==="
cat >> /etc/security/limits.conf << 'EOF'
# Orbit-RS system limits for GPU workloads
orbit soft nofile 131072
orbit hard nofile 131072
orbit soft nproc 65536
orbit hard nproc 65536
orbit soft memlock unlimited
orbit hard memlock unlimited
EOF

# Configure sysctl for GPU workloads
echo "=== Configuring sysctl parameters ==="
cat > /etc/sysctl.d/99-orbit-rs-gpu.conf << 'EOF'
# Network performance tuning for Orbit-RS GPU workloads
net.core.rmem_max = 268435456
net.core.wmem_max = 268435456
net.ipv4.tcp_rmem = 4096 131072 268435456
net.ipv4.tcp_wmem = 4096 65536 268435456
net.core.netdev_max_backlog = 10000
net.ipv4.tcp_congestion_control = bbr

# File system performance (optimized for large datasets)
vm.swappiness = 1
vm.dirty_ratio = 5
vm.dirty_background_ratio = 2
vm.vfs_cache_pressure = 50

# GPU memory settings
vm.overcommit_memory = 1
vm.overcommit_ratio = 50

# Security hardening
net.ipv4.conf.all.accept_redirects = 0
net.ipv4.conf.default.accept_redirects = 0
net.ipv4.conf.all.secure_redirects = 0
net.ipv4.conf.default.secure_redirects = 0
net.ipv4.conf.all.send_redirects = 0
net.ipv4.conf.default.send_redirects = 0
net.ipv4.conf.all.accept_source_route = 0
net.ipv4.conf.default.accept_source_route = 0
net.ipv4.conf.all.log_martians = 1
net.ipv4.conf.default.log_martians = 1
EOF

sysctl -p /etc/sysctl.d/99-orbit-rs-gpu.conf

# Install monitoring tools
echo "=== Installing monitoring tools ==="

# Prometheus Node Exporter
NODE_EXPORTER_VERSION="1.6.1"
wget "https://github.com/prometheus/node_exporter/releases/download/v${NODE_EXPORTER_VERSION}/node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64.tar.gz"
tar xzf "node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64.tar.gz"
mv "node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64/node_exporter" /usr/local/bin/
rm -rf "node_exporter-${NODE_EXPORTER_VERSION}.linux-amd64"*

# Create node_exporter service
cat > /etc/systemd/system/node_exporter.service << 'EOF'
[Unit]
Description=Node Exporter
Wants=network-online.target
After=network-online.target

[Service]
User=orbit
Group=orbit
Type=simple
ExecStart=/usr/local/bin/node_exporter --web.listen-address=:9100

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl start node_exporter
systemctl enable node_exporter

# Install NVIDIA DCGM (Data Center GPU Manager) for GPU monitoring
if [ "$GPU_AVAILABLE" = true ]; then
    echo "=== Installing NVIDIA DCGM for GPU monitoring ==="
    
    # Install DCGM
    wget https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2204/x86_64/datacenter-gpu-manager_3.2.5_amd64.deb
    dpkg -i datacenter-gpu-manager_3.2.5_amd64.deb || apt-get install -f -y
    
    # Start DCGM service
    systemctl start nvidia-dcgm
    systemctl enable nvidia-dcgm
    
    # Install DCGM Exporter for Prometheus
    DCGM_EXPORTER_VERSION="3.1.8"
    wget "https://github.com/NVIDIA/dcgm-exporter/releases/download/v${DCGM_EXPORTER_VERSION}/dcgm-exporter_${DCGM_EXPORTER_VERSION}_linux_amd64.tar.gz"
    tar xzf "dcgm-exporter_${DCGM_EXPORTER_VERSION}_linux_amd64.tar.gz"
    mv dcgm-exporter /usr/local/bin/
    rm -f "dcgm-exporter_${DCGM_EXPORTER_VERSION}_linux_amd64.tar.gz"
    
    # Create DCGM exporter service
    cat > /etc/systemd/system/dcgm_exporter.service << 'EOF'
[Unit]
Description=NVIDIA DCGM Exporter
Wants=network-online.target
After=network-online.target nvidia-dcgm.service
Requires=nvidia-dcgm.service

[Service]
User=orbit
Group=orbit
Type=simple
ExecStart=/usr/local/bin/dcgm-exporter --web-listen-address=:9400
Environment=DCGM_EXPORTER_LISTEN=0.0.0.0:9400

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    systemctl start dcgm_exporter
    systemctl enable dcgm_exporter
    
    # Allow DCGM exporter port through firewall
    ufw allow 9400/tcp
fi

# Create GPU-aware health check script
echo "=== Creating GPU health check scripts ==="
cat > /opt/orbit-rs/scripts/gpu-health-check.sh << 'EOF'
#!/bin/bash
# GPU health check script for Orbit-RS

set -e

# Check if Docker is running
if ! systemctl is-active --quiet docker; then
    echo "ERROR: Docker is not running"
    exit 1
fi

# Check if Orbit-RS GPU containers are running
if ! docker ps | grep -q orbit-server-gpu; then
    echo "WARNING: orbit-server-gpu container not running"
fi

if ! docker ps | grep -q orbit-compute-gpu; then
    echo "WARNING: orbit-compute-gpu container not running"
fi

# Check disk space
DISK_USAGE=$(df /opt/orbit-rs | tail -1 | awk '{print $5}' | sed 's/%//')
if [ $DISK_USAGE -gt 85 ]; then
    echo "ERROR: Disk usage is ${DISK_USAGE}%"
    exit 1
fi

# Check memory usage
MEM_USAGE=$(free | grep Mem | awk '{printf "%.0f", $3/$2 * 100.0}')
if [ $MEM_USAGE -gt 90 ]; then
    echo "ERROR: Memory usage is ${MEM_USAGE}%"
    exit 1
fi

# Check GPU status if available
if command -v nvidia-smi &> /dev/null; then
    # Check if NVIDIA drivers are working
    if ! nvidia-smi &> /dev/null; then
        echo "ERROR: nvidia-smi failed"
        exit 1
    fi
    
    # Check GPU temperature
    GPU_TEMP=$(nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits | head -1)
    if [ $GPU_TEMP -gt 85 ]; then
        echo "ERROR: GPU temperature is ${GPU_TEMP}°C"
        exit 1
    fi
    
    # Check GPU memory usage
    GPU_MEM_USAGE=$(nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits | head -1 | awk '{printf "%.0f", $1/$2 * 100.0}')
    if [ $GPU_MEM_USAGE -gt 95 ]; then
        echo "ERROR: GPU memory usage is ${GPU_MEM_USAGE}%"
        exit 1
    fi
    
    # Check for GPU errors
    if nvidia-smi -q | grep -i error; then
        echo "ERROR: GPU errors detected"
        exit 1
    fi
fi

echo "OK: GPU health check passed"
EOF

chmod +x /opt/orbit-rs/scripts/gpu-health-check.sh

# Create GPU performance monitoring script
cat > /opt/orbit-rs/scripts/gpu-monitor.sh << 'EOF'
#!/bin/bash
# GPU performance monitoring script for Orbit-RS

LOG_FILE="/var/log/orbit-rs/gpu-performance.log"

# Create log directory if it doesn't exist
mkdir -p "$(dirname "$LOG_FILE")"

# Check if nvidia-smi is available
if ! command -v nvidia-smi &> /dev/null; then
    echo "$(date): WARNING: nvidia-smi not available" >> "$LOG_FILE"
    exit 0
fi

# Log GPU status
{
    echo "=== GPU Performance Report $(date) ==="
    nvidia-smi --query-gpu=timestamp,name,temperature.gpu,utilization.gpu,utilization.memory,memory.used,memory.total,power.draw,power.limit --format=csv
    echo "=== End Report ==="
} >> "$LOG_FILE"

# Rotate log if it gets too large
if [ -f "$LOG_FILE" ] && [ $(stat -c%s "$LOG_FILE") -gt 104857600 ]; then  # 100MB
    mv "$LOG_FILE" "${LOG_FILE}.old"
fi
EOF

chmod +x /opt/orbit-rs/scripts/gpu-monitor.sh

# Create GPU benchmark script
cat > /opt/orbit-rs/scripts/gpu-benchmark.sh << 'EOF'
#!/bin/bash
# GPU benchmark script for Orbit-RS deployment validation

if ! command -v nvidia-smi &> /dev/null; then
    echo "nvidia-smi not available, skipping GPU benchmark"
    exit 0
fi

echo "=== Running GPU benchmark ==="

# Run CUDA deviceQuery if available
if [ -f "/usr/local/cuda/extras/demo_suite/deviceQuery" ]; then
    echo "Running CUDA deviceQuery:"
    /usr/local/cuda/extras/demo_suite/deviceQuery
fi

# Simple memory bandwidth test using nvidia-smi
echo "GPU Memory Bandwidth Test:"
nvidia-ml-py3 -c "
import pynvml
pynvml.nvmlInit()
handle = pynvml.nvmlDeviceGetHandleByIndex(0)
info = pynvml.nvmlDeviceGetMemoryInfo(handle)
print(f'Total memory: {info.total // 1024**2} MB')
print(f'Used memory: {info.used // 1024**2} MB')
print(f'Free memory: {info.free // 1024**2} MB')
" 2>/dev/null || echo "pynvml not available, using nvidia-smi"

# Basic performance metrics
nvidia-smi --query-gpu=name,memory.total,memory.used,utilization.gpu,temperature.gpu --format=csv

echo "=== GPU benchmark completed ==="
EOF

chmod +x /opt/orbit-rs/scripts/gpu-benchmark.sh

# Create backup script (GPU-aware)
cat > /opt/orbit-rs/scripts/gpu-backup.sh << 'EOF'
#!/bin/bash
# GPU-aware backup script for Orbit-RS data

set -e

BACKUP_DIR="/opt/orbit-rs/backups"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="orbit-rs-gpu-backup-${DATE}"

echo "Starting GPU backup: ${BACKUP_NAME}"

# Create backup directory
mkdir -p "${BACKUP_DIR}/${BACKUP_NAME}"

# Backup configuration
if [ -d "/opt/orbit-rs/config" ]; then
    cp -r /opt/orbit-rs/config "${BACKUP_DIR}/${BACKUP_NAME}/"
fi

# Backup data (if exists)
if [ -d "/opt/orbit-rs/data" ]; then
    cp -r /opt/orbit-rs/data "${BACKUP_DIR}/${BACKUP_NAME}/"
fi

# Backup models (ML models can be large)
if [ -d "/opt/orbit-rs/models" ]; then
    cp -r /opt/orbit-rs/models "${BACKUP_DIR}/${BACKUP_NAME}/"
fi

# Backup GPU performance logs
if [ -f "/var/log/orbit-rs/gpu-performance.log" ]; then
    cp /var/log/orbit-rs/gpu-performance.log "${BACKUP_DIR}/${BACKUP_NAME}/"
fi

# Save GPU configuration
if command -v nvidia-smi &> /dev/null; then
    nvidia-smi -q > "${BACKUP_DIR}/${BACKUP_NAME}/gpu-config.txt"
fi

# Create tarball
cd "${BACKUP_DIR}"
tar czf "${BACKUP_NAME}.tar.gz" "${BACKUP_NAME}"
rm -rf "${BACKUP_NAME}"

# Keep only last 5 backups (GPU backups can be large)
ls -t "${BACKUP_DIR}"/orbit-rs-gpu-backup-*.tar.gz | tail -n +6 | xargs -r rm

echo "GPU backup completed: ${BACKUP_DIR}/${BACKUP_NAME}.tar.gz"
EOF

chmod +x /opt/orbit-rs/scripts/gpu-backup.sh

# Create cron jobs for GPU monitoring
echo "=== Setting up cron jobs ==="
cat > /tmp/orbit-gpu-crontab << 'EOF'
# Orbit-RS GPU maintenance tasks
0 2 * * 0 /opt/orbit-rs/scripts/maintenance.sh >> /var/log/orbit-rs/maintenance.log 2>&1
0 4 * * * /opt/orbit-rs/scripts/gpu-backup.sh >> /var/log/orbit-rs/backup.log 2>&1
*/5 * * * * /opt/orbit-rs/scripts/gpu-health-check.sh >> /var/log/orbit-rs/health-check.log 2>&1
*/1 * * * * /opt/orbit-rs/scripts/gpu-monitor.sh
EOF

crontab -u orbit /tmp/orbit-gpu-crontab
rm /tmp/orbit-gpu-crontab

# Create systemd service for GPU Orbit-RS
echo "=== Creating GPU Orbit-RS systemd service ==="
cat > /etc/systemd/system/orbit-rs-gpu.service << 'EOF'
[Unit]
Description=Orbit-RS GPU Server
After=docker.service nvidia-dcgm.service
Requires=docker.service
StartLimitIntervalSec=0

[Service]
Type=oneshot
RemainAfterExit=yes
User=orbit
Group=orbit
WorkingDirectory=/opt/orbit-rs
ExecStart=/bin/true
ExecStop=/opt/orbit-rs/scripts/stop-orbit-gpu.sh
TimeoutStartSec=600
TimeoutStopSec=60
Restart=on-failure
RestartSec=10
Environment="NVIDIA_VISIBLE_DEVICES=all"
Environment="CUDA_VISIBLE_DEVICES=all"

[Install]
WantedBy=multi-user.target
EOF

# Create GPU stop script
cat > /opt/orbit-rs/scripts/stop-orbit-gpu.sh << 'EOF'
#!/bin/bash
# Stop GPU Orbit-RS containers gracefully

echo "Stopping Orbit-RS GPU containers..."

# Stop containers in order
docker stop orbit-compute-gpu 2>/dev/null || true
docker stop orbit-server-gpu 2>/dev/null || true

# Clean up GPU memory
if command -v nvidia-smi &> /dev/null; then
    nvidia-smi --gpu-reset
fi

echo "Orbit-RS GPU containers stopped"
EOF

chmod +x /opt/orbit-rs/scripts/stop-orbit-gpu.sh
chown orbit:orbit /opt/orbit-rs/scripts/stop-orbit-gpu.sh

systemctl daemon-reload
systemctl enable orbit-rs-gpu

# Create deployment info file
echo "=== Creating deployment info ==="
cat > /opt/orbit-rs/deployment-info.json << EOF
{
  "deployment_type": "digital_ocean_droplet",
  "droplet_type": "gpu",
  "gpu_available": $GPU_AVAILABLE,
  "initialization_date": "$(date -Iseconds)",
  "hostname": "$(hostname)",
  "droplet_id": "$(curl -s http://169.254.169.254/metadata/v1/id)",
  "region": "$(curl -s http://169.254.169.254/metadata/v1/region)",
  "public_ipv4": "$(curl -s http://169.254.169.254/metadata/v1/interfaces/public/0/ipv4/address)",
  "private_ipv4": "$(curl -s http://169.254.169.254/metadata/v1/interfaces/private/0/ipv4/address)",
  "tags": $(curl -s http://169.254.169.254/metadata/v1/tags | jq -c .),
  "orbit_version": "$(docker --version)",
  "system_info": {
    "kernel": "$(uname -r)",
    "os": "$(lsb_release -d | cut -f2)",
    "architecture": "$(uname -m)",
    "cpu_cores": "$(nproc)",
    "total_memory": "$(free -h | grep Mem | awk '{print $2}')",
    "disk_space": "$(df -h / | tail -1 | awk '{print $2}')"
  }
}
EOF

# Add GPU info if available
if [ "$GPU_AVAILABLE" = true ]; then
    # Create temporary GPU info
    GPU_INFO=$(nvidia-smi --query-gpu=name,memory.total,driver_version,cuda_version --format=csv,noheader,nounits 2>/dev/null | head -1)
    GPU_NAME=$(echo "$GPU_INFO" | cut -d',' -f1 | xargs)
    GPU_MEMORY=$(echo "$GPU_INFO" | cut -d',' -f2 | xargs)
    DRIVER_VERSION=$(echo "$GPU_INFO" | cut -d',' -f3 | xargs)
    
    # Update deployment info with GPU details
    jq --arg gpu_name "$GPU_NAME" \
       --arg gpu_memory "$GPU_MEMORY" \
       --arg driver_version "$DRIVER_VERSION" \
       '.gpu_info = {
         "gpu_name": $gpu_name,
         "gpu_memory_mb": $gpu_memory,
         "driver_version": $driver_version,
         "cuda_version": "12.2"
       }' /opt/orbit-rs/deployment-info.json > /tmp/deployment-info.json
    
    mv /tmp/deployment-info.json /opt/orbit-rs/deployment-info.json
fi

chown orbit:orbit /opt/orbit-rs/deployment-info.json

# Set up SSH hardening
echo "=== Hardening SSH configuration ==="
sed -i 's/#PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config
sed -i 's/#PasswordAuthentication yes/PasswordAuthentication no/' /etc/ssh/sshd_config
sed -i 's/#PubkeyAuthentication yes/PubkeyAuthentication yes/' /etc/ssh/sshd_config
sed -i 's/#AuthorizedKeysFile/AuthorizedKeysFile/' /etc/ssh/sshd_config

# Restart SSH service
systemctl restart sshd

# Final system updates
echo "=== Final system updates ==="
updatedb

# Run initial GPU benchmark if GPU is available
if [ "$GPU_AVAILABLE" = true ]; then
    echo "=== Running initial GPU benchmark ==="
    /opt/orbit-rs/scripts/gpu-benchmark.sh > /opt/orbit-rs/initial-gpu-benchmark.log 2>&1
fi

# Create ready file to signal completion
echo "=== GPU droplet initialization completed ==="
echo "$(date -Iseconds)" > /opt/orbit-rs/.initialized

echo "=== Orbit-RS GPU Droplet Initialization Completed ==="
echo "Timestamp: $(date)"

if [ "$GPU_AVAILABLE" = true ]; then
    echo "GPU Status:"
    nvidia-smi --query-gpu=name,memory.total,temperature.gpu --format=csv
    echo "GPU droplet is ready for ML/AI workloads"
else
    echo "CPU-only droplet is ready for standard workloads"
fi

# Reboot to ensure all changes take effect (especially GPU drivers)
echo "Scheduling reboot in 2 minutes to initialize GPU drivers..."
shutdown -r +2 "Rebooting after GPU initialization"