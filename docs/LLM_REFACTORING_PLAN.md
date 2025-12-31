# CORINT LLM 重构执行方案

> **创建日期**: 2025-12-31
> **状态**: 待执行
> **预计工期**: 3天
> **注意**: 项目尚未发布，无需考虑向后兼容性

---

## 📋 方案概述

### 背景

CORINT是一个定位于**实时决策引擎**的系统，延迟要求通常在100-300ms以内。然而，LLM推理的典型延迟为2-5秒，在实时pipeline中同步调用LLM是不现实的。

当前代码中，LLM相关的DSL定义（reason步骤）已经在AST、Parser、IR中定义，但Compiler未实现，说明团队已经意识到这个架构问题。

### 目标

1. ❌ 从DSL中移除LLM实时调用（reason步骤）
2. ✅ 创建独立的 `corint-llm` crate，专注于离线场景
3. ✅ 在SDK中提供代码生成接口
4. ✅ 支持基于自然语言描述自动生成Rule/Ruleset/Pipeline配置

### 架构调整

**之前（实时调用 - 不可行）**：
```
Pipeline YAML → Parser → Compiler → Runtime → CallLLM (阻塞2-5秒)
```

**之后（离线生成 - 可行）**：
```
用户自然语言描述 → LLM Provider → 生成YAML配置 → 保存到repository
```

### LLM的新定位

| 场景 | 使用方式 | 延迟容忍度 |
|------|---------|-----------|
| ❌ Pipeline实时步骤 | 同步调用LLM | 需要 <300ms，实际2-5s |
| ✅ 规则代码生成 | 开发时生成配置 | 无限制 |
| ✅ 离线分析 | 批处理历史数据 | 分钟级 |
| ✅ 人工审核辅助 | 后台生成建议 | 秒级 |
| ✅ 规则优化建议 | 分析现有规则 | 无限制 |

---

## 📦 Phase 1: 创建 corint-llm Crate

### 1.1 Crate 目录结构

```
crates/corint-llm/
├── Cargo.toml
├── README.md
├── examples/
│   ├── generate_rule.rs
│   ├── generate_pipeline.rs
│   └── batch_analysis.rs
└── src/
    ├── lib.rs                 # 公共接口导出
    ├── error.rs               # 错误类型
    ├── client.rs              # LLMClient trait和类型
    ├── cache.rs               # LLM响应缓存
    ├── provider/
    │   ├── mod.rs             # Provider trait定义
    │   ├── openai.rs          # OpenAI provider (GPT-4, O1, O3)
    │   ├── anthropic.rs       # Anthropic provider (Claude)
    │   ├── gemini.rs          # Google Gemini provider
    │   ├── deepseek.rs        # DeepSeek provider
    │   └── mock.rs            # Mock provider (测试用)
    └── generator/             # 代码生成相关
        ├── mod.rs
        ├── rule_generator.rs      # Rule生成器
        ├── ruleset_generator.rs   # Ruleset生成器
        ├── pipeline_generator.rs  # Pipeline生成器
        ├── api_config_generator.rs # API配置生成器
        ├── prompt_templates.rs    # 提示词模板
        ├── yaml_extractor.rs      # 从LLM响应中提取YAML
        └── validator.rs           # 生成内容验证
```

### 1.2 Cargo.toml 配置

```toml
[package]
name = "corint-llm"
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
description = "LLM integration for CORINT - Code generation and AI-powered analysis"
keywords = ["llm", "ai", "code-generation", "decision-engine"]

[dependencies]
# 核心依赖
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
async-trait = "0.1"
thiserror = "1.0"
anyhow = "1.0"

# 异步运行时
tokio = { version = "1.0", features = ["full"] }

# HTTP客户端
reqwest = { version = "0.11", features = ["json"] }

# 缓存
dashmap = "5.5"  # 并发HashMap for cache

# 日志
tracing = "0.1"

# 可选：结构化输出验证
jsonschema = { version = "0.17", optional = true }

# 可选：CORINT解析器（用于验证生成的YAML）
corint-parser = { path = "../corint-parser", optional = true }

[dev-dependencies]
tokio-test = "0.4"

[features]
default = []
validation = ["jsonschema", "corint-parser"]  # 生成内容验证
```

### 1.3 需要移动的代码

