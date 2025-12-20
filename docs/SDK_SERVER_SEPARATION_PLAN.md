# SDK 统一集成方案

## 1. 设计目标

### 1.1 核心原则

**SDK 是唯一的集成入口**：无论是 HTTP Server、WASM 还是 FFI，外部只需依赖 `corint-sdk` 即可获得完整的决策引擎功能。

```
外部集成只需要:
1. 依赖 corint-sdk
2. 传入 RepositoryConfig (指定加载方式: File/Database/API/Memory)
3. 调用 build() 获得 DecisionEngine
```

### 1.2 职责划分

| 组件 | 职责 | 加载内容 |
|------|------|---------|
| **corint-server** | 应用配置 | `config/server.yaml` (host, port, log_level, llm 等) |
| **corint-repository** | 业务配置加载 | `repository/` 下所有内容 (pipelines, rules, apis, features, lists 等) |
| **corint-sdk** | 统一入口 | 接收 RepositoryConfig，调用 repository，编译执行 |

### 1.3 Repository 加载内容

```
repository/
├── registry.yaml                    # Pipeline 路由注册
├── pipelines/                       # Pipeline 定义
│   ├── fraud_detection.yaml
│   └── payment_pipeline.yaml
├── library/
│   ├── rules/                       # 规则定义
│   ├── rulesets/                    # 规则集定义
│   └── templates/                   # 决策模板
└── configs/
    ├── apis/                        # 外部 API 配置 (ipinfo, etc.)
    ├── datasources/                 # 数据源配置 (postgres, redis, etc.)
    ├── features/                    # Feature 定义
    └── lists/                       # 列表配置 (blocklist, allowlist)
```

### 1.4 三种集成方式

| 集成方式 | 依赖 | 配置方式 | 特点 |
|---------|------|---------|------|
| **HTTP Server** | `corint-sdk` | `RepositoryConfig::file_system()` 或 `database()` | 从文件/数据库加载配置 |
| **WASM** | `corint-sdk` | `RepositoryConfig::api()` | 从后端 API 加载配置 |
| **FFI** | `corint-sdk` | 任意 `RepositoryConfig` | 根据宿主环境选择 |

### 1.5 目标架构

```
┌─────────────────────────────────────────────────────────────────┐
│                      外部集成层                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ HTTP Server  │  │    WASM      │  │  FFI Binding │          │
│  │  (Axum)      │  │ (Browser/API)│  │  (Python/Go) │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                  │                  │                  │
│         │ config/server.yaml                  │                  │
│         │ (应用配置)                           │                  │
│         │                  │                  │                  │
│         └──────────────────┼──────────────────┘                  │
│                            │ 只依赖 corint-sdk                   │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    corint-sdk (统一入口)                    ││
│  │                                                              ││
│  │  DecisionEngineBuilder::new()                               ││
│  │      .with_repository(RepositoryConfig::file_system("repo"))││
│  │      .build()                                                ││
│  │                                                              ││
│  └─────────────────────────────────────────────────────────────┘│
│                            │                                     │
│                            │ 内部依赖                            │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                  corint-repository                          ││
│  │                                                              ││
│  │  加载 repository/ 下所有业务配置:                            ││
│  │  - Pipelines, Rules, Rulesets, Templates                    ││
│  │  - API configs, Datasources, Features, Lists                ││
│  │  - Registry                                                  ││
│  │                                                              ││
│  │  返回 RepositoryContent (所有加载的内容)                     ││
│  └─────────────────────────────────────────────────────────────┘│
│                            │                                     │
│                            ▼                                     │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │        corint-compiler │ corint-runtime                     ││
│  │        (规则编译)       │  (执行引擎)                        ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

## 2. Repository API 设计

### 2.1 RepositoryConfig - 配置来源

```rust
// crates/corint-repository/src/config.rs

/// 配置来源类型
#[derive(Debug, Clone)]
pub enum RepositorySource {
    /// 从文件系统加载
    FileSystem,
    /// 从数据库加载
    Database,
    /// 从 HTTP API 加载
    Api,
    /// 内存配置 (WASM 场景)
    Memory,
}

