#!/bin/bash

# CORINT Decision Engine Server API Test Script

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

SERVER_URL="${CORINT_SERVER_URL:-http://localhost:8080}"

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}CORINT Decision Engine API Tests${NC}"
echo -e "${BLUE}Server: $SERVER_URL${NC}"
echo -e "${BLUE}======================================${NC}"
echo

# Test 1: Health Check
echo -e "${BLUE}Test 1: Health Check${NC}"
curl -s "$SERVER_URL/health" | jq '.'
echo
echo

# Test 2: Make a decision (user_001)
echo -e "${BLUE}Test 2: Make a decision (user_001)${NC}"
curl -s -X POST "$SERVER_URL/v1/decide" \
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
  }' | jq '.'
echo
echo

# Test 3: Make a decision (user_003 - should have higher risk)
echo -e "${BLUE}Test 3: Make a decision (user_003 - higher risk)${NC}"
curl -s -X POST "$SERVER_URL/v1/decide" \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_003",
      "device_id": "device_003",
      "ip_address": "203.0.113.3",
      "event.type": "transaction",
      "event.user_id": "user_003",
      "event.device_id": "device_003",
      "event.ip_address": "203.0.113.3",
      "event.event_type": "transaction"
    }
  }' | jq '.'
echo
echo

echo -e "${GREEN}All tests completed!${NC}"

