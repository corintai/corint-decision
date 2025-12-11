# CORINT Server Quick Start & Deployment Guide

**Quick Start and Operations Guide** - Step-by-step instructions for running and deploying CORINT Decision Engine HTTP Server.

> For technical documentation, architecture details, and API reference, see [README.md](README.md).

## Quick Start

### Local Development

#### 1. Run with Default Configuration

```bash
# Navigate to project root directory
cd /path/to/corint-decision
cargo run -p corint-server
```

Server will start on `http://127.0.0.1:8080`.

#### 2. Run with Custom Configuration

Create `config/server.yaml`:

```yaml
host: "127.0.0.1"
port: 8080
rules_dir: "examples/pipelines"
enable_metrics: true
enable_tracing: true
log_level: "info"
```

Then start:

```bash
cargo run -p corint-server
```

#### 3. Run with Environment Variables

```bash
CORINT_PORT=9090 cargo run -p corint-server
```

## Testing the API

### Using cURL

#### Health Check

```bash
curl http://localhost:8080/health
```

#### Make a Decision

```bash
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_001",
      "device_id": "device_001",
      "ip_address": "203.0.113.1",
      "event.type": "transaction",
      "event.user_id": "user_001",
      "event.device_id": "device_001",
      "event.ip_address": "203.0.113.1",
      "event.event_type": "transaction"
    }
  }'
```

### Using Test Scripts

#### Bash Script

```bash
cd crates/corint-server/examples
./test_api.sh
```

#### Python Script

```bash
cd crates/corint-server/examples
python3 test_api.py
```

## Production Deployment

### Prerequisites

- Linux server (Ubuntu 20.04+ / CentOS 7+ / Debian 10+)
- Rust toolchain (for compilation, or use pre-compiled binary)
- System administrator privileges (sudo)
- Firewall configuration permissions

### Deployment Steps

#### Step 1: Build Release Version

On your development machine:

```bash
# Navigate to project root directory
cd /path/to/corint-decision

# Build Release version
cargo build --release -p corint-server

# Check build result
ls -lh target/release/corint-server
```

The compiled binary is located at: `target/release/corint-server` (approximately 7-8 MB)

#### Step 2: Prepare Deployment Files

Create deployment directory structure:

```bash
# Create deployment package in project root
mkdir -p deploy/corint-server/{bin,config,rules,logs}

# Copy binary file
cp target/release/corint-server deploy/corint-server/bin/

# Copy config file
cp config/server.yaml deploy/corint-server/config/

# Copy rule files
cp -r examples/pipelines deploy/corint-server/

# Copy data source configs (if needed)
cp -r examples/configs deploy/corint-server/config/

# Create necessary directories
mkdir -p deploy/corint-server/logs
```

#### Step 3: Transfer to Server

Use `scp` or `rsync` to transfer files:

```bash
# Using scp
scp -r deploy/corint-server user@your-server:/opt/

# Or using rsync (recommended, supports resume)
rsync -avz --progress deploy/corint-server/ user@your-server:/opt/corint-server/
```

#### Step 4: Server Configuration

##### 4.1 Create System User

```bash
# SSH to server
ssh user@your-server

# Create dedicated user (optional but recommended)
sudo useradd -r -s /bin/false -d /opt/corint-server corint
sudo chown -R corint:corint /opt/corint-server
```

##### 4.2 Configure Server Settings

Edit `/opt/corint-server/config/server.yaml`:

```yaml
# Production environment configuration
host: "0.0.0.0"          # Listen on all interfaces (use with firewall)
port: 8080
rules_dir: "/opt/corint-server/rules"
enable_metrics: true
enable_tracing: true
log_level: "info"        # Use info for production, avoid excessive logs
```

##### 4.3 Set Environment Variables (Optional)

Create `/opt/corint-server/.env`:

```bash
# Log level
RUST_LOG=info,corint_server=info,corint_runtime=warn

# If using Supabase connection
# DATABASE_URL=postgresql://...
```

#### Step 5: Create Systemd Service

Create service file `/etc/systemd/system/corint-server.service`:

```ini
[Unit]
Description=CORINT Decision Engine Server
Documentation=https://github.com/your-org/corint-decision
After=network.target

[Service]
Type=simple
User=corint
Group=corint
WorkingDirectory=/opt/corint-server
Environment="RUST_LOG=info"
Environment="CORINT_HOST=0.0.0.0"
Environment="CORINT_PORT=8080"
Environment="CORINT_RULES_DIR=/opt/corint-server/rules"

# Executable path
ExecStart=/opt/corint-server/bin/corint-server

# Restart policy
Restart=always
RestartSec=10

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/corint-server/logs

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=corint-server

[Install]
WantedBy=multi-user.target
```

##### 5.1 Enable and Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable corint-server

# Start service
sudo systemctl start corint-server

# Check status
sudo systemctl status corint-server

# View logs
sudo journalctl -u corint-server -f
```

#### Step 6: Configure Firewall

```bash
# Ubuntu/Debian (ufw)
sudo ufw allow 8080/tcp
sudo ufw reload

# CentOS/RHEL (firewalld)
sudo firewall-cmd --permanent --add-port=8080/tcp
sudo firewall-cmd --reload