| 源文件 | 目标文件 | 工作量 | 说明 |
|--------|---------|--------|------|
| `corint-runtime/src/llm/client.rs` | `corint-llm/src/client.rs` | 30分钟 | 直接复制，移除runtime依赖 |
| `corint-runtime/src/llm/cache.rs` | `corint-llm/src/cache.rs` | 20分钟 | 直接复制 |
| `corint-runtime/src/llm/provider.rs` | `corint-llm/src/provider/*.rs` | 2小时 | 拆分为多个文件 |
| - | `corint-llm/src/error.rs` | 30分钟 | 新建：LLM专用错误类型 |
| - | `corint-llm/src/generator/*.rs` | 4-6小时 | 新建：代码生成逻辑 |

### 1.4 公共接口设计 (lib.rs)

```rust
//! CORINT LLM Integration
//!
//! This crate provides LLM integration for CORINT decision engine, focusing on:
//! - Code generation: Generate rules, rulesets, and pipelines from natural language
//! - Offline analysis: Batch analysis of historical data
//! - Development assistance: Rule optimization suggestions
//!
//! **Note**: This crate is NOT for real-time pipeline execution.
//! LLM calls have 2-5 second latency, unsuitable for real-time decisions.

// Re-export core types
pub use client::{LLMClient, LLMRequest, LLMResponse};
pub use cache::{LLMCache, InMemoryLLMCache};
pub use error::{LLMError, Result};

// Re-export providers
pub use provider::{
    LLMProvider,
    OpenAIProvider,
    AnthropicProvider,
    GeminiProvider,
    DeepSeekProvider,
    MockProvider,
};

// Re-export generators
pub use generator::{
    RuleGenerator,
    RulesetGenerator,
    PipelineGenerator,
    ApiConfigGenerator,
    GeneratorOptions,
};

pub mod client;
pub mod cache;
pub mod error;
pub mod provider;
pub mod generator;
```

---

## 🔧 Phase 2: 在SDK中集成代码生成功能

### 2.1 SDK新模块结构

```
crates/corint-sdk/src/
├── generator/              # 新增：代码生成模块
│   ├── mod.rs              # 主导出
│   ├── builder.rs          # CorintGenerator构建器
│   ├── rule_gen.rs         # Rule生成封装
│   ├── ruleset_gen.rs      # Ruleset生成封装
│   ├── pipeline_gen.rs     # Pipeline生成封装
│   ├── config_gen.rs       # API/Service配置生成
│   ├── flow_gen.rs         # 完整决策流程生成
│   └── types.rs            # 生成结果类型
```

### 2.2 SDK公共接口设计

```rust
// crates/corint-sdk/src/generator/mod.rs

use corint_llm::{LLMClient, LLMProvider};
use std::path::PathBuf;
use chrono::{DateTime, Utc};

/// CORINT代码生成器
///
/// 使用LLM根据自然语言描述生成规则配置
pub struct CorintGenerator {
    llm_client: Box<dyn LLMClient>,
    output_dir: PathBuf,
    options: GeneratorOptions,
}

/// 生成器配置选项
#[derive(Debug, Clone)]
pub struct GeneratorOptions {
    /// 是否验证生成的YAML
    pub validate: bool,
    /// 是否自动保存到文件
    pub auto_save: bool,
    /// 是否自动格式化
    pub format: bool,
    /// 语言偏好（用于prompt）
    pub language: String,
    /// 是否生成注释
    pub with_comments: bool,
    /// 生成失败时的重试次数
    pub max_retries: u32,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            validate: true,
            auto_save: true,
            format: true,
            language: "en".to_string(),
            with_comments: true,
            max_retries: 3,
        }
    }
}

impl CorintGenerator {
    /// 创建生成器
    pub fn new(llm_client: Box<dyn LLMClient>, output_dir: PathBuf) -> Self {
        Self {
            llm_client,
            output_dir,
            options: GeneratorOptions::default(),
        }
    }

    /// 使用自定义选项创建生成器
    pub fn with_options(
        llm_client: Box<dyn LLMClient>,
        output_dir: PathBuf,
        options: GeneratorOptions,
    ) -> Self {
        Self {
            llm_client,
            output_dir,
            options,
        }
    }

    /// 根据描述生成Rule
    ///
    /// # Example
    /// ```no_run
    /// let rule = generator.generate_rule(
    ///     "检测连续3次登录失败，标记为账户接管风险，给80分"
    /// ).await?;
    /// ```
    pub async fn generate_rule(&self, description: &str) -> Result<GeneratedRule>;

    /// 根据描述生成Ruleset
    pub async fn generate_ruleset(&self, description: &str) -> Result<GeneratedRuleset>;

    /// 根据描述生成Pipeline
    pub async fn generate_pipeline(&self, description: &str) -> Result<GeneratedPipeline>;

    /// 根据描述生成完整的决策流程（rule + ruleset + pipeline）
    ///
    /// # Example
    /// ```no_run
    /// let flow = generator.generate_decision_flow(
    ///     "创建支付欺诈检测流程：
    ///      1. 检查IP地理位置异常
    ///      2. 检查交易金额超过历史均值3倍
    ///      3. 检查设备指纹首次出现
    ///      满足2个以上条件则拒绝交易"
    /// ).await?;
    /// ```
    pub async fn generate_decision_flow(&self, description: &str) -> Result<GeneratedFlow>;

    /// 根据API文档生成API配置
    pub async fn generate_api_config(&self, api_spec: &str) -> Result<GeneratedApiConfig>;

    /// 批量生成：从文件读取多个描述，生成多个配置
    pub async fn generate_batch(&self, descriptions_file: PathBuf) -> Result<Vec<GeneratedItem>>;
}

