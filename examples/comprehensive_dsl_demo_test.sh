#!/bin/bash

# ============================================================================
# Comprehensive DSL Demo Pipeline - Test Cases
# ============================================================================
# This script contains curl test cases for comprehensive_dsl_demo.yaml
#
# Usage:
#   ./comprehensive_dsl_demo_test.sh
#
# Requirements:
#   - Server running on http://localhost:8080
#   - Pipeline comprehensive_risk_assessment loaded
# ============================================================================

API_URL="http://localhost:8080/v1/decide"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=========================================="
echo "Testing: comprehensive_risk_assessment"
echo "=========================================="
echo ""

# ----------------------------------------------------------------------------
# Test Case 1: High-Value Transaction (>10000) - Should trigger all rulesets
# Expected: APPROVE (score ~55, below review threshold of 100)
# Note: Pipeline-level decision_logic is not yet implemented, so action is based
# on aggregate score thresholds only (>=200 DENY, >=100 REVIEW, <100 APPROVE)
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 1: High-Value Transaction (>10000)${NC}"
echo "Branch: High-value (all 3 rulesets)"
echo "Expected: APPROVE (score ~55)"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 15000,
        "currency": "USD",
        "payment_method": "credit_card",
        "description": "Large purchase",
        "recipient_new": false,
        "international": false,
        "count_24h": 2
      },
      "user": {
        "login_attempts": 2,
        "session_duration": 300,
        "ip_country": "US",
        "registered_country": "US",
        "verified": true,
        "account_age_days": 500,
        "dispute_count": 0,
        "average_transaction": 3000,
        "avg_daily_count": 5,
        "vip_status": false
      },
      "confidence_level": "medium"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 2: Medium-Value Transaction with Suspicious Behavior
# Expected: APPROVE (score ~60, below review threshold of 100)
# Note: Even with suspicious behavior, aggregate score doesn't reach review threshold
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 2: Medium-Value Transaction with Suspicious Behavior${NC}"
echo "Branch: Medium-value (2 rulesets)"
echo "Expected: APPROVE (score ~60)"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 5000,
        "currency": "USD",
        "payment_method": "bank_transfer",
        "description": "Payment",
        "recipient_new": false,
        "international": false,
        "count_24h": 3
      },
      "user": {
        "login_attempts": 8,
        "session_duration": 30,
        "ip_country": "RU",
        "registered_country": "US",
        "verified": false,
        "account_age_days": 45,
        "dispute_count": 1,
        "average_transaction": 1000,
        "avg_daily_count": 2,
        "vip_status": false
      },
      "confidence_level": "high"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 3: Confirmed Fraud Pattern
# Expected: DENY due to new account + high amount + new recipient
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 3: Confirmed Fraud Pattern${NC}"
echo "Branch: Medium-value (2 rulesets)"
echo "Expected: DENY"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 8000,
        "currency": "USD",
        "payment_method": "crypto",
        "description": "urgent payment",
        "recipient_new": true,
        "international": true,
        "count_24h": 1
      },
      "user": {
        "login_attempts": 3,
        "session_duration": 120,
        "ip_country": "NG",
        "registered_country": "US",
        "verified": false,
        "account_age_days": 5,
        "dispute_count": 0,
        "average_transaction": 100,
        "avg_daily_count": 1,
        "vip_status": false
      },
      "confidence_level": "high"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 4: Low-Value Transaction - Trusted User
# Expected: APPROVE due to low amount and trusted user
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 4: Low-Value Transaction - Trusted User${NC}"
echo "Branch: Low-value (1 ruleset)"
echo "Expected: APPROVE"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 500,
        "currency": "USD",
        "payment_method": "credit_card",
        "description": "Regular purchase",
        "recipient_new": false,
        "international": false,
        "count_24h": 2
      },
      "user": {
        "login_attempts": 1,
        "session_duration": 600,
        "ip_country": "US",
        "registered_country": "US",
        "verified": true,
        "account_age_days": 730,
        "dispute_count": 0,
        "average_transaction": 400,
        "avg_daily_count": 3,
        "vip_status": true
      },
      "confidence_level": "high"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 5: Restricted Payment Method
