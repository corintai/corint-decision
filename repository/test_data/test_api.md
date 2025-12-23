# API Test Cases

These test cases demonstrate the Corint API request/response format.

## Basic Transaction Evaluation

Test Supabase feature computation:

```bash
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "transaction",
      "amount": 5000,
      "user_id": "user_001",
      "source": "supabase"
    }
  }' | jq .
```

Expected response format:
```json
{
  "request_id": "req_20251223...",
  "status": 200,
  "process_time_ms": 45,
  "pipeline_id": "...",
  "decision": {
    "result": "ALLOW",
    "actions": [],
    "scores": {
      "canonical": 150,
      "raw": 15
    },
    "evidence": {
      "triggered_rules": []
    },
    "cognition": {
      "summary": "...",
      "reason_codes": []
    }
  }
}
```

## Login Anomaly Detection

Test basic login:

```bash
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "login",
      "ip_address": "69.72.123.1",
      "user_id": "user_001"
    }
  }' | jq .
```

Test login with device info and features (with trace enabled):

```bash
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "login",
      "ip_address": "69.72.123.1",
      "user_id": "user_001",
      "device": {
        "is_emulator": false,
        "is_rooted": false,
        "fingerprint_confidence": 0.95,
        "hardware_mismatch": false
      }
    },
    "user": {
      "account_age_days": 365,
      "email_verified": true,
      "phone_verified": true
    },
    "features": {
      "hours_since_last_login": 1.5,
      "geo_distance_from_last_login": 5000.0,
      "device_fingerprint_changed_24h": 3,
      "is_new_device": true,
      "country_changed": false,
      "failed_login_count_1h": 0
    },
    "options": {
      "return_features": true,
      "enable_trace": true
    }
  }' | jq .
```

## Request with User Context

Test transaction with full user context:

```bash
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "transaction",
      "timestamp": "2025-12-23T10:30:00Z",
      "user_id": "user_12345",
      "amount": 1000,
      "currency": "USD",
      "ip_address": "203.0.113.42"
    },
    "user": {
      "account_age_days": 365,
      "email_verified": true,
      "phone_verified": true,
      "kyc_status": "verified",
      "risk_level": "low"
    },
    "options": {
      "return_features": true
    }
  }' | jq .
```
