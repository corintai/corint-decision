#!/bin/bash

echo "=== CORINT Decision Engine API Test ==="
echo ""

echo "1. Health Check..."
curl -s -X GET http://localhost:8080/health | jq
echo ""
echo ""

echo "2. Fraud Detection - Transaction Event..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "transaction",
      "user_id": "user_12345",
      "amount": 5000,
      "currency": "USD",
      "merchant_id": "merchant_001",
      "timestamp": "2025-12-11T10:30:00Z",
      "ip_address": "192.168.1.100",
      "device_id": "device_abc123",
      "country": "US"
    }
  }' | jq
echo ""
echo ""

echo "3. Payment Detection - Standard Amount (<= \$1000)..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "payment",
      "payment_amount": 500,
      "user_id": "user_98765",
      "email": "user@example.com",
      "ip_address": "203.0.113.45",
      "card_number": "************1234",
      "currency": "USD",
      "merchant_name": "Online Store",
      "timestamp": "2025-12-11T11:00:00Z"
    }
  }' | jq
echo ""
echo ""

echo "4. Payment Detection - High Amount (> \$1000)..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "payment",
      "payment_amount": 5000,
      "user_id": "user_premium_001",
      "email": "vip@example.com",
      "ip_address": "8.8.8.8",
      "card_number": "************9999",
      "currency": "USD",
      "merchant_name": "Luxury Retailer",
      "country": "US",
      "timestamp": "2025-12-11T12:00:00Z"
    }
  }' | jq
echo ""
echo ""

echo "5. Edge Case - Unmatched Event Type (login)..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "login",
      "user_id": "user_12345",
      "timestamp": "2025-12-11T13:00:00Z"
    }
  }' | jq
echo ""
echo ""

echo "6. Payment Detection with Trace - Show Decision Details..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "payment",
      "payment_amount": 5000,
      "user_id": "user_premium_001",
      "email": "vip@example.com",
      "ip_address": "8.8.8.8",
      "currency": "USD",
      "merchant_name": "Luxury Retailer",
      "country": "US",
      "timestamp": "2025-12-11T12:00:00Z"
    },
    "options": {
      "enable_trace": true,
      "return_features": true
    }
  }' | jq '{
    request_id: .request_id,
    status: .status,
    process_time_ms: .process_time_ms,
    pipeline_id: .pipeline_id,
    decision: .decision,
    trace_summary: {
      pipeline_id: .trace.pipeline.pipeline_id,
      steps_count: (.trace.pipeline.steps | length),
      rulesets: [.trace.pipeline.rulesets[].ruleset_id]
    }
  }'
echo ""
echo ""

echo "Test completed!"
