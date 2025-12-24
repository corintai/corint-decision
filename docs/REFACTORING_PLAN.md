# Corint Decision 代码重构方案

## 一、总体分析

| 文件 | 行数 | 问题 | 优先级 |
|------|------|------|--------|
| `decision_engine.rs` | 3643 | 混合了类型定义、引擎实现、测试 | **P0** |
| `pipeline_executor.rs` | 1696 | 指令执行逻辑冗长，混合测试 | **P1** |
| `feature/executor.rs` | 1407 | 特征计算逻辑复杂，混合测试 | **P1** |
| `context.rs` | 1210 | 系统变量构建逻辑冗长 | **P2** |
| `pipeline_parser.rs` | 1229 | 解析逻辑复杂，混合测试 | **P2** |

## 二、优先级 P0：`decision_engine.rs` (3643行 → 拆分为3-4个文件)

### 2.1 拆分方案

**原文件结构分析：**
```rust
// Lines 1-145: 类型定义
pub struct DecisionOptions { ... }
pub struct DecisionRequest { ... }
pub struct DecisionResponse { ... }

// Lines 168-3173: DecisionEngine 实现
pub struct DecisionEngine { ... }
impl DecisionEngine { ... } // ~3000 行

// Lines 3174-3643: 测试代码 (~470行)
mod tests { ... }
```

**新文件结构：**

```
crates/corint-sdk/src/
├── decision_engine/
│   ├── mod.rs                 (~100行)  - 导出和顶层接口
│   ├── types.rs               (~150行)  - DecisionOptions, DecisionRequest 等
│   ├── engine.rs              (~800行)  - DecisionEngine 核心实现
│   ├── api_config_loader.rs    (~200行)  - API配置加载逻辑
│   └── tests.rs               (~500行)  - 所有单元测试
└── decision_engine.rs          (删除，改为 mod.rs 的重导出)
```

### 2.2 详细拆分内容

#### `types.rs` (150行)
```rust
//! Request/Response types for DecisionEngine

pub struct DecisionOptions { ... }
pub struct DecisionRequest { ... }
pub struct DecisionResponse { ... }
pub struct InternalMetrics { ... }
```

#### `engine.rs` (800行)
```rust
//! Core DecisionEngine implementation

pub struct DecisionEngine {
    // 只保留核心字段
    programs: HashMap<String, Arc<Program>>,
    registry: Option<PipelineRegistry>,
    pipeline_executor: Arc<PipelineExecutor>,
    // ...
}

impl DecisionEngine {
    // 保留核心方法
    pub async fn execute(&self, request: DecisionRequest) -> Result<DecisionResponse> { ... }
    pub async fn execute_pipeline(&self, ...) -> Result<...> { ... }
    pub fn get_program(&self, id: &str) -> Option<Arc<Program>> { ... }

    // 移除到单独文件的方法:
    // - load_api_configs() -> api_config_loader.rs
    // - 任何超过200行的内部方法需要进一步拆分
}
```

#### `api_config_loader.rs` (200行)
```rust
//! API Configuration Loader

pub struct ApiConfigLoader;

impl ApiConfigLoader {
    pub fn load_from_directory(dir: &Path) -> Result<Vec<ApiConfig>> { ... }
    pub fn load_from_configs(configs: &[String]) -> Result<Vec<ApiConfig>> { ... }
}
```

#### `tests.rs` (500行)
```rust
//! Unit tests for DecisionEngine

#[cfg(test)]
mod decision_engine_tests {
    // 所有测试代码
}
```

---

## 三、优先级 P1：`pipeline_executor.rs` (1696行 → 拆分为4个文件)

### 3.1 拆分方案

```
crates/corint-runtime/src/engine/
├── mod.rs                    (~50行)
├── pipeline_executor.rs        (~600行)  - 核心流程控制
├── instruction_handlers.rs    (~400行)  - 指令处理逻辑
├── operation_handlers.rs      (~300行)  - 运算操作
└── tests.rs                  (~400行)  - 测试代码
```

### 3.2 详细拆分内容

#### `instruction_handlers.rs` (400行)
```rust
//! Pipeline instruction handlers

pub struct InstructionHandler<'a> {
    ctx: &'a mut ExecutionContext,
    executor: &'a PipelineExecutor,
}

impl<'a> InstructionHandler<'a> {
    pub fn execute_load_field(&mut self, path: &[String]) -> Result<Value> { ... }
    pub fn execute_load_const(&mut self, value: &Value) { ... }
    pub fn execute_binary_op(&mut self, op: &Operator) -> Result<Value> { ... }
    pub fn execute_jump(&mut self, offset: isize) { ... }
    pub fn execute_check_event_type(&mut self, expected: &str) -> Result<()> { ... }
    pub fn execute_call_ruleset(&mut self, ruleset_id: &str) { ... }
    pub fn execute_call_feature(&mut self, ...) -> Result<Value> { ... }
    pub fn execute_call_llm(&mut self, ...) -> Result<Value> { ... }
    pub fn execute_call_service(&mut self, ...) -> Result<Value> { ... }
    pub fn execute_call_external_api(&mut self, ...) -> Result<Value> { ... }
    // ... 其他指令
}
```

