test case


测试supabase特征计算：
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{"event": {"type": "transaction", "amount": 5000, "user_id": "user_001","source":"supabase"}}'

测试Login异常登陆场景：
curl -s -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{"event": {"type": "login", "ip_address": "69.72.123.1", "user_id": "user_001"}}'

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
    "features": {
        "hours_since_last_login": 1.5,
        "geo_distance_from_last_login": 5000.0,
        "device_fingerprint_changed_24h": 3,
        "is_new_device": true,
        "country_changed": false,
        "failed_login_count_1h": 0
    }
  }' | jq .