# Rule/Ruleset 重用方案设计文档

## 📊 当前问题分析

### 重复定义问题
- `card_testing`、`velocity_check` 等常用 rules 在多个 pipeline 中重复定义
- 相同的 decision_logic 模式在不同 ruleset 中重复
- 维护成本高，修改一处需要同步多处

### 现有结构
```
examples/                   # 示例和教学目录（将改为 repository）
├── pipelines/              # 完整的pipeline文件（包含所有定义）
│   ├── fraud_detection.yaml
│   ├── payment_pipeline.yaml
│   └── supabase_feature_ruleset.yaml
└── configs/                # 已有的配置文件（datasources、features、apis）
    ├── datasources/
    ├── features/
    └── apis/
```

---

## ✨ 推荐方案：分层模块化架构 + Repository Pattern

### 方案概览

创建一个**三层复用架构**：
1. **Library 层**：可重用的 rules 和 rulesets 库
2. **Template 层**：通用的 decision_logic 模板
3. **Pipeline 层**：业务场景编排（引用 library）

**实现方式**：编译时合并 + Repository Pattern
- **编译时合并**：运行时无需解析 imports，性能最优
- **类型检查和验证**：在编译期完成
- **最终生成独立的可执行 IR**：无运行时依赖，部署简单
- **Repository Pattern**：抽象数据源，支持文件系统、数据库等多种存储

---

## 📁 目录结构设计

### 生产环境目录结构

```
corint-decision/
├── crates/                     # Rust 源代码
│   ├── corint-core/
│   ├── corint-compiler/
│   ├── corint-runtime/
│   ├── corint-sdk/
│   └── corint-repository/      # 【新增】Repository 抽象层
│       ├── src/
│       │   ├── lib.rs
│       │   ├── traits.rs       # Repository trait 定义
│       │   ├── file_system.rs  # 文件系统实现
│       │   ├── database.rs     # 数据库实现（PostgreSQL/MySQL）
│       │   └── cache.rs        # 缓存层
│       └── Cargo.toml
│
├── repository/                 # 【生产规则策略仓库】
│   ├── library/                # 可重用组件库
│   │   ├── rules/              # 规则库
│   │   │   ├── fraud/          # 欺诈检测规则集
│   │   │   │   ├── fraud_farm.yaml
│   │   │   │   ├── account_takeover.yaml
│   │   │   │   ├── velocity_abuse.yaml
│   │   │   │   └── amount_outlier.yaml
│   │   │   ├── payment/        # 支付风控规则集
│   │   │   │   ├── card_testing.yaml
│   │   │   │   ├── velocity_check.yaml
│   │   │   │   ├── new_account_risk.yaml
│   │   │   │   └── suspicious_email.yaml
│   │   │   └── geography/      # 地理位置规则集
│   │   │       ├── suspicious_geography.yaml
│   │   │       └── suspicious_ip.yaml
│   │   │
│   │   ├── rulesets/           # Ruleset库
│   │   │   ├── fraud_detection_core.yaml
│   │   │   ├── payment_standard.yaml
│   │   │   ├── payment_high_value.yaml
│   │   │   └── transaction_risk_base.yaml
│   │   │
│   │   └── templates/          # Decision Logic模板
│   │       ├── score_based_decision.yaml
│   │       ├── pattern_based_decision.yaml
│   │       └── hybrid_decision.yaml
│   │
│   ├── pipelines/              # 业务场景编排
│   │   ├── fraud_detection.yaml
│   │   ├── payment_pipeline.yaml
│   │   └── supabase_feature_ruleset.yaml
│   │
│   └── configs/                # 运行时配置
│       ├── datasources/        # 数据源配置
│       ├── features/           # 特征配置
│       └── apis/               # 外部API配置
│
├── examples/                   # 【示例和教学】
│   ├── quick_start/            # 快速开始示例
│   ├── tutorials/              # 教程
│   └── advanced/               # 高级用法示例
│
├── tests/                      # 测试
│   ├── integration/
│   └── fixtures/
│
├── docs/                       # 文档
│   ├── api/
│   ├── guides/
│   └── reference/
│
├── scripts/                    # 工具脚本
│   ├── generate_catalog.sh
│   └── validate_rules.sh
│
└── docs/
    └── schema/                 # 数据库 Schema 定义（用于数据库模式）
        ├── 001_create_rules_table.sql
        ├── 002_create_rulesets_table.sql
        ├── 003_create_templates_table.sql
        ├── 004_create_pipelines_table.sql
        └── 005_create_audit_log.sql
```

### 目录命名说明

| 目录名 | 用途 | 存储方式 |
|--------|------|---------|
| `repository/` | 生产环境规则策略仓库 | 文件系统（当前）/ 数据库（未来） |
| `examples/` | 示例和教学资料 | 仅文件系统 |
| `crates/corint-repository/` | Repository 抽象层代码 | 源代码 |
| `docs/schema/` | 数据库 schema 定义 | SQL 脚本 |

---

## 📝 语法设计

### 1. Rules Library 示例

**文件**: `library/rules/fraud/fraud_farm.yaml`

```yaml
version: "0.1"

# Reusable fraud farm detection rule
rule:
  id: fraud_farm_pattern
  name: Fraud Farm Detection
  description: Detect organized fraud farms with high IP/device association

  when:
    conditions:
      - ip_device_count > 10
      - ip_user_count > 5

  score: 100

  metadata:
    category: fraud
    severity: critical
    tags: [organized_fraud, bot_networks]
    rule_version: "2.1.0"
    last_updated: "2024-12-11"
```

### 2. Ruleset Library 示例

**文件**: `library/rulesets/fraud_detection_core.yaml`

```yaml
version: "0.1"

# ✅ Ruleset 显式导入依赖的 rules
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
    - library/rules/fraud/velocity_abuse.yaml
    - library/rules/fraud/amount_outlier.yaml
    - library/rules/geography/suspicious_geography.yaml
    - library/rules/fraud/new_user_fraud.yaml

---

# Reusable core fraud detection ruleset
ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection Ruleset
  description: Reusable fraud detection with 6 common patterns

  # 引用已导入的 rules（通过 ID）
  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
    - velocity_abuse_pattern
    - amount_outlier_pattern
    - suspicious_geography_pattern
    - new_user_fraud_pattern

  # Decision logic
  decision_logic:
    # Critical patterns
    - condition: triggered_rules contains "fraud_farm_pattern"
      action: deny
      reason: "Critical: Fraud farm detected"
      terminate: true

    # Score thresholds
    - condition: total_score >= 200
      action: deny
      reason: "Critical risk (score: {total_score})"
      terminate: true

    - condition: total_score >= 150
      action: deny
      reason: "Very high risk (score: {total_score})"
      terminate: true

    - condition: total_score >= 100
      action: deny
      reason: "High risk (score: {total_score})"
      terminate: true

    - condition: total_score >= 60
      action: review
      reason: "Medium-high risk (score: {total_score})"
      terminate: false

    - condition: total_score >= 30
      action: review
      reason: "Medium risk (score: {total_score})"
      terminate: false

    # Multiple indicators
    - condition: triggered_count >= 3
      action: review
      reason: "Multiple fraud indicators ({triggered_count} patterns)"

    # Default
    - default: true
      action: approve
      reason: "Low risk - normal pattern"
```

### 3. Pipeline 引用方式

**文件**: `pipelines/fraud_detection.yaml`

```yaml
version: "0.1"

# ✅ Pipeline 只需导入 ruleset
# Ruleset 的依赖会自动传递加载
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  description: Production fraud detection using reusable components

  when:
    event.type: transaction

  steps:
    # 直接引用已导入的 ruleset
    - include:
        ruleset: fraud_detection_core
```

**效果对比**：
- 原有文件：338 行（包含所有定义）
- 新文件：~20 行（仅包含 imports 和 pipeline 定义）
- 代码减少：**94%** 🎉

**依赖传递机制**：
1. Pipeline 导入 `fraud_detection_core.yaml`
2. 编译器解析 ruleset 文件，发现它有 `imports.rules`
3. 自动递归加载所有依赖的 rules
4. 合并去重后生成最终的编译上下文

