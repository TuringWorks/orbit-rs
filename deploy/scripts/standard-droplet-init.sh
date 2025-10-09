#!/bin/bash
# Standard Digital Ocean Droplet Initialization Script
# This script sets up a standard compute droplet for Orbit-RS deployment

set -euo pipefail

# Enable logging
exec > >(tee /var/log/orbit-droplet-init.log)
exec 2>&1

echo "=== Orbit-RS Standard Droplet Initialization Started ==="
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
    ntp

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
mkdir -p /opt/orbit-rs/{config,data,logs,scripts,backups}
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

# Configure system limits
echo "=== Configuring system limits ==="
cat >> /etc/security/limits.conf << 'EOF'
# Orbit-RS system limits
orbit soft nofile 65536
orbit hard nofile 65536
orbit soft nproc 32768
orbit hard nproc 32768
EOF

# Configure sysctl for performance
echo "=== Configuring sysctl parameters ==="
cat > /etc/sysctl.d/99-orbit-rs.conf << 'EOF'
# Network performance tuning for Orbit-RS
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728
net.core.netdev_max_backlog = 5000
net.ipv4.tcp_congestion_control = bbr

# File system performance
vm.swappiness = 10
vm.dirty_ratio = 15
vm.dirty_background_ratio = 5

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

sysctl -p /etc/sysctl.d/99-orbit-rs.conf

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

# Create health check script
echo "=== Creating health check scripts ==="
cat > /opt/orbit-rs/scripts/health-check.sh << 'EOF'
#!/bin/bash
# Health check script for Orbit-RS

set -e

# Check if Docker is running
if ! systemctl is-active --quiet docker; then
    echo "ERROR: Docker is not running"
    exit 1
fi

# Check if Orbit-RS containers are running
if ! docker ps | grep -q orbit-server; then
    echo "WARNING: orbit-server container not running"
    # Don't exit here as this might be during deployment
fi

# Check disk space
DISK_USAGE=$(df /opt/orbit-rs | tail -1 | awk '{print $5}' | sed 's/%//')
if [ $DISK_USAGE -gt 90 ]; then
    echo "ERROR: Disk usage is ${DISK_USAGE}%"
    exit 1
fi

# Check memory usage
MEM_USAGE=$(free | grep Mem | awk '{printf "%.0f", $3/$2 * 100.0}')
if [ $MEM_USAGE -gt 95 ]; then
    echo "ERROR: Memory usage is ${MEM_USAGE}%"
    exit 1
fi

echo "OK: Health check passed"
EOF

chmod +x /opt/orbit-rs/scripts/health-check.sh

# Create backup script
cat > /opt/orbit-rs/scripts/backup.sh << 'EOF'
#!/bin/bash
# Backup script for Orbit-RS data

set -e

BACKUP_DIR="/opt/orbit-rs/backups"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_NAME="orbit-rs-backup-${DATE}"

echo "Starting backup: ${BACKUP_NAME}"

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

# Create tarball
cd "${BACKUP_DIR}"
tar czf "${BACKUP_NAME}.tar.gz" "${BACKUP_NAME}"
rm -rf "${BACKUP_NAME}"

# Keep only last 7 backups
ls -t "${BACKUP_DIR}"/orbit-rs-backup-*.tar.gz | tail -n +8 | xargs -r rm

echo "Backup completed: ${BACKUP_DIR}/${BACKUP_NAME}.tar.gz"
EOF

chmod +x /opt/orbit-rs/scripts/backup.sh

# Create system maintenance script
cat > /opt/orbit-rs/scripts/maintenance.sh << 'EOF'
#!/bin/bash
# System maintenance script for Orbit-RS droplet

set -e

echo "=== Starting system maintenance ==="

# Update packages
apt-get update
apt-get upgrade -y
apt-get autoremove -y
apt-get autoclean

# Clean Docker resources
docker system prune -f
docker volume prune -f

# Clean log files older than 30 days
find /var/log -name "*.log" -type f -mtime +30 -delete
find /opt/orbit-rs/logs -name "*.log" -type f -mtime +30 -delete

# Clean temporary files
find /tmp -type f -atime +7 -delete
find /var/tmp -type f -atime +7 -delete

# Update system time
ntpdate -s time.nist.gov

echo "=== System maintenance completed ==="
EOF

chmod +x /opt/orbit-rs/scripts/maintenance.sh

# Create cron jobs
echo "=== Setting up cron jobs ==="
cat > /tmp/orbit-crontab << 'EOF'
# Orbit-RS maintenance tasks
0 2 * * 0 /opt/orbit-rs/scripts/maintenance.sh >> /var/log/orbit-rs/maintenance.log 2>&1
0 3 * * * /opt/orbit-rs/scripts/backup.sh >> /var/log/orbit-rs/backup.log 2>&1
*/5 * * * * /opt/orbit-rs/scripts/health-check.sh >> /var/log/orbit-rs/health-check.log 2>&1
EOF

crontab -u orbit /tmp/orbit-crontab
rm /tmp/orbit-crontab

# Create systemd service for Orbit-RS
echo "=== Creating Orbit-RS systemd service ==="
cat > /etc/systemd/system/orbit-rs.service << 'EOF'
[Unit]
Description=Orbit-RS Server
After=docker.service
Requires=docker.service
StartLimitIntervalSec=0

[Service]
Type=oneshot
RemainAfterExit=yes
User=orbit
Group=orbit
WorkingDirectory=/opt/orbit-rs
ExecStart=/bin/true
ExecStop=/opt/orbit-rs/scripts/stop-orbit.sh
TimeoutStartSec=300
TimeoutStopSec=30
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# Create stop script
cat > /opt/orbit-rs/scripts/stop-orbit.sh << 'EOF'
#!/bin/bash
# Stop Orbit-RS containers gracefully

echo "Stopping Orbit-RS containers..."

# Stop containers in order
docker stop orbit-operator 2>/dev/null || true
docker stop orbit-client 2>/dev/null || true
docker stop orbit-server 2>/dev/null || true

echo "Orbit-RS containers stopped"
EOF

chmod +x /opt/orbit-rs/scripts/stop-orbit.sh
chown orbit:orbit /opt/orbit-rs/scripts/stop-orbit.sh

systemctl daemon-reload
systemctl enable orbit-rs

# Create deployment info file
echo "=== Creating deployment info ==="
cat > /opt/orbit-rs/deployment-info.json << EOF
{
  "deployment_type": "digital_ocean_droplet",
  "droplet_type": "standard",
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

# Create ready file to signal completion
echo "=== Droplet initialization completed ==="
echo "$(date -Iseconds)" > /opt/orbit-rs/.initialized

echo "=== Orbit-RS Standard Droplet Initialization Completed ==="
echo "Timestamp: $(date)"
echo "Droplet is ready for deployment"

# Reboot to ensure all changes take effect
echo "Scheduling reboot in 1 minute..."
shutdown -r +1 "Rebooting after initialization"