/// 生成的Rule
#[derive(Debug, Clone)]
pub struct GeneratedRule {
    pub id: String,
    pub yaml_content: String,
    pub file_path: PathBuf,
    pub metadata: GenerationMetadata,
}

impl GeneratedRule {
    /// 保存到文件
    pub fn save(&self) -> Result<()>;

    /// 验证YAML语法
    pub fn validate(&self) -> Result<()>;
}

/// 生成的Ruleset
#[derive(Debug, Clone)]
pub struct GeneratedRuleset {
    pub id: String,
    pub yaml_content: String,
    pub file_path: PathBuf,
    pub rules: Vec<String>,  // 引用的rule IDs
    pub metadata: GenerationMetadata,
}

/// 生成的Pipeline
#[derive(Debug, Clone)]
pub struct GeneratedPipeline {
    pub id: String,
    pub yaml_content: String,
    pub file_path: PathBuf,
    pub steps: Vec<String>,  // Step IDs
    pub metadata: GenerationMetadata,
}

/// 生成的完整决策流程
#[derive(Debug, Clone)]
pub struct GeneratedFlow {
    pub pipeline: GeneratedPipeline,
    pub rulesets: Vec<GeneratedRuleset>,
    pub rules: Vec<GeneratedRule>,
    pub api_configs: Vec<GeneratedApiConfig>,
}

impl GeneratedFlow {
    /// 保存所有生成的文件
    pub fn save_all(&self) -> Result<()>;

    /// 生成项目总结报告
    pub fn summary(&self) -> String;
}

/// 生成的API配置
#[derive(Debug, Clone)]
pub struct GeneratedApiConfig {
    pub name: String,
    pub yaml_content: String,
    pub file_path: PathBuf,
    pub endpoints: Vec<String>,
    pub metadata: GenerationMetadata,
}

