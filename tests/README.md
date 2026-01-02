# CORINT Decision Engine - E2E Tests

Complete end-to-end testing suite for the CORINT Decision Engine.

## Quick Start

Run all tests with a single command:

```bash
# From project root directory - ONE COMMAND TO RUN ALL TESTS
./tests/scripts/run_e2e_tests.sh
```

### Automated Test Flow

The script automatically executes the following steps:

```
┌─────────────────────────────────────────────────────────────┐
│  Step 1: Generate SQL Test Data                            │
│  ✓ Creates test_data.sql with 445 events                   │
│  ✓ Creates 12 list entries (blocked users/IPs/countries)   │
│  ✓ Uses relative timestamps (1h, 24h, 7d, 30d ago)         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  Step 2: Create SQLite Database                            │
│  ✓ Deletes old database                                    │
│  ✓ Executes: sqlite3 e2e_test.db < test_data.sql           │
│  ✓ Verifies 445 events + 12 list entries inserted          │
│  ✓ Displays list statistics (blocked_users/IPs/countries)  │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  Step 3: Build Server (Release Mode)                       │
│  ✓ cargo build --release                                   │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  Step 4: Start Server                                      │
│  ✓ Starts server on port 8080                              │
│  ✓ Uses test repository (tests/e2e_repo)                   │
│  ✓ Waits for health check                                  │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  Step 5: Run 10 Test Cases                                 │
│  • 4 Transaction Tests                                     │
│  • 3 Login Tests                                           │
│  • 3 Payment Tests                                         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  Step 6: Display Test Report                               │
│  ✓ Summary: Total / Passed / Failed                        │
│  ✓ List all passed tests                                   │
│  ✓ List all failed tests with details                      │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│  Step 7: Cleanup                                           │
│  ✓ Stops server process                                    │
│  ✓ Exit with appropriate code                              │
└─────────────────────────────────────────────────────────────┘
```

### What You DON'T Need to Do

The test automation handles everything - you don't need to:

- ❌ Manually run Python scripts
- ❌ Manually create the database
- ❌ Manually start the server
- ❌ Manually check log files
- ❌ Manually track which tests passed or failed

**Everything is automated!** 🚀

## Example Output

When you run the test script, you'll see detailed output for each step:

```bash
$ ./tests/scripts/run_e2e_tests.sh

============================================================================
CORINT Decision Engine - E2E Test Suite
============================================================================

[INFO] Step 1: Generating SQL test data...
============================================================
CORINT E2E Test Data Generator
============================================================

Generating test data...
  ✓ Normal transactions: 100 events
  ✓ Suspicious transactions: 20 events
  ✓ Velocity abuse patterns: 30 events
  ✓ Login events: 200 events
  ✓ Payment events: 80 events
  ✓ Account takeover patterns: 15 events

Generating list data...
  ✓ Blocked users: 5 entries
  ✓ Blocked IPs: 3 entries
  ✓ High risk countries: 4 entries

✓ SQL file generated: tests/data/test_data.sql
[✓] SQL data generated

[INFO] Step 1.5: Creating database and inserting test data...
[✓] Database created with 445 events and 12 list entries

[INFO] Step 1.6: Verifying database list data...
[✓] Database list verification passed
  Lists created:
  - blocked_ips: 3 entries
  - blocked_users: 5 entries
  - high_risk_countries: 4 entries

[INFO] Step 2: Building server...
[✓] Server built successfully

[INFO] Step 3: Starting test server...
[INFO] Server started (PID: 12345)
[INFO] Waiting for server to start...
[✓] Server is ready!

[INFO] Step 4: Running test cases...
============================================================================

--- Transaction Flow Tests ---

[✓] Normal Transaction: PASSED (decision: APPROVE)
[✓] Blocked User Transaction: PASSED (decision: DENY)
[✓] High Value New User: PASSED (decision: REVIEW)
[✓] High Risk Country: PASSED (decision: REVIEW)

--- Login Flow Tests ---

[✓] Normal Login: PASSED (decision: APPROVE)
[✓] Blocked IP Login: PASSED (decision: DENY)
[✓] High Risk Country Login: PASSED (decision: REVIEW)

--- Payment Flow Tests ---

[✓] Normal Payment: PASSED (decision: APPROVE)
[✓] Very High Payment: PASSED (decision: REVIEW)
[✓] Blocked User Payment: PASSED (decision: DENY)

============================================================================

[INFO] Step 5: Test Report
============================================================================

Test Summary:
  Total Tests:  10
  Passed:       10
  Failed:       0

✓ Passed Tests (10):
  ✓ Normal Transaction
  ✓ Blocked User Transaction
  ✓ High Value New User
  ✓ High Risk Country
  ✓ Normal Login
  ✓ Blocked IP Login
  ✓ High Risk Country Login
  ✓ Normal Payment
  ✓ Very High Payment
  ✓ Blocked User Payment

============================================================================

[✓] All tests passed! 🎉
```

### Example with Failed Tests

If some tests fail, you'll see detailed information:

```
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

## Generated Files

After running the test script, the following files are automatically created:

```
tests/
├── data/
│   ├── test_data.sql       # Generated SQL statements (445 events + 12 list entries)
│   └── e2e_test.db         # SQLite database (contains events and list_entries tables)
└── results/
    └── server.log          # Server output logs
```

## Test Data

Test data is generated as **SQL INSERT statements** with **relative timestamps** to avoid time window issues:

```python
# Events generated relative to current time
- 1 hour ago
- 24 hours ago
- 7 days ago
- 30 days ago
```

### Automatic Workflow

When you run `./tests/scripts/run_e2e_tests.sh`, the script automatically:

1. **Generate SQL**: Python script creates `test_data.sql` with 445 events
2. **Create Database**: Executes `sqlite3 e2e_test.db < test_data.sql`
3. **Build Server**: Compiles release build
4. **Run Tests**: Executes 10 test cases via API calls
5. **Report Results**: Displays pass/fail summary in console

### Fresh Test Environment

Every test run creates a fresh environment:

- ✅ Regenerates SQL with timestamps relative to current time
- ✅ Deletes old database and rebuilds from scratch
- ✅ Recompiles the server
- ✅ Re-executes all test cases

This ensures consistent and reproducible test results.

### Data Diversity
- **Users**: 50 normal + 10 suspicious + 5 VIP = 65 total
- **Events**: ~445 events across 5 types
- **Patterns**: Velocity abuse, account takeover, geographic anomalies
- **Time Range**: Last 30 days (relative to test execution time)

### Database Content

The test database contains two main tables:

#### 1. Events Table
- **445 test events** across multiple event types
- Event types: `transaction`, `login`, `payment`
- Timestamps relative to current time (1h to 30d ago)
- Diverse user patterns (normal, suspicious, VIP)

#### 2. List Entries Table (12 total entries)

**List Structure:**
- **blocked_users** (5 entries): `sus_0001`, `sus_0002`, `sus_0003`, `sus_0004`, `sus_0005`
- **blocked_ips** (3 entries): Malicious IP addresses from known botnets
- **high_risk_countries** (4 entries): `NG`, `RU`, `CN`, `KP`

**Database Schema:**
```sql
CREATE TABLE list_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    list_id TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT,
    metadata TEXT
);
```

**Query List Data:**
```bash
# View all lists with counts
sqlite3 tests/data/e2e_test.db \
  "SELECT list_id, COUNT(*) FROM list_entries GROUP BY list_id"

# Output:
# blocked_ips|3
# blocked_users|5
# high_risk_countries|4

# Check specific list
sqlite3 tests/data/e2e_test.db \
  "SELECT * FROM list_entries WHERE list_id = 'blocked_users'"

# Or use the automated verification script
bash tests/scripts/verify_db_lists.sh
```

**Implementation Note**: The current test suite uses **memory-backed lists** (loaded from YAML configs). Database-backed lists require PostgreSQL. The SQLite list data exists for verification and future migration.

## Architecture

```
tests/
├── README.md                          # This file (complete test documentation)
├── e2e_test.md                        # Detailed test plan
├── data/                              # Generated test data (created at runtime)
│   ├── test_data.sql                 # Generated SQL INSERT statements
│   └── e2e_test.db                   # SQLite database (created from SQL)
├── e2e_repo/                          # Test-specific repository
│   ├── registry.yaml                 # Event routing configuration
│   ├── pipelines/
│   │   ├── transaction_test.yaml    # Transaction flow pipeline
│   │   ├── login_test.yaml          # Login flow pipeline
│   │   └── payment_test.yaml        # Payment flow pipeline
│   └── configs/
│       ├── datasources/
│       │   └── sqlite_e2e.yaml      # SQLite datasource config
│       ├── features/
│       │   └── e2e_features.yaml    # 17 comprehensive features
│       └── lists/
│           ├── blocked_users.yaml   # Memory-backed blocked users list
│           ├── blocked_ips.yaml     # Memory-backed blocked IPs list
│           └── high_risk_countries.yaml  # Memory-backed country list
├── scripts/
│   ├── generate_test_data.py        # Generate SQLite test data + lists
│   ├── verify_db_lists.sh           # Verify database list entries
│   └── run_e2e_tests.sh             # Main test runner (run this!)
└── results/                           # Test results (created at runtime)
    └── server.log                    # Server output logs
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

### 3. Verify Database Lists (After Running Tests)

```bash
# Run automated verification script
bash tests/scripts/verify_db_lists.sh

# Manual queries
sqlite3 tests/data/e2e_test.db "SELECT list_id, COUNT(*) FROM list_entries GROUP BY list_id"
sqlite3 tests/data/e2e_test.db "SELECT * FROM list_entries WHERE list_id = 'blocked_users'"
sqlite3 tests/data/e2e_test.db "SELECT * FROM list_entries WHERE list_id = 'blocked_ips'"
sqlite3 tests/data/e2e_test.db "SELECT * FROM list_entries WHERE list_id = 'high_risk_countries'"
```

### 4. Start Server Manually with Test Config (Optional)

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

### 5. Manual API Testing

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

### 6. View Generated SQL (After Running Tests)

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
