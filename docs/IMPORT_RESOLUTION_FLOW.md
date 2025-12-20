# Pipeline Import 解析和编译流程

## 📋 概述

Pipeline 中的 `import` 功能允许引用外部的 rules 和 rulesets。整个流程分为三个阶段：

1. **Parser 阶段**：解析 YAML，提取 imports 声明
2. **Import Resolver 阶段**：加载并合并所有引用的内容
3. **Compiler 阶段**：将合并后的内容编译成 IR

## 🔄 完整流程

### 阶段 1: Parser（解析阶段）

**位置**：`corint-parser/src/pipeline_parser.rs`

**功能**：解析 YAML 文件，提取 imports 声明，但不加载实际内容

```rust
// PipelineParser::parse_with_imports()
let document = PipelineParser::parse_with_imports(yaml_content)?;
// 返回 RdlDocument<Pipeline>，包含：
// - document.imports: Option<Imports>  // imports 声明（路径列表）
// - document.definition: Pipeline     // pipeline 定义
```

**输入示例**：
```yaml
version: "0.1"
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml
    - library/rulesets/payment_high_value.yaml
---
pipeline:
  id: fraud_detection_pipeline
  steps:
    - include:
        ruleset: fraud_detection_core
```

**输出**：
- `document.imports` = `Some(Imports { rulesets: ["library/rulesets/...", ...] })`
- `document.definition` = `Pipeline { id: "fraud_detection_pipeline", ... }`

**关键点**：
- ✅ 只解析 imports 声明，不加载文件内容
- ✅ 支持多文档 YAML（用 `---` 分隔）
- ✅ 兼容旧格式（无 imports）

### 阶段 2: Import Resolver（合并阶段）

**位置**：`corint-compiler/src/import_resolver.rs`

**功能**：根据 imports 声明，从 repository 加载所有引用的内容并合并

```rust
// ImportResolver::resolve_imports()
let resolved = compiler
    .import_resolver_mut()
    .resolve_imports(&document)?;
// 返回 ResolvedDocument，包含：
// - resolved.rules: Vec<Rule>        // 所有合并的 rules
// - resolved.rulesets: Vec<Ruleset>  // 所有合并的 rulesets
```

**详细流程**：

```rust
pub fn resolve_imports<T>(&mut self, document: &RdlDocument<T>) -> Result<ResolvedDocument> {
    let mut resolved_rules = Vec::new();
    let mut resolved_rulesets = Vec::new();

    // 1. 加载直接导入的 rules
    if let Some(imports) = &document.imports {
        for rule_path in &imports.rules {
            let (rule, _) = self.load_rule(rule_path)?;  // 从文件系统加载
            resolved_rules.push(rule);
        }

        // 2. 加载直接导入的 rulesets（递归加载其依赖）
        for ruleset_path in &imports.rulesets {
            let (ruleset, deps) = self.load_ruleset_with_deps(ruleset_path)?;
            // deps.rules 包含 ruleset 导入的所有 rules
            
            // 添加 ruleset 的依赖 rules
            resolved_rules.extend(deps.rules);
            
            // 添加 ruleset 本身
            resolved_rulesets.push(ruleset);
        }
    }

    // 3. 去重（同一个 rule/ruleset 可能被多次引用）
    resolved_rules = self.deduplicate_rules(resolved_rules)?;
    resolved_rulesets = self.deduplicate_rulesets(resolved_rulesets)?;

    // 4. 验证 ID 唯一性
    self.validate_unique_ids(&resolved_rules, &resolved_rulesets)?;

    Ok(ResolvedDocument {
        rules: resolved_rules,
        rulesets: resolved_rulesets,
    })
}
```

**关键方法：`load_ruleset_with_deps`**

这个方法会递归加载 ruleset 的所有依赖：