# Or using iptables
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
sudo iptables-save
```

#### Step 7: Configure Nginx Reverse Proxy (Recommended)

##### 7.1 Install Nginx

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install nginx

# CentOS/RHEL
sudo yum install nginx
```

##### 7.2 Create Nginx Configuration

Create `/etc/nginx/sites-available/corint-server`:

```nginx
upstream corint_backend {
    server 127.0.0.1:8080;
    keepalive 32;
}

server {
    listen 80;
    server_name api.your-domain.com;  # Replace with your domain

    # Logging
    access_log /var/log/nginx/corint-access.log;
    error_log /var/log/nginx/corint-error.log;

    # Client request size limit
    client_max_body_size 10M;

    # Timeout settings
    proxy_connect_timeout 60s;
    proxy_send_timeout 60s;
    proxy_read_timeout 60s;

    # Health check endpoint (direct access, no proxy)
    location /health {
        proxy_pass http://corint_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Health check doesn't log access
        access_log off;
    }

    # API endpoints
    location /v1/ {
        proxy_pass http://corint_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Connection reuse
        proxy_http_version 1.1;
        proxy_set_header Connection "";
    }

    # Root path redirects to health check
    location = / {
        return 301 /health;
    }
}
```

##### 7.3 Enable Configuration

```bash
# Ubuntu/Debian
sudo ln -s /etc/nginx/sites-available/corint-server /etc/nginx/sites-enabled/
sudo nginx -t  # Test configuration
sudo systemctl reload nginx

# CentOS/RHEL
sudo cp /etc/nginx/sites-available/corint-server /etc/nginx/conf.d/
sudo nginx -t
sudo systemctl reload nginx
```

#### Step 8: Configure HTTPS (Optional but Recommended)

Use Let's Encrypt free SSL certificate:

```bash
# Install certbot
sudo apt-get install certbot python3-certbot-nginx  # Ubuntu/Debian
sudo yum install certbot python3-certbot-nginx     # CentOS/RHEL

# Get certificate
sudo certbot --nginx -d api.your-domain.com

# Test auto-renewal
sudo certbot renew --dry-run
```

#### Step 9: Configure Log Rotation

Create `/etc/logrotate.d/corint-server`:

```
/opt/corint-server/logs/*.log {
    daily
    missingok
    rotate 14
    compress
    delaycompress
    notifempty
    create 0640 corint corint
    sharedscripts
    postrotate
        systemctl reload corint-server > /dev/null 2>&1 || true
    endscript
}
```

#### Step 10: Monitoring and Health Checks

##### 10.1 Set Up Health Check Script

Create `/opt/corint-server/scripts/healthcheck.sh`:

```bash
#!/bin/bash
HEALTH_URL="http://localhost:8080/health"
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" $HEALTH_URL)

if [ "$RESPONSE" = "200" ]; then
    exit 0
else
    echo "Health check failed: HTTP $RESPONSE"
    exit 1
fi
```

```bash
chmod +x /opt/corint-server/scripts/healthcheck.sh
```

##### 10.2 Configure Cron Monitoring

```bash
# Edit crontab
sudo crontab -e

# Add check every 5 minutes
*/5 * * * * /opt/corint-server/scripts/healthcheck.sh || systemctl restart corint-server
```

## Quick Deployment Using Script

Use the automated deployment script for faster deployment:

```bash
# Build release version
cargo build --release -p corint-server

# Deploy to server
./deploy.sh user@your-server
```

The script automatically:
- Builds Release version
- Prepares deployment files
- Transfers to server
- Configures Systemd service
- Starts service
- Verifies deployment

## Common Management Commands

### Service Management

```bash
# Start service
sudo systemctl start corint-server

# Stop service
sudo systemctl stop corint-server

# Restart service
sudo systemctl restart corint-server

# View status
sudo systemctl status corint-server

# View logs
sudo journalctl -u corint-server -f
sudo journalctl -u corint-server --since "1 hour ago"
```

### Updating Deployment

```bash
# 1. Stop service
sudo systemctl stop corint-server

# 2. Backup current version
sudo cp /opt/corint-server/bin/corint-server /opt/corint-server/bin/corint-server.backup

# 3. Upload new version
scp target/release/corint-server user@server:/opt/corint-server/bin/

# 4. Set permissions
sudo chown corint:corint /opt/corint-server/bin/corint-server
sudo chmod +x /opt/corint-server/bin/corint-server

# 5. Start service
sudo systemctl start corint-server

# 6. Verify
curl http://localhost:8080/health
```

### Viewing Logs

```bash
# Systemd logs
sudo journalctl -u corint-server -n 100 -f

# Nginx access logs
sudo tail -f /var/log/nginx/corint-access.log

# Nginx error logs
sudo tail -f /var/log/nginx/corint-error.log

# Application logs (if file logging configured)
tail -f /opt/corint-server/logs/*.log
```

## Security Recommendations

### 1. Firewall Configuration

Only open necessary ports:

```bash
# Only allow Nginx ports (80/443)
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# Do not allow direct access to 8080 (local only)
# Port 8080 should not be exposed externally
```