---

## 📦 依赖管理与导入机制

### 设计原则

所有文件都应该**显式声明自己的依赖**，不依赖外部上下文：

| 文件类型 | 依赖层级 | 是否需要 imports | 说明 |
|---------|---------|-----------------|------|
| **Rule** | 叶子节点 | ❌ 不需要 | 规则是最底层，无依赖 |
| **Ruleset** | 中间层 | ✅ 需要 | 声明依赖的 rules |
| **Pipeline** | 顶层 | ✅ 需要 | 声明依赖的 rulesets（rules 通过传递自动加载） |

### 依赖传递示例

```
Pipeline (fraud_detection.yaml)
  ↓ imports
  Ruleset (fraud_detection_core.yaml)
    ↓ imports
    Rule 1 (fraud_farm.yaml)
    Rule 2 (account_takeover.yaml)
    Rule 3 (velocity_abuse.yaml)
    ...
```

编译器会自动：
1. 从 Pipeline 开始解析
2. 递归加载 Ruleset 的依赖
3. 合并所有 Rules（去重）
4. 验证引用的 Rule IDs 是否存在

### 完整示例

#### 层级 1: Rule 定义（无依赖）

```yaml
# library/rules/fraud/fraud_farm.yaml
version: "0.1"

rule:
  id: fraud_farm_pattern
  name: Fraud Farm Detection

  when:
    conditions:
      - ip_device_count > 10
      - ip_user_count > 5

  score: 100
```

#### 层级 2: Ruleset 定义（依赖 Rules）

```yaml
# library/rulesets/fraud_detection_core.yaml
version: "0.1"

# ✅ Ruleset 显式声明依赖
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
    - library/rules/fraud/velocity_abuse.yaml

---

ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection

  # 引用已导入的 rules
  rules:
    - fraud_farm_pattern
    - account_takeover_pattern
    - velocity_abuse_pattern

  decision_logic:
    - condition: triggered_rules contains "fraud_farm_pattern"
      action: deny
      terminate: true
    - default: true
      action: approve
```

#### 层级 3: Pipeline 定义（依赖 Ruleset）

```yaml
# pipelines/fraud_detection.yaml
version: "0.1"

# ✅ Pipeline 只导入 ruleset
# Rules 通过 ruleset 自动传递
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: fraud_detection_pipeline

  when:
    event.type: transaction

  steps:
    - include:
        ruleset: fraud_detection_core
```

### 优势

✅ **依赖关系清晰**：每个文件都知道自己依赖什么
✅ **独立可测试**：Ruleset 可以单独验证和测试
✅ **自动传递**：Pipeline 不需要重复声明 rules
✅ **去重处理**：同一个 rule 被多次引用时自动去重
✅ **循环检测**：编译器可以检测循环依赖

### 编译期检查保证

编译器在加载和解析阶段会进行严格的验证：

| 检查项 | 说明 | 错误类型 |
|--------|------|---------|
| **Rule ID 唯一性** | 所有 rule IDs 必须全局唯一 | `DuplicateRuleId` |
| **Ruleset ID 唯一性** | 所有 ruleset IDs 必须全局唯一 | `DuplicateRulesetId` |
| **ID 命名空间冲突** | Rule ID 不能与 Ruleset ID 相同 | `IdConflict` |
| **循环依赖检测** | Ruleset 之间不能形成循环引用 | `CircularDependency` |
| **引用有效性** | Ruleset 引用的 rule IDs 必须存在 | `RuleNotFound` |
| **文件格式正确** | YAML 格式必须正确 | `InvalidYaml` |
| **必需字段检查** | Rule/Ruleset 必须包含必需字段 | `MissingField` |

**检查时机**：
1. ⏰ **导入时检查**：加载每个文件时验证格式和内容
2. ⏰ **合并时检查**：合并所有 imports 后验证 ID 唯一性
3. ⏰ **引用时检查**：验证所有引用的 IDs 都存在

**优势**：
- 🚫 **编译失败优于运行时错误**：问题在开发阶段就被发现
- 📍 **精确错误定位**：错误消息包含文件路径和具体位置
- 🔒 **类型安全**：确保所有引用都是有效的

---

## 🔧 高级特性

### 1. Ruleset 继承与扩展

```yaml
# pipelines/payment_pipeline.yaml
version: "0.1"

imports:
  rulesets:
    - library/rulesets/payment_standard.yaml

---

# 扩展标准 ruleset，添加自定义规则
ruleset:
  id: payment_high_value_extended
  name: High-Value Payment with Custom Rules

  # 继承基础 ruleset
  extends: payment_standard  # from library

  # 添加额外规则
  additional_rules:
    - custom_vip_whitelist

  # 覆盖 decision_logic
  decision_logic_override:
    - condition: user.vip_level > 3
      action: approve
      reason: "VIP user auto-approved"
      terminate: true

    # 继续执行父 ruleset 的 decision_logic
    - inherit: true
```

### 2. 参数化规则

**定义参数化规则**:

```yaml
# library/rules/payment/velocity_check.yaml
version: "0.1"

rule:
  id: velocity_check
  name: Velocity Violation
  description: High transaction frequency detection

  # 支持参数化配置
  params:
    threshold_24h: 20        # 默认值
    score_value: 50

  when:
    conditions:
      - transaction_count_24h > params.threshold_24h

  score: params.score_value
```

**使用时覆盖参数**:

```yaml
# 在 pipeline 中覆盖参数
imports:
  rules:
    - library/rules/payment/velocity_check.yaml

---

ruleset:
  id: strict_payment_rules
  name: Strict Payment Rules

  rules:
    - id: velocity_check
      params:
        threshold_24h: 10    # 覆盖默认的 20
        score_value: 80      # 覆盖默认的 50
```

### 3. Decision Logic 模板

**模板定义**: `library/templates/score_based_decision.yaml`

```yaml
version: "0.1"

template:
  id: score_based_decision
  name: Score-Based Decision Template
  description: Standard score threshold decision logic

  # 参数化的决策逻辑
  params:
    critical_threshold: 200
    high_threshold: 100
    medium_threshold: 60
    low_threshold: 30

  decision_logic:
    - condition: total_score >= params.critical_threshold
      action: deny
      reason: "Critical risk (score: {total_score})"
      terminate: true

    - condition: total_score >= params.high_threshold
      action: deny
      reason: "High risk (score: {total_score})"
      terminate: true

    - condition: total_score >= params.medium_threshold
      action: review
      reason: "Medium risk (score: {total_score})"
      terminate: true

    - condition: total_score >= params.low_threshold
      action: review
      reason: "Low risk monitoring"
      terminate: false

    - default: true
      action: approve
      reason: "Clean transaction"
```

**模板使用**:

```yaml
imports:
  templates:
    - library/templates/score_based_decision.yaml

---

ruleset:
  id: custom_payment_ruleset
  name: Custom Payment Ruleset

  rules:
    - card_testing
    - velocity_check

  # 使用模板并覆盖参数
  decision_logic:
    template: score_based_decision
    params:
      critical_threshold: 150  # 自定义阈值
      high_threshold: 80
```

---

## 🛠️ 技术实现方案（编译时合并）

### 1. AST 扩展

在 `corint-core` 中扩展 AST 定义：

```rust
// crates/corint-core/src/ast.rs

/// Import declaration in pipeline file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Imports {
    /// Imported rules (file paths)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,

    /// Imported rulesets (file paths)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rulesets: Option<Vec<String>>,

    /// Imported templates (file paths)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub templates: Option<Vec<String>>,
}

/// Multi-document YAML file containing imports and definitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    /// Version
    pub version: String,

    /// Imports from library
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imports: Option<Imports>,

    /// Rules defined in this file
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rules: Vec<Rule>,

    /// Rulesets defined in this file
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub rulesets: Vec<Ruleset>,

    /// Pipelines defined in this file
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub pipelines: Vec<Pipeline>,
}
```

### 2. Import Resolver 实现（支持依赖传递）

在 `corint-compiler` 中实现导入解析器：

