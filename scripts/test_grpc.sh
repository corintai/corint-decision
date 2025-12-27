#!/bin/bash
# gRPC Integration Test Script

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

GRPC_HOST="localhost:50051"

echo -e "${YELLOW}=== gRPC Integration Tests ===${NC}\n"

# Test 1: Health Check
echo -e "${YELLOW}Test 1: Health Check${NC}"
HEALTH_RESPONSE=$(grpcurl -plaintext $GRPC_HOST corint.decision.v1.DecisionService/HealthCheck)
if echo "$HEALTH_RESPONSE" | grep -q '"status": "healthy"'; then
    echo -e "${GREEN}✓ Health check passed${NC}"
else
    echo -e "${RED}✗ Health check failed${NC}"
    exit 1
fi

# Test 2: Decision API - Basic Transaction
echo -e "\n${YELLOW}Test 2: Decision API - Basic Transaction${NC}"
DECISION_RESPONSE=$(grpcurl -plaintext -d '{
    "event": {
        "user_id": {"string_value": "user_001"},
        "type": {"string_value": "transaction"},
        "amount": {"double_value": 100.0}
    }
}' $GRPC_HOST corint.decision.v1.DecisionService/Decide)

if echo "$DECISION_RESPONSE" | grep -q '"result"'; then
    echo -e "${GREEN}✓ Decision API basic transaction passed${NC}"
    echo "Response: $(echo $DECISION_RESPONSE | jq -c '.decision.result')"
else
    echo -e "${RED}✗ Decision API basic transaction failed${NC}"
    exit 1
fi

# Test 3: Decision API - High Risk Transaction
echo -e "\n${YELLOW}Test 3: Decision API - High Risk Transaction${NC}"
HIGH_RISK_RESPONSE=$(grpcurl -plaintext -d '{
    "event": {
        "user_id": {"string_value": "user_002"},
        "type": {"string_value": "transaction"},
        "amount": {"double_value": 50000.0}
    }
}' $GRPC_HOST corint.decision.v1.DecisionService/Decide)

if echo "$HIGH_RISK_RESPONSE" | grep -q '"result"'; then
    echo -e "${GREEN}✓ Decision API high risk transaction passed${NC}"
    echo "Response: $(echo $HIGH_RISK_RESPONSE | jq -c '.decision.result')"
else
    echo -e "${RED}✗ Decision API high risk transaction failed${NC}"
    exit 1
fi

# Test 4: Decision API - With User Context
echo -e "\n${YELLOW}Test 4: Decision API - With User Context${NC}"
USER_CONTEXT_RESPONSE=$(grpcurl -plaintext -d '{
    "event": {
        "user_id": {"string_value": "user_003"},
        "type": {"string_value": "transaction"},
        "amount": {"double_value": 250.0},
        "merchant_id": {"string_value": "merchant_001"}
    },
    "user": {
        "account_age_days": {"int_value": 30},
        "is_verified": {"bool_value": true}
    }
}' $GRPC_HOST corint.decision.v1.DecisionService/Decide)

if echo "$USER_CONTEXT_RESPONSE" | grep -q '"result"'; then
    echo -e "${GREEN}✓ Decision API with user context passed${NC}"
    echo "Response: $(echo $USER_CONTEXT_RESPONSE | jq -c '.decision.result')"
else
    echo -e "${RED}✗ Decision API with user context failed${NC}"
    exit 1
fi

# Test 5: Service Reflection
echo -e "\n${YELLOW}Test 5: Service Reflection${NC}"
SERVICES=$(grpcurl -plaintext $GRPC_HOST list)
if echo "$SERVICES" | grep -q "corint.decision.v1.DecisionService"; then
    echo -e "${GREEN}✓ gRPC reflection working${NC}"
    echo "Available services:"
    echo "$SERVICES" | sed 's/^/  /'
else
    echo -e "${RED}✗ gRPC reflection failed${NC}"
    exit 1
fi

# Test 6: Reload Repository
echo -e "\n${YELLOW}Test 6: Reload Repository${NC}"
RELOAD_RESPONSE=$(grpcurl -plaintext $GRPC_HOST corint.decision.v1.DecisionService/ReloadRepository)
if echo "$RELOAD_RESPONSE" | grep -q '"success": true'; then
    echo -e "${GREEN}✓ Repository reload passed${NC}"
else
    echo -e "${RED}✗ Repository reload failed${NC}"
    exit 1
fi

# Summary
echo -e "\n${GREEN}=== All gRPC Integration Tests Passed ===${NC}"
echo -e "Tested endpoints:"
echo -e "  ✓ HealthCheck"
echo -e "  ✓ Decide (basic transaction)"
echo -e "  ✓ Decide (high risk transaction)"
echo -e "  ✓ Decide (with user context)"
echo -e "  ✓ Service reflection"
echo -e "  ✓ ReloadRepository"
