# CORINT Decision Engine Server 使用指南

## 概述

CORINT Decision Engine Server 是一个 HTTP/REST API 服务器，提供决策引擎功能。服务器自动加载规则文件，支持特征按需计算，并通过 RESTful API 对外提供服务。

## 功能特性

- ✅ **REST API**: 通过 HTTP 端点执行决策规则
- ✅ **自动规则加载**: 自动从配置目录加载所有规则文件
- ✅ **特征按需计算**: 规则引用特征时自动从数据源计算
- ✅ **特征缓存**: 计算的特征值被缓存以提高性能
- ✅ **健康检查**: 提供健康检查端点用于监控
- ✅ **CORS 支持**: 跨域资源共享支持
- ✅ **请求追踪**: 内置请求/响应追踪

## 快速开始

### 1. 启动服务器

#### 使用默认配置

```bash
cd /Users/bryanzh/Workspace/corint/corint-decision
cargo run -p corint-server
```

服务器将在 `http://0.0.0.0:8080` 上启动并自动加载 `examples/rules/` 目录中的规则。

#### 使用 Release 模式（推荐生产环境）

```bash
cargo build --release -p corint-server
./target/release/corint-server
```

#### 使用自定义端口

```bash
CORINT_PORT=9090 cargo run -p corint-server
```

### 2. 验证服务器运行

```bash
curl http://localhost:8080/health
```

预期响应：

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### 3. 执行决策请求

```bash
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_001",
      "device_id": "device_001",
      "event.type": "transaction",
      "event.user_id": "user_001"
    }
  }'
```

## 配置

### 配置方式

服务器支持三种配置方式（优先级从高到低）：

1. **环境变量** (前缀: `CORINT_`)
2. **配置文件** (`config/server.yaml`)
3. **默认值**

### 环境变量

```bash
# 服务器主机地址
CORINT_HOST=0.0.0.0

# 服务器端口
CORINT_PORT=8080

# 规则文件目录
CORINT_RULES_DIR=examples/rules

# 启用指标收集
CORINT_ENABLE_METRICS=true

# 启用追踪
CORINT_ENABLE_TRACING=true

# 日志级别
CORINT_LOG_LEVEL=info

# Rust 日志配置
RUST_LOG=info,corint_server=debug,corint_runtime=debug
```

### 配置文件示例

创建 `config/server.yaml`:

```yaml
host: "0.0.0.0"
port: 8080
rules_dir: "examples/rules"
enable_metrics: true
enable_tracing: true
log_level: "info"
```

## API 端点

### 1. 健康检查

**GET** `/health`

返回服务器健康状态。

**响应示例:**

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### 2. 执行决策

**POST** `/v1/decide`

使用事件数据执行决策规则。

**请求体:**

```json
{
  "event_data": {
    "user_id": "user_001",
    "device_id": "device_001",
    "ip_address": "203.0.113.1",
    "event.type": "transaction",
    "event.user_id": "user_001",
    "event.device_id": "device_001",
    "event.ip_address": "203.0.113.1",
    "event.event_type": "transaction"
  }
}
```

**响应示例:**

```json
{
  "action": "Approve",
  "score": 35,
  "triggered_rules": ["high_value_transaction"],
  "explanation": "Transaction approved with moderate risk",
  "processing_time_ms": 125
}
```

**字段说明:**

- `action`: 决策动作 (Approve/Deny/Review/Infer)
- `score`: 风险评分
- `triggered_rules`: 触发的规则列表
- `explanation`: 决策说明
- `processing_time_ms`: 处理时间（毫秒）


## 使用示例

### cURL 示例

```bash
# 健康检查
curl http://localhost:8080/health

# 执行决策
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_003",
      "event.type": "transaction",
      "event.user_id": "user_003"
    }
  }' | jq '.'
```

### Python 示例

```python
import requests

# 健康检查
response = requests.get("http://localhost:8080/health")
print(response.json())

# 执行决策
event_data = {
    "user_id": "user_001",
    "device_id": "device_001",
    "event.type": "transaction",
    "event.user_id": "user_001"
}

response = requests.post(
    "http://localhost:8080/v1/decide",
    json={"event_data": event_data}
)

result = response.json()
print(f"Action: {result['action']}")
print(f"Score: {result['score']}")
print(f"Triggered Rules: {result['triggered_rules']}")
print(f"Processing Time: {result['processing_time_ms']}ms")
```