/// 生成元数据（用于审计和追踪）
#[derive(Debug, Clone)]
pub struct GenerationMetadata {
    /// 生成时间
    pub generated_at: DateTime<Utc>,
    /// 使用的模型
    pub model: String,
    /// 使用的provider
    pub provider: String,
    /// 用户原始描述
    pub user_description: String,
    /// 实际发送给LLM的prompt
    pub actual_prompt: String,
    /// 消耗的tokens
    pub tokens_used: u32,
    /// 生成耗时（毫秒）
    pub generation_time_ms: u64,
}
```

### 2.3 使用示例

#### 示例1: 生成单个Rule

```rust
use corint_sdk::generator::{CorintGenerator, GeneratorOptions};
use corint_llm::OpenAIProvider;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 创建LLM provider
    let api_key = std::env::var("OPENAI_API_KEY")?;
    let provider = OpenAIProvider::new(api_key);

    // 2. 创建生成器
    let generator = CorintGenerator::new(
        Box::new(provider),
        PathBuf::from("./repository")
    );

    // 3. 根据自然语言描述生成规则
    let rule = generator.generate_rule(
        "检测用户在1小时内登录失败超过3次，标记为账户接管风险，分数80分"
    ).await?;

    // 4. 查看生成结果
    println!("Generated rule: {}", rule.id);
    println!("Saved to: {}", rule.file_path.display());
    println!("Tokens used: {}", rule.metadata.tokens_used);
    println!("\nContent:\n{}", rule.yaml_content);

    Ok(())
}
```

#### 示例2: 生成完整决策流程

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = OpenAIProvider::new(std::env::var("OPENAI_API_KEY")?);
    let generator = CorintGenerator::new(
        Box::new(provider),
        PathBuf::from("./repository")
    );

    // 生成完整的支付欺诈检测流程
    let flow = generator.generate_decision_flow(
        r#"
        创建一个支付欺诈检测决策流程：

        步骤1: 调用IP地理位置API，检查IP是否来自高风险国家
        步骤2: 查询用户历史交易，计算平均交易金额
        步骤3: 执行欺诈检测规则集：
            - 规则1: 如果IP地理位置与用户常用地址不同，且距离>1000km，+30分
            - 规则2: 如果交易金额>历史均值的3倍，+40分
            - 规则3: 如果设备指纹首次出现，+30分
            - 规则4: 如果是夜间交易(0-6点)且金额>$500，+20分
        步骤4: 路由决策：
            - 如果总分>=80，拒绝交易
            - 如果总分50-79，人工审核
            - 如果总分<50，批准交易
        "#
    ).await?;

    // 打印生成摘要
    println!("{}", flow.summary());

    // 保存所有文件
    flow.save_all()?;

    println!("\n✅ Decision flow generated successfully!");
    println!("Generated {} rules, {} rulesets, 1 pipeline",
        flow.rules.len(),
        flow.rulesets.len()
    );

    Ok(())
}
```

#### 示例3: 批量生成

```rust
// descriptions.txt:
// ---
// Rule 1: 检测账户接管 - 连续登录失败
// Rule 2: 检测信用卡欺诈 - 异常消费模式
// Rule 3: 检测洗钱 - 大额快速转账
// ---

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = AnthropicProvider::new(std::env::var("ANTHROPIC_API_KEY")?);

    let mut options = GeneratorOptions::default();
    options.language = "zh-CN".to_string();  // 中文prompt

    let generator = CorintGenerator::with_options(
        Box::new(provider),
        PathBuf::from("./repository"),
        options,
    );

    // 批量生成
    let items = generator.generate_batch(
        PathBuf::from("./descriptions.txt")
    ).await?;

    println!("Generated {} items", items.len());

    Ok(())
}
```

---

## 🗑️ Phase 3: 清理DSL中的LLM相关定义

### 3.1 需要移除的文件

```bash
# DSL文档
docs/dsl/llm.md

# 示例代码
crates/corint-runtime/examples/llm_providers.rs

# Runtime LLM模块
crates/corint-runtime/src/llm/
```

### 3.2 需要修改的代码文件

| 文件 | 修改内容 |
|------|---------|
| `corint-core/src/ast/pipeline.rs` | 移除 `StepDetails::Reason`、`PromptTemplate`、`Schema` |
| `corint-core/src/ir/instruction.rs` | 移除 `Instruction::CallLLM` 和相关测试 |
| `corint-parser/src/pipeline_parser.rs` | 移除 reason 步骤解析逻辑 |
| `corint-parser/src/pipeline/parser.rs` | 移除 reason 相关代码 |
| `corint-runtime/src/lib.rs` | 移除 `pub mod llm;` |
| `corint-runtime/src/engine/pipeline_executor.rs` | 移除 `CallLLM` 指令执行逻辑 (约25行) |
| `corint-runtime/src/engine/tests/integrations_test.rs` | 移除 `test_llm_integration()` 测试 |

### 3.3 详细清理步骤

#### 步骤1: 清理AST定义

```rust
// 文件: crates/corint-core/src/ast/pipeline.rs

// 移除以下代码段（约50行）：

// Line 216-229: 移除 Reason variant
Reason {
    /// LLM provider
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    /// Model name
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// Prompt template
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt: Option<PromptTemplate>,
    /// Output schema
    #[serde(skip_serializing_if = "Option::is_none")]
    output_schema: Option<Schema>,
}

// Line 265-270: 移除 PromptTemplate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub template: String,
}

// Line 273-290: 移除 Schema 和 SchemaProperty
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, SchemaProperty>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaProperty {
    #[serde(rename = "type")]
    pub property_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
```

