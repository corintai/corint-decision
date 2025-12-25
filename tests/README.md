# CORINT Decision Engine - E2E Tests

Complete end-to-end testing suite for the CORINT Decision Engine.

## Quick Start

```bash
# From project root directory - ONE COMMAND TO RUN ALL TESTS
./tests/scripts/run_e2e_tests.sh
```

The script will automatically:
1. ✅ Generate SQL test data with relative timestamps
2. ✅ Create SQLite database and insert data
3. ✅ Build the server in release mode
4. ✅ Start the server with test configuration
5. ✅ Run all 10 test cases
6. ✅ Display detailed test report in console
7. ✅ Clean up server process

## Test Report Example

```
============================================================================
Test Summary:
  Total Tests:  10
  Passed:       8
  Failed:       2

✓ Passed Tests (8):
  ✓ Normal Transaction
  ✓ Blocked User Transaction
  ✓ High Value New User
  ✓ Normal Login
  ✓ Blocked IP Login
  ✓ Normal Payment
  ✓ Very High Payment
  ✓ Blocked User Payment

✗ Failed Tests (2):
  ✗ High Risk Country
    → Expected: review, Got: approve
  ✗ High Risk Country Login
    → Expected: review, Got: decline
============================================================================
```

## What Gets Tested

### Feature Types
- ✅ **Simple Features**: Direct event field access
- ✅ **SQL Features**: Database queries
- ✅ **Aggregate Features**: COUNT, SUM, AVG, MAX, MIN
- ✅ **Window Features**: 1h, 24h, 7d, 30d time windows
- ✅ **List Features**: Blocklist/allowlist matching
- ✅ **Velocity Features**: Rate of change detection

### Rule Types
- ✅ **Threshold Rules**: Numeric comparisons
- ✅ **List-based Rules**: Membership checks
- ✅ **Complex Logic**: AND/OR/NOT combinations
- ✅ **Conditional Rules**: When clauses

### Pipeline Scenarios
- ✅ **Transaction Flow**: Fraud detection, velocity checks, geo-risk
- ✅ **Login Flow**: Brute force detection, account takeover
- ✅ **Payment Flow**: Amount validation, user verification

### Test Cases (10 Total)

#### Transaction Tests (4)
1. Normal Transaction → APPROVE
2. Blocked User → DENY
3. High Value from New User → REVIEW
4. High-Risk Country → REVIEW

#### Login Tests (3)
5. Normal Login → APPROVE
6. Blocked IP Address → DENY
7. High-Risk Country → REVIEW

#### Payment Tests (3)
8. Normal Payment → APPROVE
9. Very High Payment → REVIEW
10. Blocked User → DENY

## Test Data

Test data is generated as **SQL INSERT statements** with **relative timestamps** to avoid time window issues:

```python
# Events generated relative to current time
- 1 hour ago
- 24 hours ago
- 7 days ago
- 30 days ago
```

### Workflow (Automatic)

When you run `./tests/scripts/run_e2e_tests.sh`:

1. **Generate SQL**: Python script creates `test_data.sql` with 445 events
2. **Create Database**: Executes `sqlite3 e2e_test.db < test_data.sql`
3. **Build Server**: Compiles release build
4. **Run Tests**: Executes 10 test cases via API calls
5. **Report Results**: Displays pass/fail summary in console

### Data Diversity
- **Users**: 50 normal + 10 suspicious + 5 VIP = 65 total
- **Events**: ~445 events across 5 types
- **Patterns**: Velocity abuse, account takeover, geographic anomalies
- **Time Range**: Last 30 days (relative to test execution time)

## Architecture

```
tests/
├── README.md                          # This file
├── e2e_test.md                        # Detailed test plan
├── data/
│   ├── test_data.sql                 # Generated SQL INSERT statements
│   └── e2e_test.db                   # SQLite database (created from SQL)
├── e2e_repo/                          # Test-specific repository
│   ├── registry.yaml                 # Event routing
│   ├── pipelines/
│   │   ├── transaction_test.yaml
│   │   ├── login_test.yaml
│   │   └── payment_test.yaml
│   └── configs/
│       ├── datasources/
│       │   └── sqlite_e2e.yaml
│       ├── features/
│       │   └── e2e_features.yaml    # 17 comprehensive features
│       └── lists/
│           ├── blocked_users.yaml
│           ├── blocked_ips.yaml
│           └── high_risk_countries.yaml
├── scripts/
│   ├── generate_test_data.py        # Generate SQLite test data
│   └── run_e2e_tests.sh             # Main test runner
└── results/
    ├── server.log                    # Server output
    └── failed_tests.log              # Failed test details
```