#### `operation_handlers.rs` (300行)
```rust
//! Arithmetic and comparison operations

pub struct OperationHandler;

impl OperationHandler {
    pub fn execute_binary_op(left: &Value, op: &Operator, right: &Value) -> Result<Value> { ... }
    pub fn execute_compare(left: &Value, op: &Operator, right: &Value) -> Result<bool> { ... }
    pub fn execute_unary_op(operand: &Value, op: &UnaryOperator) -> Result<Value> { ... }
    pub fn is_truthy(value: &Value) -> bool { ... }
}
```

---

## 四、优先级 P1：`feature/executor.rs` (1407行 → 拆分为3个文件)

### 4.1 拆分方案

```
crates/corint-runtime/src/feature/
├── mod.rs                    (~50行)
├── executor.rs               (~600行)  - 核心执行逻辑
├── cache.rs                 (~300行)  - 缓存管理
└── tests.rs                  (~300行)  - 测试代码
```

### 4.2 详细拆分内容

#### `cache.rs` (300行)
```rust
//! Feature caching (L1 local, L2 Redis)

pub struct CacheEntry { ... }
pub struct FeatureCache { ... }

impl FeatureCache {
    pub fn get(&self, key: &str) -> Option<CacheEntry> { ... }
    pub fn set(&self, key: &str, value: Value, ttl: u64) { ... }
    pub fn clear(&self) { ... }
}
```

---

## 五、优先级 P2：`context.rs` (1210行 → 拆分为2个文件)

### 5.1 拆分方案

```
crates/corint-runtime/src/
├── context.rs                (~600行)  - ExecutionContext 核心逻辑
└── context_vars.rs           (~400行)  - 系统变量和环境变量构建
```

### 5.2 详细拆分内容

#### `context_vars.rs` (400行)
```rust
//! System and environment variable builders

pub struct SystemVarsBuilder;
pub struct EnvVarsBuilder;

impl SystemVarsBuilder {
    pub fn build() -> HashMap<String, Value> { ... }
}

impl EnvVarsBuilder {
    pub fn build() -> HashMap<String, Value> { ... }
    pub fn parse_env_value(value: &str) -> Value { ... }
}
```

---

## 六、优先级 P2：`pipeline_parser.rs` (1229行 → 拆分为3个文件)

### 6.1 拆分方案

```
crates/corint-parser/src/
├── pipeline_parser.rs         (~600行)  - 核心解析逻辑
├── step_parsers.rs          (~400行)  - 各步骤类型解析
└── pipeline_parser_tests.rs  (~300行)  - 测试代码
```

---

## 七、重构实施步骤

### 阶段1：P0 优先（1-2周）
1. 拆分 `decision_engine.rs`
   - 创建 `decision_engine/` 目录
   - 提取类型定义到 `types.rs`
   - 提取 API 配置加载到 `api_config_loader.rs`
   - 提取测试到 `tests.rs`
   - 重构主文件到 800 行以内

### 阶段2：P1 优先（2-3周）
2. 拆分 `pipeline_executor.rs`
3. 拆分 `feature/executor.rs`

### 阶段3：P2 优先（1-2周）
4. 拆分 `context.rs`
5. 拆分 `pipeline_parser.rs`

### 阶段4：验证和优化（1周）
6. 运行所有测试确保功能不变
7. 更新文档
8. 代码审查和优化

---

## 八、预期收益

| 指标 | 重构前 | 重构后 | 改善 |
|--------|---------|---------|------|
| 最大文件行数 | 3643 | 800 | **-78%** |
| 平均文件行数 | 1585 | 350 | **-78%** |
| 文件可读性 | 差 | 优 | **大幅提升** |
| 测试维护性 | 困难 | 容易 | **大幅提升** |
| 代码复用性 | 低 | 高 | **提升** |

---

## 九、重构原则

1. **保持功能不变** - 重构不应改变任何功能行为
2. **保持测试通过** - 所有测试在重构后仍应通过
3. **减少文件大小** - 所有源文件应控制在 1000 行以内
4. **提高可读性** - 每个文件应有清晰的单一职责
5. **保持公共 API** - 外部 API 不应被破坏
6. **文档同步** - 重构后更新相关文档