#### 步骤2: 清理IR指令

```rust
// 文件: crates/corint-core/src/ir/instruction.rs

// 移除 Line 97-105: CallLLM指令
/// Call LLM for reasoning
CallLLM {
    /// LLM provider (e.g., "openai")
    provider: String,
    /// Model name (e.g., "gpt-4")
    model: String,
    /// Prompt template
    prompt: String,
},

// 移除 Line 390-409: test_llm_call 测试
#[test]
fn test_llm_call() {
    let call_llm = Instruction::CallLLM {
        provider: "openai".to_string(),
        model: "gpt-4".to_string(),
        prompt: "Analyze this event".to_string(),
    };

    if let Instruction::CallLLM {
        provider,
        model,
        prompt,
    } = call_llm
    {
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-4");
        assert_eq!(prompt, "Analyze this event");
    } else {
        panic!("Expected CallLLM instruction");
    }
}
```

#### 步骤3: 清理Parser

```rust
// 文件: crates/corint-parser/src/pipeline_parser.rs

// 搜索并移除 "reason" 相关的解析代码
// 预计在以下位置：
// - Line 259-264
// - Line 536-541

// 使用以下命令查找确切位置：
// grep -n "Reason\|reason" crates/corint-parser/src/pipeline_parser.rs
```

#### 步骤4: 清理Runtime

```bash
# 删除整个LLM模块
rm -rf crates/corint-runtime/src/llm

# 删除示例
rm crates/corint-runtime/examples/llm_providers.rs
```

```rust
// 文件: crates/corint-runtime/src/lib.rs
// 移除这一行：
pub mod llm;
```

```rust
// 文件: crates/corint-runtime/src/engine/pipeline_executor.rs
// 移除 Line 523-546: CallLLM 执行逻辑

// LLM calls
Instruction::CallLLM { prompt, model, .. } => {
    let llm_start = Instant::now();
    let value = if let Some(ref client) = self.llm_client {
        use crate::llm::LLMRequest;
        let request = LLMRequest::new(prompt.clone(), model.clone());
        match client.call(request).await {
            Ok(response) => {
                self.metrics.counter("llm_calls_success").inc();
                Value::String(response.content)
            }
            Err(e) => {
                self.metrics.counter("llm_calls_error").inc();
                Value::String(format!("LLM Error: {}", e))
            }
        }
    } else {
        Value::String("LLM not configured".to_string())
    };
    self.metrics
        .record_execution_time("llm_call", llm_start.elapsed());
    ctx.push(value);
    pc += 1;
}

// 同时移除 pipeline_executor 结构体中的 llm_client 字段
// 移除 with_llm_client() 方法
```

#### 步骤5: 清理测试

```rust
// 文件: crates/corint-runtime/src/engine/tests/integrations_test.rs
// 移除整个 test_llm_integration 测试函数
```

#### 步骤6: 更新文档引用

```bash
# 检查所有文档中对 llm.md 的引用
grep -r "llm.md" docs/

# 检查 ARCHITECTURE.md 中的引用
# 检查 DSL_DESIGN.md 中的引用
# 移除或更新这些引用
```

### 3.4 验证清理完整性

```bash
# 1. 搜索残留的LLM引用
grep -r "CallLLM" crates/
grep -r "StepDetails::Reason" crates/
grep -r "PromptTemplate" crates/

# 2. 编译检查
cargo check --all

# 3. 运行测试
cargo test --all

# 4. Clippy检查
cargo clippy --all
```

---

## 📝 Phase 4: 详细执行时间线（3天计划）

### Day 1: 创建 corint-llm Crate

**上午（09:00-12:00）**: 基础设施搭建

- [ ] **09:00-09:30** 创建crate结构和目录
  ```bash
  cargo new --lib crates/corint-llm
  mkdir -p crates/corint-llm/src/{provider,generator}
  mkdir -p crates/corint-llm/examples
  ```

- [ ] **09:30-10:30** 移动核心代码
  - 复制 `client.rs`, `cache.rs` 到新crate
  - 移除对 `corint-runtime` 的依赖
  - 更新 Cargo.toml