```rust
// crates/corint-compiler/src/import_resolver.rs

use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet};
use corint_core::ast::{Rule, Ruleset, Pipeline, Imports, Document};
use crate::error::{Result, CompilerError};

/// Import resolver for loading and merging library components
pub struct ImportResolver {
    /// Base path for library files
    library_base_path: PathBuf,

    /// Cached loaded rules (path -> Rule)
    rule_cache: HashMap<String, Rule>,

    /// Cached loaded rulesets (path -> Ruleset)
    ruleset_cache: HashMap<String, Ruleset>,

    /// Track loading stack to detect circular dependencies
    loading_stack: Vec<String>,
}

impl ImportResolver {
    /// Create a new import resolver
    pub fn new(library_base_path: impl Into<PathBuf>) -> Self {
        Self {
            library_base_path: library_base_path.into(),
            rule_cache: HashMap::new(),
            ruleset_cache: HashMap::new(),
            loading_stack: Vec::new(),
        }
    }

    /// Resolve all imports in a document (with dependency propagation)
    pub fn resolve_imports(&mut self, document: &Document) -> Result<ResolvedDocument> {
        let mut resolved_rules = Vec::new();
        let mut resolved_rulesets = Vec::new();

        // 1. Load imported rules
        if let Some(imports) = &document.imports {
            if let Some(rule_paths) = &imports.rules {
                for path in rule_paths {
                    let rule = self.load_rule(path)?;
                    resolved_rules.push(rule);
                }
            }

            if let Some(ruleset_paths) = &imports.rulesets {
                for path in ruleset_paths {
                    // ✅ 加载 ruleset 及其依赖的 rules
                    let (ruleset, deps) = self.load_ruleset_with_deps(path)?;

                    // 添加 ruleset 依赖的所有 rules
                    resolved_rules.extend(deps.rules);

                    // 添加 ruleset 本身
                    resolved_rulesets.push(ruleset);
                }
            }
        }

        // 2. Add rules/rulesets defined in the document
        resolved_rules.extend(document.rules.clone());
        resolved_rulesets.extend(document.rulesets.clone());

        // 3. 去重（同一个 rule 可能被多次引用）
        resolved_rules = self.deduplicate_rules(resolved_rules)?;
        resolved_rulesets = self.deduplicate_rulesets(resolved_rulesets)?;

        // 4. Validate IDs
        self.validate_unique_ids(&resolved_rules, &resolved_rulesets)?;

        Ok(ResolvedDocument {
            rules: resolved_rules,
            rulesets: resolved_rulesets,
            pipelines: document.pipelines.clone(),
        })
    }

    /// Load a rule from file with caching
    fn load_rule(&mut self, path: &str) -> Result<Rule> {
        // Check cache first
        if let Some(cached) = self.rule_cache.get(path) {
            return Ok(cached.clone());
        }

        // Resolve full path
        let full_path = self.library_base_path.join(path);

        // Load and parse YAML
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| CompilerError::ImportNotFound {
                path: path.to_string(),
                source: e,
            })?;

        let document: Document = serde_yaml::from_str(&content)
            .map_err(|e| CompilerError::InvalidYaml {
                path: path.to_string(),
                source: e,
            })?;

        // Extract the rule
        let rule = document.rules.into_iter().next()
            .ok_or_else(|| CompilerError::NoRuleInFile {
                path: path.to_string(),
            })?;

        // Cache it
        self.rule_cache.insert(path.to_string(), rule.clone());

        Ok(rule)
    }

    /// ✅ Load a ruleset with its dependencies (递归加载)
    fn load_ruleset_with_deps(&mut self, path: &str) -> Result<(Ruleset, Dependencies)> {
        // 检测循环依赖
        if self.loading_stack.contains(&path.to_string()) {
            return Err(CompilerError::CircularDependency {
                path: path.to_string(),
                stack: self.loading_stack.clone(),
            });
        }

        // 加入加载栈
        self.loading_stack.push(path.to_string());

        // Check cache
        if let Some(cached) = self.ruleset_cache.get(path) {
            self.loading_stack.pop();
            return Ok((cached.clone(), Dependencies { rules: vec![] }));
        }

        // 1. Load ruleset file
        let full_path = self.library_base_path.join(path);
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| CompilerError::ImportNotFound {
                path: path.to_string(),
                source: e,
            })?;

        let document: Document = serde_yaml::from_str(&content)
            .map_err(|e| CompilerError::InvalidYaml {
                path: path.to_string(),
                source: e,
            })?;

        // 2. ✅ 递归解析 ruleset 的 imports（依赖传递）
        let mut deps_rules = Vec::new();
        if let Some(imports) = &document.imports {
            if let Some(rule_paths) = &imports.rules {
                for rule_path in rule_paths {
                    let rule = self.load_rule(rule_path)?;
                    deps_rules.push(rule);
                }
            }

            // 也可以支持 ruleset 依赖其他 ruleset（更深的传递）
            if let Some(ruleset_paths) = &imports.rulesets {
                for ruleset_path in ruleset_paths {
                    let (_, sub_deps) = self.load_ruleset_with_deps(ruleset_path)?;
                    deps_rules.extend(sub_deps.rules);
                }
            }
        }

        // 3. Extract ruleset
        let ruleset = document.rulesets.into_iter().next()
            .ok_or_else(|| CompilerError::NoRulesetInFile {
                path: path.to_string(),
            })?;

        // Cache it
        self.ruleset_cache.insert(path.to_string(), ruleset.clone());

        // 移出加载栈
        self.loading_stack.pop();

        Ok((ruleset, Dependencies { rules: deps_rules }))
    }

    /// ✅ 去重 rules（保留第一次出现的）
    fn deduplicate_rules(&self, rules: Vec<Rule>) -> Result<Vec<Rule>> {
        let mut seen = HashSet::new();
        let mut unique_rules = Vec::new();

        for rule in rules {
            if seen.insert(rule.id.clone()) {
                unique_rules.push(rule);
            }
        }

        Ok(unique_rules)
    }

    /// 去重 rulesets
    fn deduplicate_rulesets(&self, rulesets: Vec<Ruleset>) -> Result<Vec<Ruleset>> {
        let mut seen = HashSet::new();
        let mut unique_rulesets = Vec::new();

        for ruleset in rulesets {
            if seen.insert(ruleset.id.clone()) {
                unique_rulesets.push(ruleset);
            }
        }

        Ok(unique_rulesets)
    }

    /// ✅ 验证所有 rule/ruleset IDs 的唯一性（编译期检查）
    fn validate_unique_ids(&self, rules: &[Rule], rulesets: &[Ruleset]) -> Result<()> {
        // 检查 Rule IDs 唯一性
        let mut rule_ids = HashMap::new();
        for rule in rules {
            if let Some(existing_path) = rule_ids.insert(&rule.id, &rule.metadata) {
                // 发现重复的 Rule ID
                return Err(CompilerError::DuplicateRuleId {
                    id: rule.id.clone(),
                    first_defined: self.get_rule_source(&rule.id).unwrap_or("unknown".to_string()),
                    also_defined: "current import".to_string(),
                });
            }
        }

        // 检查 Ruleset IDs 唯一性
        let mut ruleset_ids = HashMap::new();
        for ruleset in rulesets {
            if let Some(_) = ruleset_ids.insert(&ruleset.id, &ruleset.name) {
                // 发现重复的 Ruleset ID
                return Err(CompilerError::DuplicateRulesetId {
                    id: ruleset.id.clone(),
                    first_defined: self.get_ruleset_source(&ruleset.id).unwrap_or("unknown".to_string()),
                    also_defined: "current import".to_string(),
                });
            }
        }

        // 检查 Rule ID 和 Ruleset ID 之间没有冲突
        // (虽然它们在不同命名空间，但为了避免混淆，也可以检查)
        for ruleset_id in ruleset_ids.keys() {
            if rule_ids.contains_key(ruleset_id) {
                return Err(CompilerError::IdConflict {
                    id: ruleset_id.to_string(),
                    conflict: "Rule ID conflicts with Ruleset ID".to_string(),
                });
            }
        }

        Ok(())
    }

    /// 获取 rule 的源文件路径（用于错误消息）
    fn get_rule_source(&self, rule_id: &str) -> Option<String> {
        for (path, rule) in &self.rule_cache {
            if rule.id == rule_id {
                return Some(path.clone());
            }
        }
        None
    }

    /// 获取 ruleset 的源文件路径（用于错误消息）
    fn get_ruleset_source(&self, ruleset_id: &str) -> Option<String> {
        for (path, ruleset) in &self.ruleset_cache {
            if ruleset.id == ruleset_id {
                return Some(path.clone());
            }
        }
        None
    }
}

/// Dependencies loaded from a ruleset
#[derive(Debug, Clone)]
struct Dependencies {
    rules: Vec<Rule>,
}

/// Resolved document with all imports merged
#[derive(Debug, Clone)]
pub struct ResolvedDocument {
    pub rules: Vec<Rule>,
    pub rulesets: Vec<Ruleset>,
    pub pipelines: Vec<Pipeline>,
}
```