```rust
fn load_ruleset_with_deps(&mut self, path: &str) -> Result<(Ruleset, Dependencies)> {
    // 1. 检查缓存
    if let Some(cached) = self.ruleset_cache.get(path) {
        return Ok(cached.clone());
    }

    // 2. 防止循环依赖
    if self.loading_stack.contains(path) {
        return Err(CompileError::CircularDependency { ... });
    }
    self.loading_stack.push(path.to_string());

    // 3. 加载文件并解析
    let content = std::fs::read_to_string(&full_path)?;
    let document = RulesetParser::parse_with_imports(&content)?;

    // 4. 递归加载 ruleset 的 imports（依赖传播）
    let mut deps_rules = Vec::new();
    if let Some(imports) = &document.imports {
        // 加载 ruleset 导入的 rules
        for rule_path in &imports.rules {
            let (rule, _) = self.load_rule(rule_path)?;
            deps_rules.push(rule);
        }

        // 支持 rulesets 导入其他 rulesets（更深层的传播）
        for ruleset_path in &imports.rulesets {
            let (_, sub_deps) = self.load_ruleset_with_deps(ruleset_path)?;
            deps_rules.extend(sub_deps.rules);
        }
    }

    // 5. 处理继承（extends）
    let mut ruleset = document.definition;
    if let Some(extends_id) = ruleset.extends.clone() {
        ruleset = self.apply_inheritance(ruleset, &extends_id, path)?;
    }

    // 6. 应用模板（decision_template）
    if let Some(template_ref) = ruleset.decision_template.clone() {
        ruleset = self.apply_template(ruleset, &template_ref)?;
    }

    // 7. 缓存结果
    self.ruleset_cache.insert(path.to_string(), (ruleset.clone(), Dependencies { rules: deps_rules.clone() }));

    Ok((ruleset, Dependencies { rules: deps_rules }))
}
```

**关键点**：
- ✅ **递归加载**：ruleset 的 imports 会被递归加载
- ✅ **依赖传播**：导入 ruleset 时，自动获取其所有依赖的 rules
- ✅ **去重**：相同 rule/ruleset 只保留一份
- ✅ **循环检测**：防止循环依赖
- ✅ **缓存**：已加载的内容会被缓存

### 阶段 3: Compiler（编译阶段）

**位置**：`corint-sdk/src/decision_engine.rs`

**功能**：将合并后的内容编译成 IR

```rust
// 在 compile_rules_from_content() 中
// 1. 解析 pipeline（包含 imports 声明）
let document = PipelineParser::parse_with_imports(content)?;

// 2. 解析 imports（合并所有引用的内容）
let resolved = compiler
    .import_resolver_mut()
    .resolve_imports(&document)?;

// 3. 编译所有合并的 rules
for rule in &resolved.rules {
    let rule_prog = compiler.compile_rule(rule)?;
    programs.push(rule_prog);
}

// 4. 编译所有合并的 rulesets
for ruleset in &resolved.rulesets {
    let ruleset_prog = compiler.compile_ruleset(ruleset)?;
    programs.push(ruleset_prog);
}

// 5. 最后编译 pipeline 本身
let prog = compiler.compile_pipeline(&document.definition)?;
programs.push(prog);
```

**关键点**：
- ✅ **顺序编译**：先编译 rules，再编译 rulesets，最后编译 pipeline
- ✅ **完整上下文**：pipeline 编译时，所有依赖的 rules/rulesets 已经编译完成
- ✅ **独立 Program**：每个 rule/ruleset/pipeline 编译成独立的 Program

## 📊 流程图