- [ ] **10:30-11:30** 拆分 provider.rs
  - 创建 `provider/mod.rs` (trait定义)
  - 创建 `provider/openai.rs`
  - 创建 `provider/anthropic.rs`
  - 创建 `provider/gemini.rs`
  - 创建 `provider/deepseek.rs`
  - 创建 `provider/mock.rs`

- [ ] **11:30-12:00** 创建 error.rs
  - 定义 `LLMError` 枚举
  - 定义 `Result<T>` 类型别名

**下午（14:00-18:00）**: 提示词模板和生成器

- [ ] **14:00-15:00** 设计提示词模板 (`generator/prompt_templates.rs`)
  - Rule生成提示词
  - Ruleset生成提示词
  - Pipeline生成提示词
  - API配置生成提示词

- [ ] **15:00-16:30** 实现基础生成器
  - `generator/rule_generator.rs` - RuleGenerator
  - `generator/yaml_extractor.rs` - 从LLM响应提取YAML

- [ ] **16:30-18:00** 编写测试
  - 为 MockProvider 编写测试
  - 为 RuleGenerator 编写单元测试
  - 创建示例 `examples/generate_rule.rs`

**晚上（可选，19:00-21:00）**: 完善文档

- [ ] **19:00-20:00** 编写 README.md
  - Crate用途说明
  - 快速开始指南
  - 示例代码

- [ ] **20:00-21:00** 编写API文档注释
  - 为所有公共API添加文档注释
  - 运行 `cargo doc --open` 检查

---

### Day 2: SDK集成和生成器完善

**上午（09:00-12:00）**: 完善生成器

- [ ] **09:00-10:00** 实现 RulesetGenerator
  - `generator/ruleset_generator.rs`
  - 支持生成包含多个规则的ruleset

- [ ] **10:00-11:00** 实现 PipelineGenerator
  - `generator/pipeline_generator.rs`
  - 支持生成复杂的多步骤pipeline

- [ ] **11:00-12:00** 实现 ApiConfigGenerator
  - `generator/api_config_generator.rs`
  - 根据API文档生成CORINT API配置

**下午（14:00-18:00）**: SDK集成

- [ ] **14:00-14:30** 更新SDK依赖
  ```toml
  # crates/corint-sdk/Cargo.toml
  corint-llm = { path = "../corint-llm" }
  ```

- [ ] **14:30-15:30** 创建SDK生成器模块
  - `generator/mod.rs` - 主导出
  - `generator/types.rs` - 生成结果类型
  - `generator/builder.rs` - CorintGenerator

- [ ] **15:30-16:30** 实现 CorintGenerator
  - `generate_rule()` 方法
  - `generate_ruleset()` 方法
  - `generate_pipeline()` 方法
  - `generate_decision_flow()` 方法

- [ ] **16:30-17:30** 添加验证逻辑
  - `generator/validator.rs`
  - 使用 `corint-parser` 验证生成的YAML
  - 自动保存到 repository 目录

- [ ] **17:30-18:00** 编写集成测试
  - 测试完整的生成流程
  - 验证生成的YAML能被解析

**晚上（可选，19:00-21:00）**: CLI工具

- [ ] **19:00-20:30** 创建CLI示例
  - `crates/corint-sdk/examples/generate_rule_cli.rs`
  - 支持命令行参数
  - 支持交互式输入

- [ ] **20:30-21:00** 文档和示例
  - 更新SDK README
  - 添加使用示例

---

### Day 3: 清理和文档

**上午（09:00-12:00）**: 清理DSL和Runtime

- [ ] **09:00-09:30** 清理AST定义
  - 编辑 `corint-core/src/ast/pipeline.rs`
  - 移除 `StepDetails::Reason`
  - 移除 `PromptTemplate` 和 `Schema`

- [ ] **09:30-10:00** 清理IR指令
  - 编辑 `corint-core/src/ir/instruction.rs`
  - 移除 `Instruction::CallLLM`
  - 移除相关测试

- [ ] **10:00-10:30** 清理Parser
  - 编辑 `corint-parser/src/pipeline_parser.rs`
  - 移除 reason 步骤解析逻辑

- [ ] **10:30-11:00** 清理Runtime
  ```bash
  rm -rf crates/corint-runtime/src/llm
  rm crates/corint-runtime/examples/llm_providers.rs
  ```
  - 编辑 `corint-runtime/src/lib.rs` - 移除 `pub mod llm;`
  - 编辑 `pipeline_executor.rs` - 移除 CallLLM 执行逻辑