/// Repository 配置
#[derive(Debug, Clone)]
pub struct RepositoryConfig {
    /// 配置来源
    pub source: RepositorySource,
    /// 文件系统路径 (FileSystem 模式必需)
    pub base_path: Option<String>,
    /// 数据库连接 URL (Database 模式必需)
    pub database_url: Option<String>,
    /// API 地址 (Api 模式必需)
    pub api_url: Option<String>,
    /// API Key (Api 模式可选)
    pub api_key: Option<String>,
}

impl RepositoryConfig {
    /// 创建文件系统配置
    pub fn file_system(path: impl Into<String>) -> Self {
        Self {
            source: RepositorySource::FileSystem,
            base_path: Some(path.into()),
            ..Default::default()
        }
    }

    /// 创建数据库配置
    pub fn database(url: impl Into<String>) -> Self {
        Self {
            source: RepositorySource::Database,
            database_url: Some(url.into()),
            ..Default::default()
        }
    }

    /// 创建 API 配置
    pub fn api(url: impl Into<String>) -> Self {
        Self {
            source: RepositorySource::Api,
            api_url: Some(url.into()),
            ..Default::default()
        }
    }

    /// 创建内存配置 (用于 WASM)
    pub fn memory() -> Self {
        Self {
            source: RepositorySource::Memory,
            ..Default::default()
        }
    }

    /// 设置 API Key
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }
}
```

### 2.2 RepositoryContent - 加载结果

```rust
// crates/corint-repository/src/content.rs

/// Repository 加载的所有内容
#[derive(Debug, Clone, Default)]
pub struct RepositoryContent {
    /// Registry 内容 (pipeline 路由)
    pub registry: Option<String>,

    /// Pipeline 定义 (id -> yaml content)
    pub pipelines: Vec<(String, String)>,

    /// Rule 定义 (id -> yaml content)
    pub rules: Vec<(String, String)>,

    /// Ruleset 定义 (id -> yaml content)
    pub rulesets: Vec<(String, String)>,

    /// Template 定义 (id -> yaml content)
    pub templates: Vec<(String, String)>,

    /// API 配置
    pub api_configs: Vec<ApiConfig>,

    /// 数据源配置
    pub datasource_configs: Vec<DataSourceConfig>,

    /// Feature 定义
    pub feature_definitions: Vec<FeatureDefinition>,

    /// List 配置
    pub list_configs: Vec<ListConfig>,
}
```

### 2.3 RepositoryLoader - 统一加载器

```rust
// crates/corint-repository/src/loader.rs

/// 统一的 Repository 加载器
pub struct RepositoryLoader {
    config: RepositoryConfig,
}

impl RepositoryLoader {
    pub fn new(config: RepositoryConfig) -> Self {
        Self { config }
    }

    /// 加载所有内容
    pub async fn load_all(&self) -> RepositoryResult<RepositoryContent> {
        match self.config.source {
            RepositorySource::FileSystem => self.load_from_filesystem().await,
            RepositorySource::Database => self.load_from_database().await,
            RepositorySource::Api => self.load_from_api().await,
            RepositorySource::Memory => Ok(RepositoryContent::default()),
        }
    }

    #[cfg(feature = "file-system")]
    async fn load_from_filesystem(&self) -> RepositoryResult<RepositoryContent> {
        let base_path = self.config.base_path.as_ref()
            .ok_or_else(|| RepositoryError::Config("base_path required".into()))?;

        let repo = FileSystemRepository::new(base_path)?;
        let mut content = RepositoryContent::default();

        // 1. 加载 registry
        if let Ok(registry) = repo.load_registry().await {
            content.registry = Some(registry);
        }

        // 2. 加载 pipelines
        for id in repo.list_pipelines().await? {
            let (_, yaml) = repo.load_pipeline(&id).await?;
            content.pipelines.push((id, yaml));
        }

        // 3. 加载 rules
        for id in repo.list_rules().await? {
            let (_, yaml) = repo.load_rule(&id).await?;
            content.rules.push((id, yaml));
        }

        // 4. 加载 rulesets
        for id in repo.list_rulesets().await? {
            let (_, yaml) = repo.load_ruleset(&id).await?;
            content.rulesets.push((id, yaml));
        }

        // 5. 加载 templates
        for id in repo.list_templates().await? {
            let (_, yaml) = repo.load_template(&id).await?;
            content.templates.push((id, yaml));
        }

        // 6. 加载 configs/apis/
        content.api_configs = self.load_api_configs(base_path).await?;

        // 7. 加载 configs/datasources/
        content.datasource_configs = self.load_datasource_configs(base_path).await?;

        // 8. 加载 configs/features/
        content.feature_definitions = self.load_feature_definitions(base_path).await?;

        // 9. 加载 configs/lists/
        content.list_configs = self.load_list_configs(base_path).await?;

        Ok(content)
    }
}
```

## 3. SDK API 设计

### 3.1 DecisionEngineBuilder - 统一入口

```rust
// crates/corint-sdk/src/builder.rs

