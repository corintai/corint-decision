# E2E Test Data

This directory contains test data for end-to-end testing of the CORINT Decision Engine.

## Files

| File | Description |
|------|-------------|
| `events.json` | Raw event data (~90 records) in JSON format |
| `redis_features.txt` | Pre-computed feature values for Redis lookup features |

## Database Initialization Scripts

The raw data is loaded into databases by the init scripts in the parent directory:

| Script | Database | Usage |
|--------|----------|-------|
| `../init_postgresql.sh` | PostgreSQL | Reads events.json and inserts into PostgreSQL |
| `../init_clickhouse.sh` | ClickHouse | Reads events.json and inserts into ClickHouse |
| `../init_sqlite.sh` | SQLite | Reads events.json and inserts into SQLite |
| `../init_redis.sh` | Redis | Reads redis_features.txt and loads into Redis |

## Test Scenarios

The test data covers the following scenarios:

| User ID | Scenario | Expected Behavior |
|---------|----------|-------------------|
| `normal_user_001` | Normal user (low risk) | Approve |
| `normal_user_002` | Normal user with moderate activity | Approve |
| `high_freq_001` | High frequency login (15 logins/24h, 8/1h) | Review (velocity) |
| `failed_login_001` | Multiple failed logins (6 failures/1h) | Review/Decline (account takeover) |
| `high_txn_001` | High transaction volume (12 txns/24h, $8500) | Review (velocity abuse) |
| `multi_device_001` | Multiple devices (8 devices/7d) | Decline (fraud farm) |
| `multi_ip_001` | Multiple IPs/geos (5 IPs/24h, 6 cities/30d) | Review/Decline (impossible travel) |
| `new_user_001` | New user (2 days old) with high-value txn | Review (new account risk) |
| `vip_user_001` | VIP verified user with high-value txn | Approve (bypass rules) |
| `suspicious_001` | Multiple risk signals combined | Decline (fraud farm + account takeover) |
| `stats_user_001` | User for statistical feature testing | Approve (baseline data) |

## Feature Coverage

### Aggregation Features (from events.sql)

| Feature | Coverage |
|---------|----------|
| `login_count_24h` | All users with login events |
| `login_count_1h` | `high_freq_001` (8 logins) |
| `failed_login_count_1h` | `failed_login_001` (6 failures) |
| `transaction_count_24h` | `high_txn_001` (12 txns) |
| `transaction_sum_24h` | `high_txn_001` ($8,500) |
| `transaction_sum_7d` | `high_txn_001` ($15,000+) |
| `avg_transaction_30d` | `stats_user_001` (varied amounts) |
| `max_transaction_14d` | Multiple users |
| `unique_devices_7d` | `multi_device_001` (8 devices) |
| `unique_ips_24h` | `multi_ip_001` (5 IPs) |
| `unique_cities_30d` | `multi_ip_001` (6 cities) |

### Lookup Features (from redis_features.txt)

| Feature | Coverage |
|---------|----------|
| `user_risk_score` | All 11 test users (0.05-0.95) |
| `user_kyc_status` | All users (verified/pending/failed) |
| `user_is_verified` | All users (true/false) |
| `device_risk_score` | 20+ devices |
| `ip_risk_score` | 25+ IPs |
| `ip_country` | Geolocation data |

## Usage

The init scripts read database configuration from `repository/configs/datasources/` YAML files.

### 1. Load Event Data into PostgreSQL

Config file: `repository/configs/datasources/postgres_events.yaml`

```bash
# Use default config file
../init_postgresql.sh

# Use custom config file
../init_postgresql.sh -c /path/to/custom_config.yaml
```

### 2. Load Event Data into ClickHouse

Config file: `repository/configs/datasources/clickhouse_events.yaml`

```bash
# Use default config file
../init_clickhouse.sh

# Use custom config file
../init_clickhouse.sh -c /path/to/custom_config.yaml
```

### 3. Load Event Data into SQLite

Config file: `repository/configs/datasources/sqlite_events.yaml`

```bash
# Use default config file
../init_sqlite.sh

# Use custom config file
../init_sqlite.sh -c /path/to/custom_config.yaml
```

### 4. Load Redis Features

Config file: `repository/configs/datasources/redis_features.yaml`

```bash
# Use default config file
../init_redis.sh

# Use custom config file
../init_redis.sh -c /path/to/custom_config.yaml

# Or manually load data
cat redis_features.txt | grep -v '^#' | grep -v '^$' | redis-cli
```

### 5. Run E2E Tests

```bash
# Start the server
cargo run -p corint-server

# Test with different user scenarios
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{"event": {"user_id": "normal_user_001", "type": "transaction", "amount": 100}}'

curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{"event": {"user_id": "suspicious_001", "type": "transaction", "amount": 999}}'
```

## Expected Results

| User | Expected Decision | Triggered Rules |
|------|-------------------|-----------------|
| `normal_user_001` | APPROVE | None |
| `high_freq_001` | REVIEW | velocity pattern |
| `failed_login_001` | REVIEW/DECLINE | account takeover |
| `high_txn_001` | REVIEW | velocity abuse |
| `multi_device_001` | DECLINE | fraud farm pattern |
| `multi_ip_001` | REVIEW/DECLINE | suspicious geography |
| `new_user_001` | REVIEW | new user fraud |
| `vip_user_001` | APPROVE | VIP bypass |
| `suspicious_001` | DECLINE | multiple patterns |

## Notes

1. **Timestamps**: Event timestamps use `timestamp_offset` field in JSON (e.g., "-2h", "-30m", "-7d"). The init scripts convert these to database-specific timestamp expressions relative to when the data is loaded.

2. **City Field**: The `city` field is included for `unique_cities_30d` feature. Ensure your events table has this column.

3. **Redis TTL**: The Redis features are loaded without TTL. For production, consider adding expiration.

4. **Dependencies**: The init scripts require:
   - `jq` for JSON parsing (required)
   - `yq` for YAML parsing (optional, falls back to grep/sed)

   Install via:
   - macOS: `brew install jq yq`
   - Ubuntu/Debian: `apt-get install jq` (yq via snap or pip)
   - RHEL/CentOS: `yum install jq` (yq via pip)

5. **Data Cleanup**: The init scripts automatically clean up existing test data before loading. You can also manually clean up:
   ```sql
   DELETE FROM events WHERE user_id LIKE 'normal_user_%'
     OR user_id LIKE 'high_freq_%' OR user_id LIKE 'failed_login_%'
     OR user_id LIKE 'high_txn_%' OR user_id LIKE 'multi_device_%'
     OR user_id LIKE 'multi_ip_%' OR user_id LIKE 'new_user_%'
     OR user_id LIKE 'vip_user_%' OR user_id LIKE 'suspicious_%'
     OR user_id LIKE 'stats_user_%';
   ```