### 3. 集成到 Compiler

修改 `Compiler` 使用 `ImportResolver`：

```rust
// crates/corint-compiler/src/compiler.rs

use crate::import_resolver::{ImportResolver, ResolvedDocument};

pub struct Compiler {
    options: CompilerOptions,
    import_resolver: ImportResolver,
    // ... other fields
}

impl Compiler {
    pub fn new() -> Self {
        Self::with_options(CompilerOptions::default())
    }

    pub fn with_options(options: CompilerOptions) -> Self {
        // Determine library base path (e.g., from env or config)
        let library_base_path = std::env::var("CORINT_LIBRARY_PATH")
            .unwrap_or_else(|_| "examples".to_string());

        Self {
            options,
            import_resolver: ImportResolver::new(library_base_path),
            // ...
        }
    }

    /// Compile a pipeline file with imports
    pub fn compile_pipeline_file(&mut self, file_path: &Path) -> Result<Program> {
        // 1. Load the file
        let content = std::fs::read_to_string(file_path)?;
        let document: Document = serde_yaml::from_str(&content)?;

        // 2. Resolve imports
        let resolved = self.import_resolver.resolve_imports(&document)?;

        // 3. Build a complete context with all rules/rulesets
        let context = CompilationContext {
            rules: resolved.rules,
            rulesets: resolved.rulesets,
        };

        // 4. Compile the pipeline
        let pipeline = resolved.pipelines.into_iter().next()
            .ok_or(CompilerError::NoPipelineInFile)?;

        self.compile_pipeline_with_context(&pipeline, &context)
    }
}
```

### 4. 错误处理

```rust
// crates/corint-compiler/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum CompilerError {
    #[error("Import not found: {path}")]
    ImportNotFound {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid YAML in {path}")]
    InvalidYaml {
        path: String,
        #[source]
        source: serde_yaml::Error,
    },

    #[error("No rule found in file: {path}")]
    NoRuleInFile { path: String },

    #[error("No ruleset found in file: {path}")]
    NoRulesetInFile { path: String },

    #[error("Duplicate rule ID: '{id}'\n  First defined in: {first_defined}\n  Also defined in: {also_defined}")]
    DuplicateRuleId {
        id: String,
        first_defined: String,
        also_defined: String,
    },

    #[error("Duplicate ruleset ID: '{id}'\n  First defined in: {first_defined}\n  Also defined in: {also_defined}")]
    DuplicateRulesetId {
        id: String,
        first_defined: String,
        also_defined: String,
    },

    #[error("ID conflict: '{id}'\n  {conflict}")]
    IdConflict {
        id: String,
        conflict: String,
    },

    #[error("Rule not found: {id}")]
    RuleNotFound { id: String },

    #[error("Ruleset not found: {id}")]
    RulesetNotFound { id: String },

    #[error("Circular dependency detected: {path}\n  Loading stack: {}", stack.join(" -> "))]
    CircularDependency {
        path: String,
        stack: Vec<String>,
    },

    // ... other errors
}
```

### 编译期检查示例

#### ✅ 成功案例

```yaml
# library/rules/fraud/fraud_farm.yaml
rule:
  id: fraud_farm_pattern  # ✅ 唯一

# library/rules/fraud/velocity_abuse.yaml
rule:
  id: velocity_abuse_pattern  # ✅ 唯一，没有冲突
```

**编译通过** ✓

---

#### ❌ 错误案例 1：重复的 Rule ID

```yaml
# library/rules/fraud/fraud_farm.yaml
rule:
  id: fraud_farm_pattern

# library/rules/payment/card_testing.yaml
rule:
  id: fraud_farm_pattern  # ❌ 重复！
```

**编译错误**：
```
Error: Duplicate rule ID: 'fraud_farm_pattern'
  First defined in: library/rules/fraud/fraud_farm.yaml
  Also defined in: library/rules/payment/card_testing.yaml
```

---

#### ❌ 错误案例 2：重复的 Ruleset ID

```yaml
# library/rulesets/fraud_detection_core.yaml
ruleset:
  id: fraud_detection

# library/rulesets/fraud_detection_v2.yaml
ruleset:
  id: fraud_detection  # ❌ 重复！
```

**编译错误**：
```
Error: Duplicate ruleset ID: 'fraud_detection'
  First defined in: library/rulesets/fraud_detection_core.yaml
  Also defined in: library/rulesets/fraud_detection_v2.yaml
```

---

#### ❌ 错误案例 3：Rule ID 与 Ruleset ID 冲突

```yaml
# library/rules/fraud/fraud_detection.yaml
rule:
  id: fraud_detection

# library/rulesets/fraud_detection_core.yaml
ruleset:
  id: fraud_detection  # ❌ 与 rule ID 冲突！
```

**编译错误**：
```
Error: ID conflict: 'fraud_detection'
  Rule ID conflicts with Ruleset ID
  This may cause confusion when referencing IDs
```

---

## 📋 实施计划

### Phase 1: 基础架构（2-3天）

**目标**：实现基本的 import 功能

1. **AST 扩展**（0.5天）
   - [ ] 在 `corint-core` 中添加 `Imports` 和 `Document` 类型
   - [ ] 添加 serde 序列化支持
   - [ ] 编写单元测试

2. **Import Resolver 实现**（1天）
   - [ ] 实现 `ImportResolver` 基础结构
   - [ ] 实现 `load_rule()` 方法
   - [ ] 实现 `load_ruleset_with_deps()` 方法（支持依赖传递）
   - [ ] 添加缓存机制
   - [ ] 实现 `loading_stack` 循环依赖检测
   - [ ] 实现 `deduplicate_rules()` 和 `deduplicate_rulesets()` 去重逻辑
   - [ ] ✅ **实现 `validate_unique_ids()` - 编译期 ID 唯一性检查**
   - [ ] ✅ **实现 `get_rule_source()` 和 `get_ruleset_source()` - 错误定位**
   - [ ] 编写单元测试（包括 ID 冲突测试用例）

3. **Compiler 集成与错误处理**（1天）
   - [ ] 修改 `Compiler` 集成 `ImportResolver`
   - [ ] 添加 `compile_pipeline_file()` 方法
   - [ ] ✅ **扩展错误类型**：
     - [ ] `DuplicateRuleId` (包含源文件路径)
     - [ ] `DuplicateRulesetId` (包含源文件路径)
     - [ ] `IdConflict` (Rule/Ruleset ID 冲突)
     - [ ] `CircularDependency` (循环依赖)
     - [ ] `RuleNotFound` / `RulesetNotFound`
   - [ ] ✅ **改进错误消息**：包含文件路径、行号等上下文信息
   - [ ] 编写集成测试
   - [ ] ✅ **添加负面测试用例**（重复 ID、循环依赖等）

4. **创建目录结构**（0.5天）
   - [ ] 创建 `examples/library/` 目录结构
   - [ ] 创建 README 文档说明目录用途

### Phase 2: 规则提取与迁移（2-3天）

**目标**：将现有规则提取到 library

1. **提取 Fraud 规则**（1天）
   - [ ] `library/rules/fraud/fraud_farm.yaml`
   - [ ] `library/rules/fraud/account_takeover.yaml`
   - [ ] `library/rules/fraud/velocity_abuse.yaml`
   - [ ] `library/rules/fraud/amount_outlier.yaml`
   - [ ] `library/rules/fraud/new_user_fraud.yaml`