- [ ] **11:00-11:30** 运行测试
  ```bash
  cargo test --all
  cargo check --all
  cargo clippy --all
  ```

- [ ] **11:30-12:00** 修复编译错误和警告

**下午（14:00-18:00）**: 文档更新

- [ ] **14:00-14:30** 删除LLM DSL文档
  ```bash
  rm docs/dsl/llm.md
  ```

- [ ] **14:30-15:30** 更新 ARCHITECTURE.md
  - 移除 "LLM Reasoning Step" 章节
  - 添加 "Code Generation with LLM" 章节
  - 说明新的架构：LLM用于离线生成，不用于实时pipeline

- [ ] **15:30-16:00** 更新 DSL_DESIGN.md
  - 移除 reason 步骤的所有引用
  - 更新步骤类型列表

- [ ] **16:00-17:30** 创建 CODE_GENERATION.md
  - LLM代码生成完整指南
  - 包含所有示例代码
  - 最佳实践和注意事项
  - 支持的场景和限制

- [ ] **17:30-18:00** 更新根目录 README.md
  - 更新项目描述
  - 添加代码生成功能介绍
  - 更新快速开始指南

**晚上（可选，19:00-21:00）**: 最终验证

- [ ] **19:00-20:00** 端到端测试
  - 使用真实的LLM API测试生成功能
  - 验证生成的配置能正确运行
  - 记录性能数据（tokens消耗、耗时等）

- [ ] **20:00-20:30** 代码审查
  - 检查代码风格一致性
  - 确保所有公共API有文档
  - 运行 `cargo fmt --all`

- [ ] **20:30-21:00** 准备交付
  - 生成 CHANGELOG
  - 打tag（如果需要）
  - 准备演示示例

---

## 🎯 成功验收标准

### 功能性验收

- [ ] **编译通过**: `cargo build --all --release` 无错误
- [ ] **测试通过**: `cargo test --all` 100%通过
- [ ] **Clippy检查**: `cargo clippy --all` 无警告
- [ ] **格式检查**: `cargo fmt --all -- --check` 通过

### 代码生成功能验收

- [ ] 能够根据自然语言描述生成有效的Rule YAML
- [ ] 能够根据描述生成有效的Ruleset YAML
- [ ] 能够根据描述生成有效的Pipeline YAML
- [ ] 生成的YAML能被 `corint-parser` 成功解析
- [ ] 生成的配置能在 `corint-runtime` 中正确执行
- [ ] 支持至少3个LLM provider（OpenAI、Anthropic、Gemini）

### 代码质量验收

- [ ] 测试覆盖率 >= 70%
- [ ] 所有公共API有完整的文档注释
- [ ] 无unsafe代码（除非有充分理由）
- [ ] 遵循Rust最佳实践

### 文档完整性验收

- [ ] `corint-llm/README.md` 完整清晰
- [ ] `docs/CODE_GENERATION.md` 包含完整指南
- [ ] API文档生成正常：`cargo doc --open`
- [ ] 至少提供3个完整的工作示例
- [ ] ARCHITECTURE.md 正确反映新架构

### 性能验收（参考值）

- [ ] Rule生成：< 5秒（使用GPT-4）
- [ ] Pipeline生成：< 10秒（使用GPT-4）
- [ ] 生成的YAML大小 < 10KB
- [ ] 缓存命中率 > 80%（重复请求）

---

## 📦 交付物清单

### 代码

1. ✅ **新crate**: `crates/corint-llm/`
   - 完整的LLM provider实现
   - 代码生成器实现
   - 单元测试和集成测试

2. ✅ **SDK更新**: `crates/corint-sdk/src/generator/`
   - CorintGenerator接口
   - 生成结果类型定义
   - 验证和保存逻辑

3. ✅ **示例代码**:
   - `crates/corint-llm/examples/generate_rule.rs`
   - `crates/corint-llm/examples/generate_pipeline.rs`
   - `crates/corint-sdk/examples/generate_rule_cli.rs`
   - `crates/corint-sdk/examples/generate_flow.rs`

4. ✅ **清理代码**:
   - 移除AST中的Reason定义
   - 移除IR中的CallLLM指令
   - 移除runtime中的llm模块
   - 移除parser中的reason解析