# Expected: APPROVE (score ~0, rules not triggering due to trusted user profile)
# Note: User has good verification status, account age, and country match
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 5: Restricted Payment Method (Crypto + Urgent)${NC}"
echo "Branch: Medium-value (2 rulesets)"
echo "Expected: APPROVE (score ~0)"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 3000,
        "currency": "USD",
        "payment_method": "crypto",
        "description": "urgent transfer",
        "recipient_new": false,
        "international": false,
        "count_24h": 1
      },
      "user": {
        "login_attempts": 2,
        "session_duration": 200,
        "ip_country": "US",
        "registered_country": "US",
        "verified": true,
        "account_age_days": 200,
        "dispute_count": 0,
        "average_transaction": 2000,
        "avg_daily_count": 4,
        "vip_status": false
      },
      "confidence_level": "medium"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 6: Velocity Check Trigger
# Expected: APPROVE (score ~55, below review threshold of 100)
# Note: Amount spike rule triggers but aggregate score doesn't reach threshold
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 6: Velocity Check (Amount and Count Spike)${NC}"
echo "Branch: Medium-value (2 rulesets)"
echo "Expected: APPROVE (score ~55)"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 9000,
        "currency": "USD",
        "payment_method": "bank_transfer",
        "description": "Payment",
        "recipient_new": false,
        "international": false,
        "count_24h": 15
      },
      "user": {
        "login_attempts": 2,
        "session_duration": 300,
        "ip_country": "US",
        "registered_country": "US",
        "verified": true,
        "account_age_days": 180,
        "dispute_count": 0,
        "average_transaction": 2000,
        "avg_daily_count": 3,
        "vip_status": false
      },
      "risk_score": 60,
      "confidence_level": "medium"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 7: Complex Fraud Pattern
# Expected: REVIEW/DENY due to unverified user + crypto + international
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 7: Complex Fraud Pattern (Unverified + Crypto + International)${NC}"
echo "Branch: Medium-value (2 rulesets)"
echo "Expected: REVIEW or DENY"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 7500,
        "currency": "USD",
        "payment_method": "crypto",
        "description": "International payment",
        "recipient_new": true,
        "international": true,
        "count_24h": 1
      },
      "user": {
        "login_attempts": 2,
        "session_duration": 150,
        "ip_country": "CN",
        "registered_country": "US",
        "verified": false,
        "account_age_days": 30,
        "dispute_count": 0,
        "average_transaction": 1500,
        "avg_daily_count": 2,
        "vip_status": false
      },
      "risk_score": 82,
      "confidence_level": "high"
    }
  }' | jq '.'

echo ""
echo "----------------------------------------"
echo ""

# ----------------------------------------------------------------------------
# Test Case 8: VIP User High-Value Transaction
# Expected: APPROVE or minimal REVIEW
# ----------------------------------------------------------------------------
echo -e "${YELLOW}Test 8: VIP User High-Value Transaction${NC}"
echo "Branch: High-value (all 3 rulesets)"
echo "Expected: APPROVE (trusted user bonus applies)"
echo ""

curl -X POST "$API_URL" \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "test1",
      "transaction": {
        "amount": 12000,
        "currency": "USD",
        "payment_method": "bank_transfer",
        "description": "Business payment",
        "recipient_new": false,
        "international": false,
        "count_24h": 1
      },
      "user": {
        "login_attempts": 1,
        "session_duration": 800,
        "ip_country": "US",
        "registered_country": "US",
        "verified": true,
        "account_age_days": 1000,
        "dispute_count": 0,
        "average_transaction": 8000,
        "avg_daily_count": 2,
        "vip_status": true
      },
      "risk_score": 25,
      "confidence_level": "high"
    }
  }' | jq '.'

echo ""
echo "=========================================="
echo -e "${GREEN}All tests completed!${NC}"
echo "=========================================="