```
┌─────────────────────────────────────────────────────────┐
│  Pipeline YAML File                                     │
│  ┌───────────────────────────────────────────────────┐ │
│  │ version: "0.1"                                   │ │
│  │ imports:                                          │ │
│  │   rulesets:                                      │ │
│  │     - library/rulesets/fraud_detection.yaml     │ │
│  │ ---                                               │ │
│  │ pipeline:                                        │ │
│  │   id: my_pipeline                                │ │
│  │   steps: [...]                                   │ │
│  └───────────────────────────────────────────────────┘ │
└───────────────────────┬─────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 1: Parser (corint-parser)                        │
│  ┌───────────────────────────────────────────────────┐ │
│  │ PipelineParser::parse_with_imports()             │ │
│  │                                                   │ │
│  │ 输出: RdlDocument<Pipeline>                      │ │
│  │  - imports: Some(Imports {                        │ │
│  │      rulesets: ["library/rulesets/..."]          │ │
│  │    })                                             │ │
│  │  - definition: Pipeline { ... }                  │ │
│  └───────────────────────────────────────────────────┘ │
└───────────────────────┬─────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 2: Import Resolver (corint-compiler)             │
│  ┌───────────────────────────────────────────────────┐ │
│  │ ImportResolver::resolve_imports()                 │ │
│  │                                                   │ │
│  │ 1. 遍历 imports.rulesets                          │ │
│  │    ├─> load_ruleset_with_deps("fraud_detection") │ │
│  │    │   ├─> 加载文件                               │ │
│  │    │   ├─> 解析 YAML                             │ │
│  │    │   ├─> 递归加载 ruleset 的 imports          │ │
│  │    │   └─> 处理继承和模板                         │ │
│  │    └─> 返回 (Ruleset, Dependencies { rules })    │ │
│  │                                                   │ │
│  │ 2. 合并所有依赖                                   │ │
│  │    - resolved_rules.extend(deps.rules)           │ │
│  │    - resolved_rulesets.push(ruleset)             │ │
│  │                                                   │ │
│  │ 3. 去重                                           │ │
│  │    - deduplicate_rules()                         │ │
│  │    - deduplicate_rulesets()                      │ │
│  │                                                   │ │
│  │ 4. 验证                                           │ │
│  │    - validate_unique_ids()                       │ │
│  │                                                   │ │
│  │ 输出: ResolvedDocument                            │ │
│  │  - rules: Vec<Rule>        // 所有合并的 rules   │ │
│  │  - rulesets: Vec<Ruleset> // 所有合并的 rulesets│ │
│  └───────────────────────────────────────────────────┘ │
└───────────────────────┬─────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────────┐
│  Stage 3: Compiler (corint-compiler)                    │
│  ┌───────────────────────────────────────────────────┐ │
│  │ 1. 编译所有 rules                                 │ │
│  │    for rule in resolved.rules {                  │ │
│  │      let prog = compiler.compile_rule(rule)?;     │ │
│  │      programs.push(prog);                        │ │
│  │    }                                              │ │
│  │                                                   │ │
│  │ 2. 编译所有 rulesets                              │ │
│  │    for ruleset in resolved.rulesets {            │ │
│  │      let prog = compiler.compile_ruleset(ruleset)?;│ │
│  │      programs.push(prog);                        │ │
│  │    }                                              │ │
│  │                                                   │ │
│  │ 3. 编译 pipeline                                  │ │
│  │    let prog = compiler.compile_pipeline(pipeline)?;│ │
│  │    programs.push(prog);                          │ │
│  │                                                   │ │
│  │ 输出: Vec<Program>                                │ │
│  │  - 每个 rule/ruleset/pipeline 一个 Program      │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

## 🔍 详细示例

### 示例：Pipeline 导入 Ruleset

**Pipeline 文件** (`my_pipeline.yaml`):
```yaml
version: "0.1"
imports:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml
---
pipeline:
  id: fraud_pipeline
  steps:
    - include:
        ruleset: fraud_detection_core
```

**Ruleset 文件** (`library/rulesets/fraud_detection_core.yaml`):
```yaml
version: "0.1"
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/fraud/velocity_abuse.yaml
---
ruleset:
  id: fraud_detection_core
  rules:
    - fraud_farm_pattern
    - velocity_abuse_pattern