2. **提取 Payment 规则**（0.5天）
   - [ ] `library/rules/payment/card_testing.yaml`
   - [ ] `library/rules/payment/velocity_check.yaml`
   - [ ] `library/rules/payment/new_account_risk.yaml`
   - [ ] `library/rules/payment/suspicious_email.yaml`

3. **提取 Geography 规则**（0.5天）
   - [ ] `library/rules/geography/suspicious_geography.yaml`
   - [ ] `library/rules/geography/suspicious_ip.yaml`

4. **创建标准 Rulesets**（1天）
   - [ ] `library/rulesets/fraud_detection_core.yaml`
   - [ ] `library/rulesets/payment_standard.yaml`
   - [ ] `library/rulesets/payment_high_value.yaml`

5. **重构现有 Pipelines**（1天）
   - [ ] 重构 `fraud_detection.yaml` 使用 imports
   - [ ] 重构 `payment_pipeline.yaml` 使用 imports
   - [ ] 验证功能等价性

### Phase 3: 高级特性（3-4天）

**目标**：实现高级功能

1. **Ruleset 继承**（1.5天）
   - [ ] AST 支持 `extends` 字段
   - [ ] 实现继承逻辑（合并 rules 和 decision_logic）
   - [ ] 添加测试

2. **参数化规则**（1.5天）
   - [ ] AST 支持 `params` 字段
   - [ ] 实现参数替换逻辑
   - [ ] 支持参数覆盖
   - [ ] 添加测试

3. **Decision Logic 模板**（1天）
   - [ ] 定义模板语法
   - [ ] 实现模板加载和应用
   - [ ] 创建标准模板
   - [ ] 添加测试

4. **文档完善**（0.5天）
   - [ ] 更新 README
   - [ ] 编写 library 使用指南
   - [ ] 添加更多示例

### Phase 4: 优化与完善（1-2天）

1. **性能优化**
   - [ ] 实现并行加载多个 imports
   - [ ] 优化缓存策略
   - [ ] 添加性能基准测试

2. **开发者体验**
   - [ ] 改进错误消息（显示具体的文件和行号）
   - [ ] 添加 import 循环依赖检测
   - [ ] 添加 rule/ruleset 版本兼容性检查

3. **工具支持**
   - [ ] 实现 `cargo run --bin rule-catalog` 生成规则目录
   - [ ] 实现 `cargo run --bin rule-test` 运行规则测试

---

## 📊 迁移前后对比

### 文件大小对比

| Pipeline | 原始行数 | 新版行数 | 减少比例 |
|----------|---------|---------|---------|
| fraud_detection.yaml | 338 | ~30 | 91% |
| payment_pipeline.yaml | 236 | ~50 | 79% |
| supabase_feature_ruleset.yaml | 146 | ~35 | 76% |

### 重用性提升

- **Rules**: 从 0% 重用 → 80%+ 重用
- **Rulesets**: 从 0% 重用 → 60%+ 重用
- **Decision Logic**: 从重复定义 → 模板化

### 维护成本降低

- 修改一个规则：从需要修改 3-5 个文件 → 只修改 1 个文件
- 添加新 pipeline：从需要复制粘贴 → 直接引用 library
- 测试覆盖：从分散测试 → 集中测试规则库

---

## 🔍 额外建议

### 1. 规则版本管理

在每个规则中添加版本信息：

```yaml
# library/rules/fraud/fraud_farm.yaml
version: "0.1"
metadata:
  rule_version: "2.1.0"
  last_updated: "2024-12-11"
  changelog:
    - version: "2.1.0"
      date: "2024-12-11"
      changes: "Adjusted ip_device_count threshold from 5 to 10"
    - version: "2.0.0"
      date: "2024-11-01"
      changes: "Added ip_user_count condition"
    - version: "1.0.0"
      date: "2024-10-01"
      changes: "Initial version"
```

### 2. 规则测试套件

为每个规则添加测试文件：

```yaml
# library/rules/fraud/fraud_farm.test.yaml
tests:
  - name: "Fraud farm detected - high device count"
    input:
      ip_device_count: 15
      ip_user_count: 8
    expected:
      triggered: true
      score: 100

  - name: "Fraud farm detected - boundary case"
    input:
      ip_device_count: 11
      ip_user_count: 6
    expected:
      triggered: true
      score: 100

  - name: "Normal traffic - below threshold"
    input:
      ip_device_count: 2
      ip_user_count: 1
    expected:
      triggered: false
      score: 0

  - name: "Edge case - only device count high"
    input:
      ip_device_count: 15
      ip_user_count: 2
    expected:
      triggered: false
      score: 0
```

### 3. 自动化规则目录

生成规则文档的 CLI 工具：

```bash
$ cargo run --bin rule-catalog

Generating rule catalog from library...
Found 12 rules:
  - Fraud (6 rules)
  - Payment (4 rules)
  - Geography (2 rules)

Output: docs/rule_catalog.md
```

生成的文档示例：

```markdown
# Rule Catalog

## Fraud Detection Rules

### fraud_farm_pattern
- **Version**: 2.1.0
- **Score**: 100
- **Severity**: Critical
- **Description**: Detect organized fraud farms with high IP/device association
- **Conditions**:
  - `ip_device_count > 10`
  - `ip_user_count > 5`
- **Tags**: organized_fraud, bot_networks
- **File**: [library/rules/fraud/fraud_farm.yaml](../library/rules/fraud/fraud_farm.yaml)

...
```

### 4. Import 语法的备选方案

如果未来需要更灵活的导入方式，可以考虑：

```yaml
# 选项A：通配符导入（Phase 4）
imports:
  rules:
    - library/rules/fraud/*          # 导入所有 fraud 规则
    - library/rules/payment/*.yaml   # 显式指定扩展名

# 选项B：包导入（Phase 4）
imports:
  packages:
    - fraud        # 等价于 library/rules/fraud/*
    - payment      # 等价于 library/rules/payment/*

# 选项C：选择性导入（Phase 4）
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/account_takeover.yaml
    exclude:
      - library/rules/fraud/new_user_fraud.yaml  # 排除特定规则
```

---

## ✅ 验收标准

### Phase 1 完成标准
- [ ] 可以编写包含 `imports` 的 pipeline YAML 文件
- [ ] 编译器能正确解析和加载 imported rules/rulesets
- [ ] 所有单元测试和集成测试通过
- [ ] 错误消息清晰（文件路径、行号等）

### Phase 2 完成标准
- [ ] 所有通用规则已提取到 `library/` 目录
- [ ] 至少 2 个现有 pipeline 已重构使用 imports
- [ ] 重构后的 pipeline 功能与原版完全一致
- [ ] 文件行数减少 70%+
- [ ] ✅ **所有 Rule IDs 和 Ruleset IDs 全局唯一（通过编译期检查验证）**

### Phase 3 完成标准
- [ ] 支持 ruleset 继承（`extends`）
- [ ] 支持参数化规则（`params`）
- [ ] 提供至少 3 个 decision logic 模板
- [ ] 文档完整，包含使用示例

### Phase 4 完成标准
- [ ] 编译时间 < 原来的 120%（性能可接受）
- [ ] 提供规则目录生成工具
- [ ] 提供规则测试运行工具
- [ ] 开发者文档完善

---

## 🚀 快速开始

完成实施后，开发者可以这样使用：

### 1. 创建新的 Pipeline

```bash
# 创建文件 pipelines/my_custom_pipeline.yaml
```

```yaml
version: "0.1"

imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: my_custom_pipeline
  name: My Custom Risk Pipeline

  when:
    event.type: transaction

  steps:
    - include:
        ruleset: fraud_detection_core
```

### 2. 编译并运行

```bash
cargo run --example my_custom_pipeline
```

### 3. 自定义规则

```yaml
# 创建自己的规则库
# library/rules/custom/my_rule.yaml

version: "0.1"

rule:
  id: my_custom_rule
  name: My Custom Rule

  when:
    conditions:
      - custom_feature > 100

  score: 50
```