## Running Individual Components

### 1. Generate SQL Test Data Only (Optional)

```bash
python3 tests/scripts/generate_test_data.py
```

Output: `tests/data/test_data.sql`

**Note**: You don't need to run this manually - the test script does it automatically.

### 2. Inspect Test Data (After Running Tests)

```bash
sqlite3 tests/data/e2e_test.db

# Sample queries
SELECT event_type, COUNT(*) FROM events GROUP BY event_type;
SELECT * FROM events WHERE user_id LIKE 'sus_%' LIMIT 10;
SELECT COUNT(DISTINCT user_id) FROM events;
```

### 3. Start Server Manually with Test Config (Optional)

```bash
# Generate data first
python3 tests/scripts/generate_test_data.py
sqlite3 tests/data/e2e_test.db < tests/data/test_data.sql

# Set environment
export DATABASE_URL="sqlite://$(pwd)/tests/data/e2e_test.db"
export CONFIG_PATH="tests/e2e_repo"

# Build and run
cargo build --release
target/release/corint-server
```

### 4. Manual API Testing

```bash
# Health check
curl http://localhost:8080/health

# Test transaction decision
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event": {
      "type": "transaction",
      "user_id": "user_0001",
      "amount": 150.50,
      "country": "US",
      "timestamp": "2025-12-25T10:00:00Z"
    },
    "options": {
      "enable_trace": true,
      "return_features": true
    }
  }' | jq
```

### 5. View Generated SQL (After Running Tests)

```bash
# View first 50 lines of generated SQL
head -50 tests/data/test_data.sql

# Count total INSERT statements
grep -c "INSERT INTO" tests/data/test_data.sql
```

**Note**: These files are automatically generated when you run the test script.

## Troubleshooting

### Server Won't Start

```bash
# Check if port is already in use
lsof -i :8080

# Kill existing process
pkill corint-server

# Check server logs
tail -f tests/results/server.log
```

### Test Data Issues

```bash
# Just re-run the test script - it will regenerate everything
./tests/scripts/run_e2e_tests.sh

# Or manually regenerate SQL and database
python3 tests/scripts/generate_test_data.py
sqlite3 tests/data/e2e_test.db < tests/data/test_data.sql

# Verify data
sqlite3 tests/data/e2e_test.db "SELECT COUNT(*) FROM events;"
```

### Failed Tests

Failed test details are shown directly in the console output:
```
✗ Failed Tests (2):
  ✗ High Risk Country
    → Expected: review, Got: approve
  ✗ High Risk Country Login
    → Expected: review, Got: decline
```

## Extending Tests

### Add New Test Case

Edit `tests/scripts/run_e2e_tests.sh`:

```bash
run_test_case "My New Test" '{
    "event": {
        "type": "transaction",
        "user_id": "user_0001",
        "amount": 999.99,
        "country": "CA"
    }
}' "APPROVE"
```

### Add New Feature

Edit `tests/e2e_repo/configs/features/e2e_features.yaml`:

```yaml
- name: my_new_feature
  type: sql
  value_type: integer
  datasource: sqlite_e2e
  query: |
    SELECT COUNT(*)
    FROM events
    WHERE user_id = :user_id
  parameters:
    user_id: event.user_id
```

### Add New Pipeline

Create `tests/e2e_repo/pipelines/my_pipeline.yaml` and update `registry.yaml`.

## CI/CD Integration

### GitHub Actions Example

```yaml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run E2E Tests
        run: ./tests/scripts/run_e2e_tests.sh
```

## Performance Benchmarks

Typical execution times:
- Data generation: ~2s
- Server build: ~30s (release mode)
- Server startup: ~3s
- Test execution: ~10s
- **Total**: ~45s

## Requirements

- Rust 1.70+
- Python 3.8+
- SQLite 3
- curl
- jq

## Support

For issues or questions:
1. Check `tests/e2e_test.md` for detailed documentation
2. Review failed test logs in `tests/results/`
3. Open an issue with reproduction steps
