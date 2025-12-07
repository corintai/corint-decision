# CORINT Server Quick Start

快速启动 CORINT Decision Engine HTTP Server。

## 启动服务器

### 1. 使用默认配置启动

```bash
cd /Users/bryanzh/Workspace/corint/corint-decision
cargo run -p corint-server
```

服务器将在 `http://127.0.0.1:8080` 上启动。

### 2. 使用自定义配置

创建 `config/server.yaml`:

```yaml
host: "0.0.0.0"
port: 8080
rules_dir: "examples/rules"
enable_metrics: true
enable_tracing: true
log_level: "info"
```

然后启动：

```bash
cargo run --bin corint-server
```

### 3. 使用环境变量

```bash
CORINT_PORT=9090 cargo run --bin corint-server
```

## 测试 API

### 使用 cURL

#### 健康检查

```bash
curl http://localhost:8080/health
```

#### 执行决策

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

## 与 Supabase Feature 集成

如果需要使用 Supabase 进行特征计算：

1. 确保 Supabase 数据库已配置（参见 `docs/schema/postgres-examples.sql`）

2. 在 `examples/rules/` 目录中添加使用特征的规则文件（如 `supabase_feature_ruleset.yaml`）

3. 启动服务器：

```bash
cargo run --bin corint-server
```

4. 发送包含事件数据的请求：

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

服务器将自动：
- 执行规则
- 当规则引用特征时，从 Supabase 计算特征
- 缓存特征值供后续使用
- 返回决策结果

## API 端点

### GET `/health`

健康检查端点。

**响应:**

```json
{
  "status": "healthy",
  "version": "0.1.0"
}
```

### POST `/v1/decide`

执行决策规则。

**请求体:**

```json
{
  "event_data": {
    "user_id": "user_001",
    "event.type": "transaction",
    ...
  }
}
```

**响应:**

```json
{
  "action": "Approve",
  "score": 35,
  "triggered_rules": ["high_value_transaction"],
  "explanation": "Transaction approved",
  "processing_time_ms": 125
}
```


## 日志配置

使用 `RUST_LOG` 环境变量控制日志级别：

```bash
# Info 级别（默认）
RUST_LOG=info cargo run --bin corint-server

# Debug 级别
RUST_LOG=debug cargo run --bin corint-server

# 特定模块的 Debug 级别
RUST_LOG=corint_server=debug,corint_runtime=info cargo run --bin corint-server
```

## 故障排查

### 服务器无法启动

1. 检查端口是否被占用：`lsof -i :8080`
2. 检查规则目录是否存在：`ls -la examples/rules`
3. 查看详细日志：`RUST_LOG=debug cargo run --bin corint-server`

### 规则未加载

1. 确保规则文件扩展名为 `.yaml` 或 `.yml`
2. 检查规则文件语法是否正确
3. 查看服务器启动日志中的规则加载信息

### 特征计算失败

1. 确保 Supabase 数据库连接正常
2. 检查 `examples/configs/datasources/supabase_events.yaml` 配置
3. 查看日志中的 SQL 查询语句和错误信息

## 生产部署建议

1. 使用 release 模式编译：`cargo build --release --bin corint-server`
2. 配置反向代理（如 Nginx）
3. 启用 HTTPS
4. 配置日志轮转
5. 设置健康检查和自动重启
6. 监控指标和性能

## 下一步

- 查看完整文档：`crates/corint-server/README.md`
- 了解规则语法：`docs/dsl/rule.md`
- 查看特征计算示例：`examples/supabase_feature_example.rs`