use corint_repository::{RepositoryConfig, RepositoryContent, RepositoryLoader};

pub struct DecisionEngineBuilder {
    /// Repository 配置
    repository_config: Option<RepositoryConfig>,

    /// 手动添加的内容 (Memory 模式或补充)
    manual_content: RepositoryContent,

    /// 运行时配置
    enable_metrics: bool,
    enable_tracing: bool,

    /// 预创建的组件
    feature_executor: Option<Arc<FeatureExecutor>>,
    list_service: Option<Arc<ListService>>,
}

impl DecisionEngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    // ========== Repository 配置 ==========

    /// 设置 Repository 配置
    ///
    /// # Example
    /// ```rust
    /// // 从文件系统加载
    /// let engine = DecisionEngineBuilder::new()
    ///     .with_repository(RepositoryConfig::file_system("repository"))
    ///     .build()
    ///     .await?;
    ///
    /// // 从数据库加载
    /// let engine = DecisionEngineBuilder::new()
    ///     .with_repository(RepositoryConfig::database("postgresql://..."))
    ///     .build()
    ///     .await?;
    ///
    /// // WASM 内存模式
    /// let engine = DecisionEngineBuilder::new()
    ///     .with_repository(RepositoryConfig::memory())
    ///     .add_pipeline_content("main", yaml_from_js)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_repository(mut self, config: RepositoryConfig) -> Self {
        self.repository_config = Some(config);
        self
    }

    // ========== 手动添加内容 (Memory 模式或补充) ==========

    /// 添加 Pipeline 内容
    pub fn add_pipeline_content(mut self, id: impl Into<String>, content: impl Into<String>) -> Self {
        self.manual_content.pipelines.push((id.into(), content.into()));
        self
    }

    /// 添加 Rule 内容
    pub fn add_rule_content(mut self, id: impl Into<String>, content: impl Into<String>) -> Self {
        self.manual_content.rules.push((id.into(), content.into()));
        self
    }

    /// 添加 API 配置
    pub fn add_api_config(mut self, config: ApiConfig) -> Self {
        self.manual_content.api_configs.push(config);
        self
    }

    /// 添加数据源配置
    pub fn add_datasource_config(mut self, config: DataSourceConfig) -> Self {
        self.manual_content.datasource_configs.push(config);
        self
    }

    /// 添加 Feature 定义
    pub fn add_feature_definition(mut self, feature: FeatureDefinition) -> Self {
        self.manual_content.feature_definitions.push(feature);
        self
    }

    /// 添加 List 配置
    pub fn add_list_config(mut self, config: ListConfig) -> Self {
        self.manual_content.list_configs.push(config);
        self
    }

    // ========== 运行时配置 ==========

    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.enable_metrics = enable;
        self
    }

    pub fn enable_tracing(mut self, enable: bool) -> Self {
        self.enable_tracing = enable;
        self
    }

    // ========== 构建 ==========

    pub async fn build(self) -> Result<DecisionEngine, SdkError> {
        // 1. 从 Repository 加载内容
        let loaded_content = if let Some(repo_config) = &self.repository_config {
            let loader = RepositoryLoader::new(repo_config.clone());
            loader.load_all().await?
        } else {
            RepositoryContent::default()
        };

        // 2. 合并手动内容
        let final_content = self.merge_content(loaded_content);

        // 3. 创建运行时组件 (FeatureExecutor, ListService, etc.)
        let runtime_components = self.create_runtime_components(&final_content).await?;

        // 4. 编译规则
        let compiled = self.compile(&final_content)?;

        // 5. 创建 DecisionEngine
        Ok(DecisionEngine::new(compiled, runtime_components))
    }
}
```

### 3.2 SDK Re-exports

```rust
// crates/corint-sdk/src/lib.rs