```yaml
# 在 pipeline 中使用
imports:
  rules:
    - library/rules/custom/my_rule.yaml
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

ruleset:
  id: extended_fraud_detection
  name: Extended Fraud Detection

  extends: fraud_detection_core

  additional_rules:
    - my_custom_rule
```

---

## 🗄️ Repository Pattern 架构设计

### 设计目标

支持多种数据源，统一接口：
- **文件系统模式**：当前使用，适合开发和小规模部署
- **数据库模式**：生产环境，支持在线编辑、版本管理、权限控制
- **混合模式**：数据库优先，文件系统作为 fallback

### Repository Trait 定义

```rust
// crates/corint-repository/src/traits.rs

use async_trait::async_trait;
use corint_core::ast::{Rule, Ruleset, Pipeline};
use crate::error::Result;

/// 规则仓库抽象接口
#[async_trait]
pub trait RuleRepository: Send + Sync {
    /// 根据 ID 加载规则
    async fn load_rule(&self, id: &str) -> Result<Rule>;

    /// 根据 ID 加载规则集
    async fn load_ruleset(&self, id: &str) -> Result<Ruleset>;

    /// 根据 ID 加载 Pipeline
    async fn load_pipeline(&self, id: &str) -> Result<Pipeline>;

    /// 列出所有规则 ID
    async fn list_rules(&self) -> Result<Vec<String>>;

    /// 列出所有规则集 ID
    async fn list_rulesets(&self) -> Result<Vec<String>>;

    /// 保存规则（用于更新）
    async fn save_rule(&self, rule: &Rule) -> Result<()>;

    /// 保存规则集
    async fn save_ruleset(&self, ruleset: &Ruleset) -> Result<()>;

    /// 删除规则
    async fn delete_rule(&self, id: &str) -> Result<()>;
}

/// 配置仓库抽象接口
#[async_trait]
pub trait ConfigRepository: Send + Sync {
    /// 加载数据源配置
    async fn load_datasource(&self, id: &str) -> Result<DataSourceConfig>;

    /// 加载特征配置
    async fn load_feature(&self, id: &str) -> Result<FeatureConfig>;

    /// 加载 API 配置
    async fn load_api(&self, id: &str) -> Result<ApiConfig>;

    /// 保存配置
    async fn save_datasource(&self, config: &DataSourceConfig) -> Result<()>;
    async fn save_feature(&self, config: &FeatureConfig) -> Result<()>;
    async fn save_api(&self, config: &ApiConfig) -> Result<()>;
}
```

### 文件系统实现

```rust
// crates/corint-repository/src/file_system.rs

use std::path::PathBuf;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

/// 文件系统仓库实现
pub struct FileSystemRepository {
    base_path: PathBuf,
    cache: Arc<RwLock<HashMap<String, Rule>>>,
}

impl FileSystemRepository {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 根据规则 ID 推断文件路径
    fn rule_path(&self, id: &str) -> PathBuf {
        // 尝试从 library/rules/ 的各个子目录查找
        // 例如: fraud_farm_pattern -> library/rules/fraud/fraud_farm.yaml
        self.base_path.join("library/rules").join(format!("{}.yaml", id))
    }
}

#[async_trait]
impl RuleRepository for FileSystemRepository {
    async fn load_rule(&self, id: &str) -> Result<Rule> {
        // 1. 检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(id) {
                return Ok(cached.clone());
            }
        }

        // 2. 从文件系统加载
        let path = self.rule_path(id);
        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| Error::RuleNotFound {
                id: id.to_string(),
                source: e
            })?;

        let rule: Rule = serde_yaml::from_str(&content)
            .map_err(|e| Error::InvalidYaml {
                path: path.display().to_string(),
                source: e
            })?;

        // 3. 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(id.to_string(), rule.clone());
        }

        Ok(rule)
    }

    async fn save_rule(&self, rule: &Rule) -> Result<()> {
        // 1. 序列化为 YAML
        let content = serde_yaml::to_string(rule)
            .map_err(|e| Error::SerializationError { source: e })?;

        // 2. 保存到文件
        let path = self.rule_path(&rule.id);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, content).await?;

        // 3. 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(rule.id.clone(), rule.clone());
        }

        Ok(())
    }

    async fn list_rules(&self) -> Result<Vec<String>> {
        let mut rules = Vec::new();
        let library_path = self.base_path.join("library/rules");

        // 递归扫描所有子目录
        let mut entries = tokio::fs::read_dir(&library_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                // 扫描子目录中的 YAML 文件
                let mut sub_entries = tokio::fs::read_dir(entry.path()).await?;
                while let Some(sub_entry) = sub_entries.next_entry().await? {
                    if let Some(ext) = sub_entry.path().extension() {
                        if ext == "yaml" || ext == "yml" {
                            if let Some(stem) = sub_entry.path().file_stem() {
                                rules.push(stem.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(rules)
    }
}
```

### 数据库实现

```rust
// crates/corint-repository/src/database.rs

use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;

/// 数据库仓库实现（PostgreSQL）
pub struct DatabaseRepository {
    pool: PgPool,
    cache: Arc<RwLock<HashMap<String, Rule>>>,
}

impl DatabaseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl RuleRepository for DatabaseRepository {
    async fn load_rule(&self, id: &str) -> Result<Rule> {
        // 1. 检查缓存
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(id) {
                return Ok(cached.clone());
            }
        }

        // 2. 从数据库加载
        let row = sqlx::query!(
            r#"
            SELECT content
            FROM rules
            WHERE id = $1 AND enabled = true
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::RuleNotFound {
            id: id.to_string(),
            source: e.into()
        })?;

        let rule: Rule = serde_yaml::from_str(&row.content)
            .map_err(|e| Error::InvalidYaml {
                path: format!("database:rules:{}", id),
                source: e
            })?;

        // 3. 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(id.to_string(), rule.clone());
        }

        Ok(rule)
    }

    async fn save_rule(&self, rule: &Rule) -> Result<()> {
        let content = serde_yaml::to_string(rule)
            .map_err(|e| Error::SerializationError { source: e })?;

        sqlx::query!(
            r#"
            INSERT INTO rules (id, name, description, content, category, version, enabled, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, true, NOW())
            ON CONFLICT (id)
            DO UPDATE SET
                name = $2,
                description = $3,
                content = $4,
                category = $5,
                version = $6,
                updated_at = NOW()
            "#,
            rule.id,
            rule.name,
            rule.description,
            content,
            rule.metadata.as_ref().and_then(|m| m.get("category")).map(|v| v.as_str()).flatten(),
            rule.metadata.as_ref().and_then(|m| m.get("rule_version")).map(|v| v.as_str()).flatten(),
        )
        .execute(&self.pool)
        .await?;

        // 更新缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(rule.id.clone(), rule.clone());
        }

        Ok(())
    }

    async fn list_rules(&self) -> Result<Vec<String>> {
        let rows = sqlx::query!(
            "SELECT id FROM rules WHERE enabled = true ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| row.id).collect())
    }
}
```

### Fallback Repository（混合模式）

```rust
// crates/corint-repository/src/fallback.rs

/// Fallback 仓库：优先使用主仓库，失败时回退到备用仓库
pub struct FallbackRepository<P, F> {
    primary: P,
    fallback: F,
}

impl<P, F> FallbackRepository<P, F> {
    pub fn new(primary: P, fallback: F) -> Self {
        Self { primary, fallback }
    }
}

#[async_trait]
impl<P, F> RuleRepository for FallbackRepository<P, F>
where
    P: RuleRepository,
    F: RuleRepository,
{
    async fn load_rule(&self, id: &str) -> Result<Rule> {
        // 优先从主仓库加载
        match self.primary.load_rule(id).await {
            Ok(rule) => Ok(rule),
            Err(_) => {
                // 失败则从 fallback 加载
                self.fallback.load_rule(id).await
            }
        }
    }

    async fn save_rule(&self, rule: &Rule) -> Result<()> {
        // 只保存到主仓库
        self.primary.save_rule(rule).await
    }

    // ... 其他方法类似
}
```

### 数据库 Schema