### 使用测试脚本

#### Bash 脚本

```bash
cd crates/corint-server/examples
./test_api.sh
```

#### Python 脚本

```bash
cd crates/corint-server/examples
python3 test_api.py
```

## 与 Supabase 集成

### 前提条件

1. Supabase 数据库已配置并插入测试数据（参见 `docs/schema/postgres-examples.sql`）
2. 已配置 `examples/configs/datasources/supabase_events.yaml`
3. 规则文件引用了特征（如 `examples/rules/supabase_feature_ruleset.yaml`）

### 工作流程

1. **启动服务器**:

```bash
cargo run -p corint-server
```

2. **发送决策请求**:

```bash
curl -X POST http://localhost:8080/v1/decide \
  -H "Content-Type: application/json" \
  -d '{
    "event_data": {
      "user_id": "user_003",
      "device_id": "device_003",
      "ip_address": "203.0.113.3",
      "event.type": "transaction",
      "event.user_id": "user_003",
      "event.device_id": "device_003",
      "event.ip_address": "203.0.113.3",
      "event.event_type": "transaction"
    }
  }'
```

3. **服务器自动处理**:
   - 执行规则集
   - 当规则引用特征时（如 `transaction_sum_7d > 5000`）
   - 自动从 Supabase 查询并计算特征
   - 缓存特征值供后续规则使用
   - 返回决策结果

### 特征计算日志示例

```
INFO corint_runtime::datasource::client: Executing PostgreSQL query: 
  SELECT SUM((attributes->>'amount')::numeric) AS sum 
  FROM events 
  WHERE user_id = 'user_003' AND event_type = 'transaction' 
  AND event_timestamp >= NOW() - INTERVAL '604800 seconds'

DEBUG corint_runtime::engine::pipeline_executor: 
  Feature 'transaction_sum_7d' calculated: Number(8000.0)
```

## 日志配置

### 基本日志级别

```bash
# Info 级别（默认）
RUST_LOG=info cargo run -p corint-server

# Debug 级别（包含详细信息）
RUST_LOG=debug cargo run -p corint-server

# Trace 级别（包含所有细节）
RUST_LOG=trace cargo run -p corint-server
```

### 模块级别日志

```bash
# 仅 server 模块 debug，其他 info
RUST_LOG=info,corint_server=debug cargo run -p corint-server

# 多个模块 debug
RUST_LOG=info,corint_server=debug,corint_runtime=debug cargo run -p corint-server

# 查看 SQL 查询
RUST_LOG=corint_runtime::datasource=debug cargo run -p corint-server

# 查看特征计算
RUST_LOG=corint_runtime::feature=debug cargo run -p corint-server
```

## 性能优化

### 1. 特征按需计算

- 特征仅在规则引用时才计算
- 避免不必要的数据库查询

### 2. 特征缓存

- 计算的特征值缓存在 `event_data` 中
- 同一请求中后续规则访问相同特征时直接使用缓存

### 3. 异步 I/O

- 所有数据库查询和外部 API 调用都是非阻塞的
- 支持高并发请求处理

### 4. 连接池

- 数据库连接使用连接池管理
- 减少连接建立开销

## 故障排查

### 问题 1: 服务器无法启动

**症状:** 服务器启动失败

**排查步骤:**

1. 检查端口是否被占用:

```bash
lsof -i :8080
```

2. 检查规则目录是否存在:

```bash
ls -la examples/rules
```

3. 查看详细日志:

```bash
RUST_LOG=debug cargo run -p corint-server
```

### 问题 2: 规则未加载

**症状:** 决策返回 "No rules loaded" 或使用默认动作

**排查步骤:**

1. 确认规则文件扩展名为 `.yaml` 或 `.yml`
2. 检查规则文件语法是否正确
3. 查看服务器启动日志中的规则加载信息:

```
INFO corint_server: Loading rules from directory: "examples/rules"
INFO corint_server: Loading rule file: "examples/rules/simple_rule.yaml"
```

### 问题 3: 特征计算失败

**症状:** 决策返回错误或特征值为 0/null

**排查步骤:**

1. 确认 Supabase 数据库连接正常:

```bash
psql "postgresql://postgres.PROJECT_REF:PASSWORD@HOST:PORT/postgres" -c "SELECT 1"
```

2. 检查数据源配置 `examples/configs/datasources/supabase_events.yaml`

3. 查看日志中的 SQL 查询和错误:

```bash
RUST_LOG=corint_runtime::datasource=debug cargo run -p corint-server
```

4. 验证数据库中有测试数据:

```sql
SELECT COUNT(*) FROM events WHERE user_id = 'user_001';
```

### 问题 4: API 请求返回 500 错误

**症状:** API 返回内部服务器错误

**排查步骤:**

1. 查看服务器日志中的错误堆栈
2. 检查请求体格式是否正确
3. 确认 `event_data` 包含所有必需字段
4. 使用 `curl -v` 查看详细响应

## 生产部署建议

### 1. 使用 Release 模式编译

```bash
cargo build --release -p corint-server
./target/release/corint-server
```

### 2. 配置反向代理（Nginx）

```nginx
server {
    listen 80;
    server_name api.example.com;

    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location /health {
        proxy_pass http://localhost:8080/health;
        access_log off;
    }
}
```

### 3. Systemd 服务

创建 `/etc/systemd/system/corint-server.service`:

```ini
[Unit]
Description=CORINT Decision Engine Server
After=network.target

[Service]
Type=simple
User=corint
WorkingDirectory=/opt/corint
Environment="RUST_LOG=info"
Environment="CORINT_PORT=8080"
ExecStart=/opt/corint/target/release/corint-server
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

启用和启动服务:

```bash
sudo systemctl enable corint-server
sudo systemctl start corint-server
sudo systemctl status corint-server
```

### 4. Docker 部署（未来）

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p corint-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/corint-server /usr/local/bin/
COPY --from=builder /app/config /config
COPY --from=builder /app/examples/rules /rules
ENV CORINT_RULES_DIR=/rules
EXPOSE 8080
CMD ["corint-server"]
```

### 5. 监控和指标

- 使用 `/health` 端点进行健康检查
- 配置定期健康检查（如每 30 秒）
- 监控响应时间和错误率
- 设置告警（如连续 3 次健康检查失败）

### 6. 日志管理

- 使用日志聚合工具（如 ELK Stack, Loki）
- 配置日志轮转
- 保留至少 7 天的日志
- 监控错误日志数量

## 架构说明

```
┌─────────────────────────────────────────┐
│         HTTP Request                     │
│  POST /v1/decide                         │
│  { "event_data": {...} }                │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│    Axum Router + Middleware             │
│    - CORS Layer                         │
│    - Trace Layer                        │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│       REST API Handler                  │
│    - Parse JSON                         │
│    - Convert to DecisionRequest         │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│      Decision Engine (SDK)              │
│    - Load compiled rules                │
│    - Execute ruleset                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│    Pipeline Executor (Runtime)          │
│    - Execute IR instructions            │
│    - LoadField: check event_data        │
└─────────────────┬───────────────────────┘
                  │
                  ▼ (field not found)
┌─────────────────────────────────────────┐
│      Feature Executor                   │
│    - Check if registered feature        │
│    - Build SQL query                    │
│    - Query Supabase                     │
│    - Calculate feature value            │
│    - Cache in event_data                │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│         HTTP Response                   │
│  { "action": "Approve",                 │
│    "score": 35,                         │
│    "triggered_rules": [...],            │
│    "processing_time_ms": 125 }          │
└─────────────────────────────────────────┘
```

## 下一步

- 查看规则语法：`docs/dsl/rule.md`
- 查看特征定义：`examples/configs/features/`
- 查看完整示例：`examples/supabase_feature_example.rs`
- 开发文档：`docs/DEV_GUIDE.md`

## 许可证

Elastic-2.0