// Re-export repository types for external use
pub use corint_repository::{RepositoryConfig, RepositorySource};

// SDK types
pub use builder::DecisionEngineBuilder;
pub use engine::DecisionEngine;
pub use request::DecisionRequest;
pub use response::DecisionResponse;
```

## 4. Server 简化

### 4.1 Server 只加载应用配置

```rust
// crates/corint-server/src/config.rs

/// 服务器配置 (只包含应用配置)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 服务器地址
    pub host: String,

    /// 服务器端口
    pub port: u16,

    /// 日志级别
    pub log_level: String,

    /// 启用 metrics
    pub enable_metrics: bool,

    /// 启用 tracing
    pub enable_tracing: bool,

    /// Repository 配置 (传递给 SDK)
    pub repository: RepositoryType,

    /// 数据库 URL (用于结果持久化)
    pub database_url: Option<String>,

    /// LLM 配置
    pub llm: Option<LLMConfig>,
}
```

### 4.2 Server main.rs 简化

```rust
// crates/corint-server/src/main.rs

use corint_sdk::{DecisionEngineBuilder, RepositoryConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 加载应用配置 (config/server.yaml)
    let server_config = ServerConfig::load()?;

    // 2. 初始化日志
    init_logging(&server_config.log_level);

    // 3. 构建 DecisionEngine (SDK 负责加载所有业务配置)
    let repo_config = match &server_config.repository {
        RepositoryType::FileSystem { path } => RepositoryConfig::file_system(path),
        RepositoryType::Database { url, .. } => RepositoryConfig::database(url),
        RepositoryType::Api { base_url, api_key } => {
            let config = RepositoryConfig::api(base_url);
            if let Some(key) = api_key {
                config.with_api_key(key)
            } else {
                config
            }
        }
    };

    let engine = DecisionEngineBuilder::new()
        .with_repository(repo_config)
        .enable_metrics(server_config.enable_metrics)
        .enable_tracing(server_config.enable_tracing)
        .build()
        .await?;

    // 4. 启动 HTTP 服务
    let app = create_router(Arc::new(engine));
    let addr = format!("{}:{}", server_config.host, server_config.port);

    info!("Starting server on {}", addr);
    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
```

## 5. Feature 设计

### 5.1 Repository Features

```toml
# crates/corint-repository/Cargo.toml

[features]
default = ["file-system"]

# 存储后端
file-system = []                    # 文件系统加载
postgres = ["dep:sqlx"]             # PostgreSQL 加载
api = ["dep:reqwest"]               # HTTP API 加载

# 配置类型解析
config-loader = []                  # 加载 configs/ 目录
```

### 5.2 SDK Features

```toml
# crates/corint-sdk/Cargo.toml

[features]
default = ["file-loader"]

# 配置加载 (通过 repository)
file-loader = ["corint-repository/file-system", "corint-repository/config-loader"]
database-loader = ["corint-repository/postgres", "corint-repository/config-loader"]
api-loader = ["corint-repository/api", "corint-repository/config-loader"]

# 运行时特性
sqlx = ["dep:sqlx", "corint-runtime/sqlx"]

# WASM 兼容 (使用 api-loader，排除文件系统依赖)
wasm = ["api-loader"]
```

## 6. 使用示例

### 6.1 HTTP Server

```rust
use corint_sdk::{DecisionEngineBuilder, RepositoryConfig};

// 从文件系统加载所有配置
let engine = DecisionEngineBuilder::new()
    .with_repository(RepositoryConfig::file_system("repository"))
    .enable_metrics(true)
    .build()
    .await?;

// 或从数据库加载
let engine = DecisionEngineBuilder::new()
    .with_repository(RepositoryConfig::database("postgresql://localhost/corint"))
    .build()
    .await?;
```

### 6.2 WASM

```rust
use corint_sdk::{DecisionEngineBuilder, RepositoryConfig};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmEngine {
    engine: DecisionEngine,
}

#[wasm_bindgen]
impl WasmEngine {
    /// 从后端 API 加载配置创建引擎
    #[wasm_bindgen(constructor)]
    pub async fn new(api_url: &str, api_key: Option<String>) -> Result<WasmEngine, JsValue> {
        let config = RepositoryConfig::api(api_url);
        let config = if let Some(key) = api_key {
            config.with_api_key(key)
        } else {
            config
        };

        let engine = DecisionEngineBuilder::new()
            .with_repository(config)
            .build()
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(WasmEngine { engine })
    }

    /// 执行决策
    #[wasm_bindgen]
    pub async fn decide(&self, event_json: &str) -> Result<String, JsValue> {
        let event: HashMap<String, Value> = serde_json::from_str(event_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let request = DecisionRequest::new(event);
        let response = self.engine.decide(request).await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        serde_json::to_string(&response)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
```

JavaScript 调用:

```javascript
import init, { WasmEngine } from 'corint-wasm';

await init();

// 从后端 API 加载配置
const engine = await new WasmEngine(
    "https://api.example.com/repository",
    "optional-api-key"
);

// 执行决策
const event = { amount: 1000, country: "US" };
const result = await engine.decide(JSON.stringify(event));
console.log(JSON.parse(result));
```

### 6.3 FFI (Python)

```python
import corint

# 从文件系统
engine = corint.Engine.from_filesystem("repository")

# 从数据库
engine = corint.Engine.from_database("postgresql://localhost/corint")

# 执行决策
result = engine.decide({"amount": 1000, "country": "US"})
```

## 7. 实施步骤

### Phase 1: Repository 增强 (4-5 小时)

1. [ ] **创建配置类型** (`crates/corint-repository/src/config.rs`)
   - 添加 `RepositorySource` 枚举
   - 添加 `RepositoryConfig` 结构体和便捷方法

2. [ ] **创建内容类型** (`crates/corint-repository/src/content.rs`)
   - 添加 `RepositoryContent` 结构体
   - 添加 `ApiConfig`, `DataSourceConfig`, `FeatureDefinition`, `ListConfig` 类型

3. [ ] **创建统一加载器** (`crates/corint-repository/src/loader.rs`)
   - 实现 `RepositoryLoader` 结构体
   - 实现 `load_all()` 方法
   - 实现各配置目录的加载逻辑

4. [ ] **扩展 Repository trait**
   - 添加 `load_api_configs()`, `load_datasources()`, `load_features()`, `load_lists()` 方法

5. [ ] **添加单元测试**

### Phase 2: SDK 增强 (3-4 小时)

1. [ ] **Re-export Repository 类型** (`crates/corint-sdk/src/lib.rs`)
   - `pub use corint_repository::{RepositoryConfig, RepositorySource};`

2. [ ] **增强 Builder API** (`crates/corint-sdk/src/builder.rs`)
   - 添加 `with_repository()` 方法
   - 添加手动内容添加方法
   - 更新 `build()` 使用 `RepositoryLoader`

3. [ ] **删除 Server 中的加载逻辑**
   - 删除 `repository_loader.rs`
   - 简化 `main.rs`

4. [ ] **添加集成测试**

### Phase 3: Server 简化 (2 小时)

1. [ ] **简化 config.rs**
   - 只保留应用配置
   - Repository 配置转换为 SDK 类型

2. [ ] **简化 main.rs**
   - 使用 SDK 的 `with_repository()` API
   - 删除冗余的加载逻辑

### Phase 4: WASM 支持 (2-3 小时)

1. [ ] **添加 WASM feature** (基于 api-loader)
2. [ ] **确保 API 加载器兼容 WASM** (使用 wasm-compatible HTTP client)
3. [ ] **创建 WASM 绑定示例** (corint-wasm crate)

### Phase 5: 文档和示例 (1-2 小时)

1. [ ] **更新 SDK README**
2. [ ] **添加使用示例**

## 8. 验收标准

- [ ] `RepositoryConfig` 和 `RepositoryLoader` 在 `corint-repository` 中实现
- [ ] SDK re-export `RepositoryConfig`，提供 `with_repository()` API
- [ ] Repository 能加载所有业务配置 (pipelines, rules, apis, features, lists 等)
- [ ] Server 只加载 `config/server.yaml`，业务配置由 SDK 处理
- [ ] WASM 通过 API 加载配置正常工作
- [ ] 所有现有测试通过