```sql
-- docs/schema/001_create_rules_table.sql

CREATE TABLE IF NOT EXISTS rules (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    content TEXT NOT NULL,          -- YAML 格式的规则定义
    category VARCHAR(100),           -- fraud, payment, geography
    version VARCHAR(50),             -- 规则版本号
    enabled BOOLEAN DEFAULT true,    -- 是否启用
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    created_by VARCHAR(255),
    updated_by VARCHAR(255)
);

CREATE INDEX idx_rules_category ON rules(category);
CREATE INDEX idx_rules_enabled ON rules(enabled);
CREATE INDEX idx_rules_updated_at ON rules(updated_at DESC);

-- docs/schema/002_create_rulesets_table.sql

CREATE TABLE IF NOT EXISTS rulesets (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    content TEXT NOT NULL,          -- YAML 格式的 ruleset 定义
    version VARCHAR(50),
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- docs/schema/003_create_pipelines_table.sql

CREATE TABLE IF NOT EXISTS pipelines (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    content TEXT NOT NULL,          -- YAML 格式的 pipeline 定义
    version VARCHAR(50),
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- docs/schema/004_create_configs_table.sql

CREATE TABLE IF NOT EXISTS configs (
    id VARCHAR(255) PRIMARY KEY,
    type VARCHAR(50) NOT NULL,      -- datasource, feature, api
    name VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,          -- YAML 或 JSON 格式
    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_configs_type ON configs(type);

-- docs/schema/005_create_rule_versions_table.sql

CREATE TABLE IF NOT EXISTS rule_versions (
    id SERIAL PRIMARY KEY,
    rule_id VARCHAR(255) NOT NULL,
    version VARCHAR(50) NOT NULL,
    content TEXT NOT NULL,
    change_log TEXT,
    created_at TIMESTAMP DEFAULT NOW(),
    created_by VARCHAR(255),
    FOREIGN KEY (rule_id) REFERENCES rules(id) ON DELETE CASCADE
);

CREATE INDEX idx_rule_versions_rule_id ON rule_versions(rule_id);
CREATE INDEX idx_rule_versions_created_at ON rule_versions(created_at DESC);
```

### ID 唯一性验证：文件系统 vs 数据库模式

不同存储模式下，ID 唯一性检查的实现方式不同：

#### 文件系统模式：编译期检查

在文件系统模式下，所有规则和规则集都存储在 YAML 文件中，编译器在编译期（compile-time）进行 ID 唯一性检查：

```rust
// 编译时检查（文件系统模式）
impl ImportResolver {
    fn validate_unique_ids(&self, rules: &[Rule], rulesets: &[Ruleset]) -> Result<()> {
        // 检查 Rule IDs 唯一性
        let mut rule_ids = HashMap::new();
        for rule in rules {
            if let Some(_) = rule_ids.insert(&rule.id, &rule.metadata) {
                return Err(CompilerError::DuplicateRuleId {
                    id: rule.id.clone(),
                    first_defined: self.get_rule_source(&rule.id).unwrap_or("unknown".to_string()),
                    also_defined: "current import".to_string(),
                });
            }
        }

        // 检查 Ruleset IDs 唯一性
        // 检查 Rule/Ruleset ID 命名空间冲突
        // ...

        Ok(())
    }
}
```

**特点**：
- ✅ **零运行时开销**：ID 冲突在编译期发现，运行时无需检查
- ✅ **快速反馈**：开发者立即得到错误信息
- ✅ **精确定位**：错误消息包含具体文件路径
- ⚠️ **限制**：需要访问所有 YAML 文件

---

#### 数据库模式：写入期检查 + 运行时验证

在数据库模式下，规则存储在数据库中，使用**两阶段检查策略**确保 ID 唯一性：

##### 第一阶段：写入期检查（Write-time Checking）

通过数据库约束在写入时保证 ID 唯一性：

```sql
-- 全局 ID 唯一性表
CREATE TABLE global_ids (
    id VARCHAR(255) PRIMARY KEY,           -- 保证全局唯一
    entity_type VARCHAR(50) NOT NULL,      -- 'rule' | 'ruleset' | 'pipeline'
    CHECK (entity_type IN ('rule', 'ruleset', 'pipeline'))
);

-- Rules 表使用 global_ids 的外键
CREATE TABLE rules (
    id VARCHAR(255) PRIMARY KEY REFERENCES global_ids(id),
    name VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    -- ...
);

-- Rulesets 表使用 global_ids 的外键
CREATE TABLE rulesets (
    id VARCHAR(255) PRIMARY KEY REFERENCES global_ids(id),
    name VARCHAR(255) NOT NULL,
    content TEXT NOT NULL,
    -- ...
);
```

**保存规则时的事务检查**：

```rust
impl DatabaseRepository {
    async fn save_rule(&self, rule: &Rule) -> Result<()> {
        // 使用事务保证原子性
        let mut tx = self.pool.begin().await?;

        // 1. 先在 global_ids 表中插入 ID（如果已存在会失败）
        sqlx::query!(
            r#"
            INSERT INTO global_ids (id, entity_type)
            VALUES ($1, 'rule')
            ON CONFLICT (id) DO NOTHING
            "#,
            rule.id
        )
        .execute(&mut *tx)
        .await?;

        // 2. 检查是否成功插入（防止 Rule/Ruleset ID 冲突）
        let existing = sqlx::query!(
            "SELECT entity_type FROM global_ids WHERE id = $1",
            rule.id
        )
        .fetch_one(&mut *tx)
        .await?;

        if existing.entity_type != "rule" {
            return Err(Error::IdConflict {
                id: rule.id.clone(),
                conflict: format!(
                    "ID '{}' already exists as a {}",
                    rule.id, existing.entity_type
                ),
            });
        }

        // 3. 插入 rule 数据
        let content = serde_yaml::to_string(rule)?;
        sqlx::query!(
            r#"
            INSERT INTO rules (id, name, description, content, ...)
            VALUES ($1, $2, $3, $4, ...)
            ON CONFLICT (id) DO UPDATE SET ...
            "#,
            rule.id, rule.name, rule.description, content
        )
        .execute(&mut *tx)
        .await?;

        // 4. 提交事务
        tx.commit().await?;

        Ok(())
    }
}
```

**特点**：
- ✅ **数据库级别保证**：通过 PRIMARY KEY 约束保证唯一性
- ✅ **跨命名空间检查**：`global_ids` 表防止 Rule/Ruleset ID 冲突
- ✅ **事务性**：保证检查和插入的原子性
- ⚠️ **错误反馈延迟**：错误在保存时（非编译时）发现

---

##### 第二阶段：运行时验证（Runtime Validation）

编译器在加载规则时进行额外验证（双重保险）：

```rust
impl Compiler {
    pub async fn compile_from_repository(&mut self, repo: &dyn RuleRepository) -> Result<Program> {
        // 1. 从数据库加载所有规则
        let rule_ids = repo.list_rules().await?;
        let mut rules = Vec::new();
        for id in rule_ids {
            let rule = repo.load_rule(&id).await?;
            rules.push(rule);
        }

        // 2. ✅ 运行时验证 ID 唯一性（额外保险）
        let mut seen_ids = HashSet::new();
        for rule in &rules {
            if !seen_ids.insert(&rule.id) {
                // 理论上不应该发生（数据库已保证），但提供额外检查
                return Err(CompilerError::DuplicateRuleId {
                    id: rule.id.clone(),
                    first_defined: "database".to_string(),
                    also_defined: "database".to_string(),
                });
            }
        }

        // 3. 继续编译流程
        self.compile_with_context(&rules, &rulesets)
    }
}
```

**特点**：
- ✅ **防御性编程**：即使数据库约束失效也能发现问题
- ✅ **一致性检查**：确保加载的数据符合预期
- ⚠️ **运行时开销**：需要遍历所有已加载的规则（但仅在编译时执行一次）

---

#### 对比总结

