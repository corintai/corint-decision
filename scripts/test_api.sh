#!/bin/bash

echo "=== CORINT Decision Engine API 测试 ==="
echo ""

echo "1. 健康检查..."
curl -s -X GET http://localhost:8080/health | jq
echo ""
echo ""

echo "2. 欺诈检测 - Transaction Event..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
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

echo "3. 支付检测 - 标准金额 (<= $1000)..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
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

echo "4. 支付检测 - 高额支付 (> $1000)..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
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

echo "5. 边界测试 - 不匹配的事件类型 (login)..."
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "type": "login",
      "user_id": "user_12345",
      "timestamp": "2025-12-11T13:00:00Z"
    }
  }' | jq
echo ""
echo ""

echo "测试完成！"