```

**执行流程**：

1. **Parser 阶段**：
   ```rust
   let document = PipelineParser::parse_with_imports(pipeline_yaml)?;
   // document.imports = Some(Imports {
   //     rulesets: ["library/rulesets/fraud_detection_core.yaml"]
   // })
   ```

2. **Import Resolver 阶段**：
   ```rust
   let resolved = resolver.resolve_imports(&document)?;
   
   // 内部执行：
   // 1. 加载 "library/rulesets/fraud_detection_core.yaml"
   // 2. 发现它导入了 2 个 rules
   // 3. 加载这 2 个 rules
   // 4. 合并结果：
   //    resolved.rules = [
   //      Rule { id: "fraud_farm_pattern", ... },
   //      Rule { id: "velocity_abuse_pattern", ... }
   //    ]
   //    resolved.rulesets = [
   //      Ruleset { id: "fraud_detection_core", ... }
   //    ]
   ```

3. **Compiler 阶段**：
   ```rust
   // 编译 rule 1
   let prog1 = compiler.compile_rule(&resolved.rules[0])?;
   
   // 编译 rule 2
   let prog2 = compiler.compile_rule(&resolved.rules[1])?;
   
   // 编译 ruleset
   let prog3 = compiler.compile_ruleset(&resolved.rulesets[0])?;
   
   // 编译 pipeline
   let prog4 = compiler.compile_pipeline(&document.definition)?;
   
   // 最终得到 4 个独立的 Program
   ```

## ⚡ 关键特性

### 1. 依赖传播（Transitive Dependencies）

当 pipeline 导入 ruleset 时，自动获取 ruleset 的所有依赖：

```
Pipeline
  └─> imports Ruleset A
        └─> imports Rule 1, Rule 2
              └─> (Rule 1, Rule 2 自动包含在 resolved.rules 中)
```

### 2. 递归加载

支持多层嵌套的 imports：

```
Pipeline
  └─> imports Ruleset A
        └─> imports Ruleset B
              └─> imports Rule 1
                    └─> (Rule 1 会被加载)
```

### 3. 去重机制

如果同一个 rule 被多个 ruleset 引用，只保留一份：

```rust
// 多个 ruleset 都引用了同一个 rule
ruleset_a.imports.rules = ["rule1.yaml"]
ruleset_b.imports.rules = ["rule1.yaml"]

// 去重后，resolved.rules 中只有一份 rule1
resolved.rules = [rule1]  // 不是 [rule1, rule1]
```

### 4. 循环依赖检测

防止循环引用：

```rust
// 如果检测到循环依赖
if self.loading_stack.contains(path) {
    return Err(CompileError::CircularDependency {
        path: path.to_string(),
        stack: self.loading_stack.clone(),
    });
}
```

### 5. 缓存机制

已加载的内容会被缓存，避免重复加载：

```rust
// 检查缓存
if let Some(cached) = self.rule_cache.get(path) {
    return Ok(cached.clone());
}

// 加载后缓存
self.rule_cache.insert(path.to_string(), (rule.clone(), content));
```

## 📝 总结

### Import 合并时机

**在编译之前，Import Resolver 阶段完成合并**：

1. ✅ **Parser 阶段**：只解析 imports 声明，不加载内容
2. ✅ **Import Resolver 阶段**：加载并合并所有引用的内容（**关键阶段**）
3. ✅ **Compiler 阶段**：编译合并后的完整内容

### 编译顺序

1. 先编译所有 rules
2. 再编译所有 rulesets
3. 最后编译 pipeline

### 关键代码位置

- **Parser**：`corint-parser/src/pipeline_parser.rs` → `parse_with_imports()`
- **Import Resolver**：`corint-compiler/src/import_resolver.rs` → `resolve_imports()`
- **Compiler**：`corint-sdk/src/decision_engine.rs` → `compile_rules_from_content()`

### 设计优势

1. ✅ **编译时解析**：所有依赖在编译时确定，运行时无开销
2. ✅ **完整上下文**：编译 pipeline 时，所有依赖已加载并编译
3. ✅ **模块化**：每个 rule/ruleset/pipeline 编译成独立的 Program
4. ✅ **可缓存**：Import Resolver 有缓存机制，提高性能
5. ✅ **类型安全**：编译时验证所有引用是否存在