| 检查方式 | 文件系统模式 | 数据库模式 |
|---------|------------|-----------|
| **检查时机** | 编译期（Compile-time） | 写入期（Write-time）+ 运行时（Runtime） |
| **实现方式** | `ImportResolver::validate_unique_ids()` | 数据库约束 + 运行时验证 |
| **唯一性保证** | 编译器检查所有文件 | `global_ids` 表的 PRIMARY KEY 约束 |
| **冲突检测** | 编译期扫描所有 Rule/Ruleset 文件 | 数据库事务检查 `global_ids.entity_type` |
| **错误反馈** | 立即（编译时） | 延迟（保存规则时） |
| **运行时开销** | 无（编译期已检查） | 有（运行时额外验证，但可选） |
| **部署要求** | 需要访问所有 YAML 文件 | 需要连接数据库 |

---

#### 为什么不需要编译期连接数据库？

有人可能会问："数据库模式下，为什么不在编译期连接数据库检查 ID 唯一性？"

**原因**：
1. **数据库约束已足够**：`global_ids` 表的 PRIMARY KEY 约束在写入时就保证了 ID 唯一性，无需编译期重复检查
2. **部署简化**：编译器无需数据库连接权限，降低部署复杂度
3. **性能考虑**：编译期连接数据库会显著增加编译时间
4. **职责分离**：
   - 编译器职责：将 YAML/数据库内容编译为可执行 IR
   - 数据库职责：保证数据完整性和唯一性约束

**两阶段检查的优势**：
- 写入期检查（数据库约束）：保证持久化数据的完整性
- 运行时验证（编译器）：提供额外的防御性检查
- 无需编译期数据库连接：简化部署和依赖

---

### 使用示例

```rust
// 在应用中使用

use corint_repository::{
    RuleRepository,
    FileSystemRepository,
    DatabaseRepository,
    FallbackRepository,
};

// 方式1: 文件系统模式（开发环境）
// ID 唯一性通过编译期检查保证
let repo: Box<dyn RuleRepository> = Box::new(
    FileSystemRepository::new("repository")
);

// 方式2: 数据库模式（生产环境）
// ID 唯一性通过数据库约束 + 运行时验证保证
let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&database_url)
    .await?;

let repo: Box<dyn RuleRepository> = Box::new(
    DatabaseRepository::new(pool)
);

// 方式3: 混合模式（生产环境 + 文件备份）
let db_repo = DatabaseRepository::new(pool);
let fs_repo = FileSystemRepository::new("repository");
let repo: Box<dyn RuleRepository> = Box::new(
    FallbackRepository::new(db_repo, fs_repo)
);

// 统一接口使用
let rule = repo.load_rule("fraud_farm_pattern").await?;
repo.save_rule(&updated_rule).await?;  // ✅ 数据库模式下会在此处检查 ID 唯一性
let all_rules = repo.list_rules().await?;
```

### 环境变量配置

```bash
# 文件系统模式
CORINT_REPOSITORY_TYPE=filesystem
CORINT_REPOSITORY_PATH=./repository

# 数据库模式
CORINT_REPOSITORY_TYPE=database
DATABASE_URL=postgresql://user:pass@localhost:5432/corint

# 混合模式
CORINT_REPOSITORY_TYPE=hybrid
CORINT_PRIMARY_REPOSITORY=database
DATABASE_URL=postgresql://user:pass@localhost:5432/corint
CORINT_FALLBACK_REPOSITORY=filesystem
CORINT_REPOSITORY_PATH=./repository
```

### 迁移路径

#### Phase 1: 文件系统（当前 - Week 1-2）
```
repository/library/rules/fraud/fraud_farm.yaml  ✅
```

#### Phase 2: Repository 抽象层（Week 3-4）
```rust
// 实现 RuleRepository trait
// 实现 FileSystemRepository
// 集成到 Compiler
```

#### Phase 3: 数据库支持（Week 5-6）
```rust
// 实现 DatabaseRepository
// 数据库迁移脚本
// 混合模式支持
```

#### Phase 4: 生产部署（Week 7+）
```
数据库模式 ✅
规则在线编辑 ✅
版本管理 ✅
```

---

## 📝 ID 命名规范建议

为了避免 ID 冲突和提高代码可读性，建议遵循以下命名规范：

### Rule ID 命名规范

**格式**：`<category>_<specific_pattern>`

| 分类 | 前缀 | 示例 |
|------|------|------|
| 欺诈检测 | `fraud_` | `fraud_farm_pattern`, `fraud_velocity_abuse` |
| 支付风控 | `payment_` | `payment_card_testing`, `payment_high_value` |
| 地理位置 | `geo_` | `geo_suspicious_country`, `geo_impossible_travel` |
| 账户安全 | `account_` | `account_takeover_pattern`, `account_new_user_risk` |
| 设备检测 | `device_` | `device_fingerprint_mismatch`, `device_multiple_users` |

**命名原则**：
- ✅ 使用 `snake_case`（小写+下划线）
- ✅ 包含类别前缀，避免全局冲突
- ✅ 描述性强，见名知意
- ✅ 使用 `_pattern` 或 `_check` 后缀表示这是一个规则
- ❌ 避免使用通用词汇（如 `rule1`, `check`, `test`）
- ❌ 避免过长（建议 < 50 字符）

**示例**：

```yaml
# ✅ 好的命名
rule:
  id: fraud_farm_pattern           # 清晰：欺诈农场模式
  id: payment_velocity_check        # 清晰：支付频率检查
  id: geo_impossible_travel_pattern # 清晰：不可能旅行模式

# ❌ 不好的命名
rule:
  id: fraud                         # 太宽泛
  id: rule1                         # 无意义
  id: check_user_payment_behavior_and_device_fingerprint  # 太长
```

### Ruleset ID 命名规范

**格式**：`<domain>_<purpose>_<variant?>`

| 领域 | 示例 |
|------|------|
| 欺诈检测 | `fraud_detection_core`, `fraud_detection_advanced` |
| 支付风控 | `payment_standard`, `payment_high_value`, `payment_strict` |
| 交易审核 | `transaction_risk_assessment`, `transaction_compliance` |
| 用户验证 | `user_verification_basic`, `user_verification_enhanced` |

**命名原则**：
- ✅ 反映业务场景，不仅仅是技术实现
- ✅ 如果有多个变体，使用后缀区分（`_core`, `_standard`, `_strict`, `_v2`）
- ✅ 避免与 Rule ID 冲突（不要使用 `_pattern` 后缀）

**示例**：

```yaml
# ✅ 好的命名
ruleset:
  id: fraud_detection_core          # 核心欺诈检测规则集
  id: payment_high_value            # 高额支付规则集
  id: user_onboarding_verification  # 用户注册验证规则集

# ❌ 不好的命名
ruleset:
  id: fraud_detection               # 与 rule ID 可能冲突
  id: rules_v1                      # 太宽泛
  id: payment                       # 与分类名冲突
```

### ID 唯一性检查清单

在添加新的 Rule 或 Ruleset 时，确保：

- [ ] ID 遵循命名规范
- [ ] 在 `library/rules/` 中没有相同的 Rule ID
- [ ] 在 `library/rulesets/` 中没有相同的 Ruleset ID
- [ ] Rule ID 和 Ruleset ID 之间没有冲突
- [ ] 运行编译器验证（编译器会自动检查）

### 自动化检查工具

可以创建一个 lint 工具来验证 ID 命名规范：

```bash
# scripts/lint_ids.sh
#!/bin/bash

echo "Checking Rule IDs..."
find repository/library/rules -name "*.yaml" -exec grep -H "^  id:" {} \; | \
  grep -v "_pattern$\|_check$" && \
  echo "Warning: Some Rule IDs don't follow naming convention"

echo "Checking for duplicate IDs..."
find repository/library -name "*.yaml" -exec grep "^  id:" {} \; | \
  sort | uniq -d && \
  echo "Error: Duplicate IDs found!"
```

---

## 📚 参考资料

- [YAML 多文档规范](https://yaml.org/spec/1.2/spec.html#id2800132)
- [Rust serde_yaml 文档](https://docs.rs/serde_yaml/)
- [模块化规则引擎最佳实践](https://martinfowler.com/bliki/RulesEngine.html)
- [Repository Pattern](https://martinfowler.com/eaaCatalog/repository.html)
- [sqlx 文档](https://docs.rs/sqlx/)

---

**文档版本**: 2.1
**最后更新**: 2024-12-11
**作者**: Corint Team