### 2. Access Restrictions

Configure IP whitelist in Nginx (if needed):

```nginx
location /v1/ {
    allow 192.168.1.0/24;  # Allow internal network
    allow 10.0.0.0/8;      # Allow private network
    deny all;              # Deny all others

    proxy_pass http://corint_backend;
    # ...
}
```

### 3. API Key Authentication (Future Implementation)

Currently API has no authentication. Recommendations:
- Use Nginx HTTP Basic Auth
- Or implement API Key authentication
- Or use OAuth2/JWT

### 4. Regular Updates

```bash
# Update system packages
sudo apt-get update && sudo apt-get upgrade  # Ubuntu/Debian
sudo yum update                             # CentOS/RHEL

# Update Rust toolchain (if recompiling needed)
rustup update
```

## Performance Optimization

### 1. Adjust Connection Limits

Edit systemd service file to increase file descriptor limit:

```ini
LimitNOFILE=65536
```

### 2. Nginx Connection Pool

```nginx
upstream corint_backend {
    server 127.0.0.1:8080;
    keepalive 32;  # Keep-alive connections
}
```

### 3. Enable Gzip Compression

Add to Nginx configuration:

```nginx
gzip on;
gzip_types application/json text/plain;
gzip_min_length 1000;
```

## Troubleshooting

### Issue 1: Server Won't Start

**Symptoms:** Server fails to start

**Troubleshooting Steps:**

1. Check if port is in use:

```bash
lsof -i :8080
```

2. Check if rules directory exists:

```bash
ls -la examples/pipelines
```

3. View detailed logs:

```bash
RUST_LOG=debug cargo run -p corint-server
```

### Issue 2: Rules Not Loading

**Symptoms:** Decision returns "No rules loaded" or uses default action

**Troubleshooting Steps:**

1. Ensure rule files have `.yaml` or `.yml` extension
2. Check rule file syntax is correct
3. View server startup logs for rule loading information:

```
INFO corint_server: Loading rules from directory: "examples/pipelines"
INFO corint_server: Loading rule file: "examples/pipelines/simple_rule.yaml"
```

### Issue 3: Feature Calculation Fails

**Symptoms:** Decision returns error or feature value is 0/null

**Troubleshooting Steps:**

1. Verify data source connection is working:

```bash
psql "postgresql://postgres.PROJECT_REF:PASSWORD@HOST:PORT/postgres" -c "SELECT 1"
```

2. Check data source configuration `examples/configs/datasources/supabase_events.yaml`

3. View SQL queries and errors in logs:

```bash
RUST_LOG=corint_runtime::datasource=debug cargo run -p corint-server
```

4. Verify test data exists in database:

```sql
SELECT COUNT(*) FROM events WHERE user_id = 'user_001';
```

### Issue 4: API Returns 500 Error

**Symptoms:** API returns internal server error

**Troubleshooting Steps:**

1. View error stack trace in server logs
2. Check request body format is correct
3. Ensure `event_data` contains all required fields
4. Use `curl -v` to view detailed response

## Deployment Checklist

- [ ] Release version compiled successfully
- [ ] Files transferred to server
- [ ] System user created
- [ ] Configuration files correct
- [ ] Systemd service configured
- [ ] Service started successfully
- [ ] Firewall configured
- [ ] Nginx reverse proxy configured
- [ ] HTTPS certificate configured (if needed)
- [ ] Log rotation configured
- [ ] Health check script
- [ ] Monitoring configured
- [ ] Backup strategy

## Quick Deployment Script

Create `deploy.sh` script for automated deployment:

```bash
#!/bin/bash
set -e

SERVER="user@your-server"
APP_DIR="/opt/corint-server"

echo "Building release..."
cargo build --release -p corint-server

echo "Creating deployment package..."
mkdir -p deploy
cp target/release/corint-server deploy/
cp config/server.yaml deploy/

echo "Uploading to server..."
rsync -avz --progress deploy/ $SERVER:$APP_DIR/

echo "Restarting service..."
ssh $SERVER "sudo systemctl restart corint-server"

echo "Checking health..."
sleep 2
curl http://your-server:8080/health

echo "Deployment complete!"
```

Usage:

```bash
chmod +x deploy.sh
./deploy.sh
```

## Production Environment Best Practices

1. **Use Process Manager**: Systemd ensures automatic service restart
2. **Reverse Proxy**: Nginx provides load balancing and SSL termination
3. **Log Management**: Centralized log collection and analysis
4. **Monitoring Alerts**: Set up health check alerts
5. **Regular Backups**: Backup configuration and rule files
6. **Version Control**: Use Git to manage configuration changes
7. **Staged Deployment**: Validate in test environment first
8. **Performance Testing**: Regular stress testing

## Related Documentation

- [Technical Documentation & API Reference](README.md) - Architecture, implementation details, and API documentation
- [Development Guide](../../docs/DEV_GUIDE.md) - Complete development guide

---

**After deployment, your API will be accessible at:**

- HTTP: `http://your-server/v1/decide`
- HTTPS: `https://api.your-domain.com/v1/decide`
- Health Check: `http://your-server/health`
