# Comprehensive DSL Demo - Test Cases

This document contains curl test cases for `comprehensive_risk_assessment` pipeline.

## Quick Start

```bash
# Run all tests
./comprehensive_dsl_demo_test.sh

# Or run individual tests below
```

## Test Cases

### Test 1: High-Value Transaction (>10000)

**Branch**: High-value (all 3 rulesets)
**Expected**: REVIEW or DENY

```bash
curl -X POST http://localhost:8080/v1/decide \
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
  }'
```

---

### Test 2: Medium-Value with Suspicious Behavior

**Branch**: Medium-value (2 rulesets)
**Expected**: REVIEW

**Triggers**:
- `suspicious_user_behavior` (login_attempts > 5, ip_country != registered_country)
- `risk_threshold_check` (risk_score = 75)

```bash
curl -X POST http://localhost:8080/v1/decide \
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
  }'
```

---

### Test 3: Confirmed Fraud Pattern

**Branch**: Medium-value (2 rulesets)
**Expected**: DENY

**Triggers**:
- `confirmed_fraud_pattern` (amount > 5000, account_age_days < 7, recipient_new)
- `complex_fraud_detection` (unverified + crypto)

```bash
curl -X POST http://localhost:8080/v1/decide \
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
  }'
```

---

### Test 4: Low-Value Trusted User

**Branch**: Low-value (1 ruleset)
**Expected**: APPROVE

**Triggers**:
- `trusted_user_bonus` (-20 score: verified, account_age_days > 365, dispute_count = 0)

```bash
curl -X POST http://localhost:8080/v1/decide \
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
  }'
```

---

### Test 5: Restricted Payment Method

**Branch**: Medium-value (2 rulesets)
**Expected**: REVIEW

**Triggers**:
- `payment_method_check` (crypto + contains "urgent")

```bash
curl -X POST http://localhost:8080/v1/decide \
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
  }'
```

---

### Test 6: Velocity Check Trigger

**Branch**: Medium-value (2 rulesets)
**Expected**: REVIEW

**Triggers**:
- `velocity_check` (amount > average * 3, count > avg_daily * 2)

```bash
curl -X POST http://localhost:8080/v1/decide \
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
      "confidence_level": "medium"
    }
  }'
```

---

### Test 7: Complex Fraud Pattern

**Branch**: Medium-value (2 rulesets)
**Expected**: REVIEW or DENY

**Triggers**:
- `complex_fraud_detection` (verified=false + payment_method=crypto)

```bash
curl -X POST http://localhost:8080/v1/decide \
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
      "confidence_level": "high"
    }
  }'
```

---

### Test 8: VIP User High-Value Transaction

**Branch**: High-value (all 3 rulesets)
**Expected**: APPROVE

**Triggers**:
- `high_value_transaction` (75 points)
- `trusted_user_bonus` (-20 points)
- Net score: 55 → Should approve

```bash
curl -X POST http://localhost:8080/v1/decide \
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
      "confidence_level": "high"
    }
  }'
```

---

## Expected Results Summary

| Test | Amount | Branch | Triggers | Expected Action |
|------|--------|--------|----------|----------------|
| 1 | $15,000 | High-value | `high_value_transaction` | REVIEW |
| 2 | $5,000 | Medium | `suspicious_user_behavior`, `risk_threshold_check` | REVIEW |
| 3 | $8,000 | Medium | `confirmed_fraud_pattern`, `complex_fraud_detection` | DENY |
| 4 | $500 | Low | `trusted_user_bonus` | APPROVE |
| 5 | $3,000 | Medium | `payment_method_check` | REVIEW |
| 6 | $9,000 | Medium | `velocity_check` | REVIEW |
| 7 | $7,500 | Medium | `complex_fraud_detection` | REVIEW/DENY |
| 8 | $12,000 | High | `high_value_transaction`, `trusted_user_bonus` | APPROVE |

## Rule Coverage

This test suite covers all 8 rules:

- ✅ `high_value_transaction` - Tests 1, 8
- ✅ `suspicious_user_behavior` - Test 2
- ✅ `confirmed_fraud_pattern` - Test 3
- ✅ `complex_fraud_detection` - Tests 3, 7
- ✅ `risk_threshold_check` - Test 2
- ✅ `payment_method_check` - Test 5
- ✅ `velocity_check` - Test 6
- ✅ `trusted_user_bonus` - Tests 4, 8

## DSL Features Demonstrated

- **Logical operators**: `any`, `all` (Test 2, 3)
- **Comparison operators**: `>`, `<`, `==`, `!=`, `>=`, `<=` (All tests)
- **String operations**: `in`, `contains` (Test 5)
- **Arithmetic expressions**: multiplication in conditions (Test 6)
- **Nested conditions**: nested `any`/`all` (Test 3, 7)
- **Negative scoring**: trusted user bonus (Tests 4, 8)
- **Conditional branching**: amount-based routing (All tests)
- **Multiple rulesets**: 3 different rulesets (All tests)
