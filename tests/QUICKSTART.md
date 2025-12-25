# E2E Tests - Quick Start Guide

## 运行测试只需一条命令

```bash
./tests/scripts/run_e2e_tests.sh
```

## 这条命令会自动完成以下所有步骤：

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

## 示例输出

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

## 生成的文件

运行后会自动创建：

```
tests/
├── data/
│   ├── test_data.sql       # 生成的 SQL 语句（445 条事件 + 12 条名单数据）
│   └── e2e_test.db         # SQLite 数据库（包含事件和名单表）
└── results/
    └── server.log          # 服务器日志
```

## 数据库内容

### 事件表 (events)
- 445 条测试事件
- 涵盖 transaction, login, payment 等类型
- 时间戳相对当前时间（1小时到30天前）

### 名单表 (list_entries)
- **blocked_users**: 5 条（sus_0001 - sus_0005）
- **blocked_ips**: 3 条（已知恶意IP地址）
- **high_risk_countries**: 4 条（NG, RU, CN, KP）

### 查询数据库名单

```bash
# 使用验证脚本
bash tests/scripts/verify_db_lists.sh

# 或手动查询
sqlite3 tests/data/e2e_test.db "SELECT list_id, COUNT(*) FROM list_entries GROUP BY list_id"
sqlite3 tests/data/e2e_test.db "SELECT * FROM list_entries WHERE list_id = 'blocked_users'"
```

## 重新运行测试

每次运行都会：
- ✅ 重新生成 SQL（时间戳相对当前时间）
- ✅ 删除旧数据库并重建
- ✅ 重新编译服务器
- ✅ 重新执行所有测试

## 你不需要

- ❌ 手动运行 Python 脚本
- ❌ 手动创建数据库
- ❌ 手动启动服务器
- ❌ 手动查看日志文件

**一切都是自动的！** 🚀
