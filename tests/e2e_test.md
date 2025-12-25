# CORINT Decision Engine - E2E Test Plan

## 目标

对 CORINT Decision Engine 进行全面的端到端测试，覆盖规则引擎的核心功能和特性。

## 测试架构

### 1. 测试数据层（SQLite）
- **数据库**: `tests/data/e2e_test.db`
- **时间策略**: 使用相对时间偏移（相对于当前时间），避免时间窗口失效
- **数据多样性**: 覆盖多种用户行为模式、交易类型、设备类型、地理位置等
- **数据量**: 足够支持各类特征计算（聚合、窗口、列表匹配等）

### 2. 测试 Repository
- **位置**: `tests/e2e_repo/`
- **内容**:
  - `registry.yaml` - 事件路由配置
  - `pipelines/` - 测试用管道
  - `configs/datasources/` - SQLite 数据源配置
  - `configs/features/` - 特征定义
  - `configs/lists/` - 黑白名单配置

### 3. 测试用例设计

#### Feature 类型覆盖
- ✅ **Simple Features**: 直接字段访问
- ✅ **SQL Features**: 数据库查询特征
- ✅ **Aggregate Features**: 聚合计算（count, sum, avg, max, min）
- ✅ **Window Features**: 时间窗口特征（1h, 24h, 7d, 30d）
- ✅ **List Features**: 黑白名单匹配
- ✅ **Calculated Features**: 派生计算特征
- ✅ **LLM Features**: AI 模型推理特征（可选）

#### Rule 类型覆盖
- ✅ **Simple Conditions**: 基本条件判断
- ✅ **Complex Logic**: AND/OR/NOT 组合
- ✅ **Feature-based Rules**: 基于特征的规则
- ✅ **List-based Rules**: 基于列表的规则
- ✅ **Threshold Rules**: 阈值规则
- ✅ **Pattern Matching**: 模式匹配规则

#### Pipeline 场景覆盖
- ✅ **Transaction Flow**: 交易风控流程
- ✅ **Login Flow**: 登录风控流程
- ✅ **Payment Flow**: 支付风控流程
- ✅ **Multi-stage Pipeline**: 多阶段管道
- ✅ **Conditional Routing**: 条件路由

### 4. 测试执行流程

```bash
# 1. 清理旧数据
rm -f tests/data/e2e_test.db

# 2. 生成测试数据（使用相对时间）
python3 tests/scripts/generate_test_data.py

# 3. 启动测试服务器
DATABASE_URL="..." cargo build --release
DATABASE_URL="..." target/release/corint-server &

# 4. 等待服务器启动
sleep 3

# 5. 执行测试用例
bash tests/scripts/run_e2e_tests.sh

# 6. 关闭服务器
pkill corint-server

# 7. 生成测试报告
python3 tests/scripts/generate_report.py
```

### 5. 测试报告

测试报告包含：
- ✅ 总体统计（通过/失败/耗时）
- ✅ 每个测试用例的详细结果
- ✅ Feature 计算性能统计
- ✅ Pipeline 执行追踪
- ✅ 错误诊断信息
- ✅ 覆盖率报告

## 文件结构

```
tests/
├── e2e_test.md                    # 本文档
├── data/
│   └── e2e_test.db               # SQLite 测试数据库（自动生成）
├── e2e_repo/                      # 测试专用 repository
│   ├── registry.yaml             # 事件路由配置
│   ├── pipelines/
│   │   ├── transaction_test.yaml
│   │   ├── login_test.yaml
│   │   └── payment_test.yaml
│   └── configs/
│       ├── datasources/
│       │   └── sqlite_e2e.yaml
│       ├── features/
│       │   └── e2e_features.yaml
│       └── lists/
│           ├── blocked_users.yaml
│           └── trusted_ips.yaml
├── scripts/
│   ├── generate_test_data.py     # 生成测试数据（Python）
│   ├── run_e2e_tests.sh          # 主测试脚本
│   └── generate_report.py        # 生成测试报告
└── results/
    └── e2e_report_*.html         # 测试报告（时间戳）
```

## 时间处理策略

为避免时间窗口失效，所有测试数据的时间戳使用相对时间：

```python
from datetime import datetime, timedelta

# 当前时间
now = datetime.now()

# 生成相对时间
events = [
    {"timestamp": now - timedelta(hours=1)},   # 1小时前
    {"timestamp": now - timedelta(hours=12)},  # 12小时前
    {"timestamp": now - timedelta(days=1)},    # 1天前
    {"timestamp": now - timedelta(days=7)},    # 7天前
    {"timestamp": now - timedelta(days=30)},   # 30天前
]
```

## 快速开始

```bash
# 运行完整 E2E 测试
cd tests
./scripts/run_e2e_tests.sh

# 查看测试报告
open results/e2e_report_latest.html
```