#!/bin/bash
# CORINT Server Quick Deployment Script
# Usage: ./deploy.sh user@server

set -e

# Color output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

# Check arguments
if [ -z "$1" ]; then
    echo -e "${RED}Error: Please provide server address${NC}"
    echo "Usage: ./deploy.sh user@server"
    echo "Example: ./deploy.sh ubuntu@192.168.1.100"
    exit 1
fi

SERVER=$1
APP_DIR="/opt/corint-server"
REMOTE_USER=$(echo $SERVER | cut -d@ -f1)

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}CORINT Server Deployment Script${NC}"
echo -e "${BLUE}========================================${NC}"
echo "Server: $SERVER"
echo "App Directory: $APP_DIR"
echo ""

# Step 1: Build Release version
echo -e "${GREEN}[1/6] Building Release version...${NC}"
cargo build --release -p corint-server
if [ $? -ne 0 ]; then
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi
echo "✓ Build completed"
echo ""

# Step 2: Prepare deployment files
echo -e "${GREEN}[2/6] Preparing deployment files...${NC}"
rm -rf deploy
mkdir -p deploy/{bin,config,rules,scripts}

# Copy binary file
cp target/release/corint-server deploy/bin/
chmod +x deploy/bin/corint-server

# Copy config file
if [ -f config/server.yaml ]; then
    cp config/server.yaml deploy/config/
else
    echo "Warning: config/server.yaml not found, will use default config"
fi

# Copy rule files
if [ -d examples/pipelines ]; then
    cp -r examples/pipelines deploy/
else
    echo "Warning: examples/pipelines directory not found"
fi

# Create health check script
cat > deploy/scripts/healthcheck.sh << 'EOF'
#!/bin/bash
HEALTH_URL="http://localhost:8080/health"
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" $HEALTH_URL)

if [ "$RESPONSE" = "200" ]; then
    exit 0
else
    echo "Health check failed: HTTP $RESPONSE"
    exit 1
fi
EOF
chmod +x deploy/scripts/healthcheck.sh

echo "✓ Deployment files prepared"
echo ""

# Step 3: Transfer files to server
echo -e "${GREEN}[3/6] Transferring files to server...${NC}"
echo "Uploading files to $SERVER:$APP_DIR ..."

# Create remote directory
ssh $SERVER "sudo mkdir -p $APP_DIR && sudo chown $REMOTE_USER:$REMOTE_USER $APP_DIR" || {
    echo -e "${RED}Failed to create remote directory, please check SSH connection and permissions${NC}"
    exit 1
}

# Transfer files using rsync
rsync -avz --progress deploy/ $SERVER:$APP_DIR/ || {
    echo -e "${RED}File transfer failed!${NC}"
    exit 1
}

echo "✓ File transfer completed"
echo ""

# Step 4: Configure server
echo -e "${GREEN}[4/6] Configuring server...${NC}"

# Create systemd service file
ssh $SERVER "sudo tee /etc/systemd/system/corint-server.service > /dev/null" << EOF
[Unit]
Description=CORINT Decision Engine Server
After=network.target

[Service]
Type=simple
User=$REMOTE_USER
Group=$REMOTE_USER
WorkingDirectory=$APP_DIR
Environment="RUST_LOG=info"
Environment="CORINT_HOST=0.0.0.0"
Environment="CORINT_PORT=8080"
Environment="CORINT_RULES_DIR=$APP_DIR/rules"
ExecStart=$APP_DIR/bin/corint-server
Restart=always
RestartSec=10
LimitNOFILE=65536
StandardOutput=journal
StandardError=journal
SyslogIdentifier=corint-server

[Install]
WantedBy=multi-user.target
EOF

# Set permissions
ssh $SERVER "sudo chown -R $REMOTE_USER:$REMOTE_USER $APP_DIR && sudo chmod +x $APP_DIR/bin/corint-server"

echo "✓ Server configuration completed"
echo ""

# Step 5: Start service
echo -e "${GREEN}[5/6] Starting service...${NC}"
ssh $SERVER "sudo systemctl daemon-reload && sudo systemctl enable corint-server && sudo systemctl restart corint-server" || {
    echo -e "${RED}Service startup failed!${NC}"
    echo "View logs: ssh $SERVER 'sudo journalctl -u corint-server -n 50'"
    exit 1
}

# Wait for service to start
sleep 3

echo "✓ Service started"
echo ""

# Step 6: Verify deployment
echo -e "${GREEN}[6/6] Verifying deployment...${NC}"

# Check service status
STATUS=$(ssh $SERVER "sudo systemctl is-active corint-server")
if [ "$STATUS" != "active" ]; then
    echo -e "${RED}Service is not running! Status: $STATUS${NC}"
    echo "View logs: ssh $SERVER 'sudo journalctl -u corint-server -n 50'"
    exit 1
fi

# Health check
HEALTH=$(ssh $SERVER "curl -s -o /dev/null -w '%{http_code}' http://localhost:8080/health" || echo "000")
if [ "$HEALTH" = "200" ]; then
    echo -e "${GREEN}✓ Health check passed${NC}"
else
    echo -e "${RED}Health check failed (HTTP $HEALTH)${NC}"
    echo "View logs: ssh $SERVER 'sudo journalctl -u corint-server -n 50'"
    exit 1
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Deployment successful!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Service Information:"
echo "  Status: $(ssh $SERVER 'sudo systemctl is-active corint-server')"
echo "  Health Check: http://$SERVER:8080/health"
echo "  API Endpoint: http://$SERVER:8080/v1/decide"
echo ""
echo "Common Commands:"
echo "  View status: ssh $SERVER 'sudo systemctl status corint-server'"
echo "  View logs: ssh $SERVER 'sudo journalctl -u corint-server -f'"
echo "  Restart service: ssh $SERVER 'sudo systemctl restart corint-server'"
echo ""