### 文档

1. ✅ **新文档**:
   - `docs/CODE_GENERATION.md` - 完整的代码生成指南
   - `crates/corint-llm/README.md` - Crate使用说明

2. ✅ **更新文档**:
   - `docs/ARCHITECTURE.md` - 反映新架构
   - `docs/DSL_DESIGN.md` - 移除LLM步骤
   - `README.md` - 添加代码生成功能介绍

3. ✅ **删除文档**:
   - `docs/dsl/llm.md` - 移除实时LLM调用文档

### 测试

1. ✅ **单元测试**: 覆盖所有生成器
2. ✅ **集成测试**: 端到端生成流程
3. ✅ **回归测试**: 确保现有功能不受影响

---

## 💡 未来扩展方向

### Phase 5+: 高级功能（可选）

#### 5.1 智能优化

- **规则优化建议**: 分析现有规则，建议性能优化
- **规则合并**: 识别重复规则，建议合并
- **规则简化**: 简化复杂的条件表达式

#### 5.2 自然语言解释

- **规则解释器**: 将YAML规则转换为自然语言解释
- **决策解释**: 解释为什么某个交易被拒绝
- **审计报告生成**: 自动生成规则执行报告

#### 5.3 测试用例生成

- **边界测试**: 为规则生成边界值测试用例
- **覆盖率测试**: 生成覆盖所有分支的测试数据
- **回归测试**: 基于历史数据生成回归测试

#### 5.4 规则转换

- **从Drools转换**: 将Drools规则转换为CORINT DSL
- **从SQL转换**: 将SQL查询转换为规则
- **从代码生成**: 从Python/Java代码生成规则

#### 5.5 交互式生成

- **CLI对话工具**: 通过命令行对话生成规则
- **Web界面**: 图形化界面配置和生成
- **VS Code插件**: IDE集成

#### 5.6 批量分析

- **历史数据分析**: 批量分析历史交易，发现模式
- **异常检测**: 使用LLM识别异常行为模式
- **趋势预测**: 分析数据趋势，建议规则调整

---

## 🚨 风险和注意事项

### 技术风险

1. **LLM输出不稳定**
   - 风险：同样的描述可能生成不同的YAML
   - 缓解：使用低temperature（0.2），添加验证逻辑

2. **YAML格式错误**
   - 风险：LLM生成的YAML可能有语法错误
   - 缓解：自动验证，重试机制（max_retries=3）

3. **依赖外部API**
   - 风险：OpenAI/Anthropic API可能不可用
   - 缓解：支持多个provider，本地缓存

### 成本风险

1. **API调用费用**
   - GPT-4: ~$0.03/1K tokens
   - Claude: ~$0.015/1K tokens
   - 缓解：缓存机制，使用更便宜的模型（GPT-3.5）

### 安全风险

1. **Prompt注入**
   - 风险：恶意用户描述可能注入提示词
   - 缓解：输入验证，sanitize用户输入

2. **敏感信息泄露**
   - 风险：用户描述可能包含敏感业务逻辑
   - 缓解：提醒用户不要在描述中包含敏感数据

---

## 📞 支持和反馈

### 问题报告

如果在执行过程中遇到问题，请记录：

1. 具体步骤和操作
2. 错误信息和日志
3. 预期行为 vs 实际行为
4. 环境信息（Rust版本、OS等）

### 进度追踪

建议使用GitHub Issues或项目管理工具追踪：

- [ ] Phase 1: 创建 corint-llm crate
- [ ] Phase 2: SDK集成
- [ ] Phase 3: 清理DSL
- [ ] Phase 4: 文档更新
- [ ] 验收测试

---

## 📌 总结

这个重构方案将LLM从"不可行的实时调用"重新定位为"实用的离线代码生成工具"。主要变更：

1. ✅ 创建独立的 `corint-llm` crate，聚焦代码生成
2. ✅ 从DSL中移除 `reason` 步骤，避免误用
3. ✅ 提供SDK接口，支持自然语言生成配置
4. ✅ 保留优秀的LLM provider实现，用于正确的场景

**预计工期**: 3天
**风险等级**: 低（项目未发布，无兼容性问题）
**价值**: 高（提供实用的开发效率工具）

---

**文档版本**: 1.0
**最后更新**: 2025-12-31
**执行状态**: 待开始
