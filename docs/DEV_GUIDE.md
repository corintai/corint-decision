# CORINT Decision Engine - Development Guide

**Complete Rust Implementation Guide for CORINT RDL**

This guide provides a comprehensive roadmap for implementing the CORINT Decision Engine in Rust, from DSL parsing to production-ready execution.

---

## 🏗️ Architecture Overview

### System Layers

```
┌─────────────────────────────────────────────────────────────┐
│  Application Layer (User Code)                              │
│  - REST API Server                                          │
│  - gRPC Server                                              │
│  - CLI Tools                                                │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  SDK Layer (corint-sdk)                                     │
│  - DecisionEngine API                                       │
│  - Configuration Management                                 │
│  - Result Types                                             │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Runtime Layer (corint-runtime)                             │
│  - Pipeline Executor                                        │
│  - Rule Engine                                              │
│  - Feature Extractor                                        │
│  - LLM Integrator                                           │
│  - Context Manager                                          │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Compiler Layer (corint-compiler)                           │
│  - Parser (YAML → AST)                                      │
│  - Semantic Analyzer                                        │
│  - Type Checker                                             │
│  - Optimizer                                                │
│  - IR Generator (AST → IR)                                  │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Core Layer (corint-core)                                   │
│  - AST Definitions                                          │
│  - IR (Intermediate Representation)                         │
│  - Type System                                              │
│  - Expression Evaluator                                     │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│  Infrastructure Layer                                       │
│  - Cache (Redis/In-Memory)                                  │
│  - Database Connectors                                      │
│  - LLM Providers                                            │
│  - External APIs                                            │
│  - Metrics & Tracing                                        │
└─────────────────────────────────────────────────────────────┘
```

---

## 📦 Project Structure

```bash
corint-decision/
├── Cargo.toml                    # Workspace configuration
├── crates/
│   ├── corint-core/              # Core type definitions
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── ast/              # AST definitions
│   │   │   │   ├── mod.rs
│   │   │   │   ├── rule.rs       # Rule AST
│   │   │   │   ├── ruleset.rs    # Ruleset AST
│   │   │   │   ├── pipeline.rs   # Pipeline AST
│   │   │   │   ├── expression.rs # Expression AST
│   │   │   │   └── types.rs      # Type definitions
│   │   │   ├── ir/               # Intermediate Representation
│   │   │   │   ├── mod.rs
│   │   │   │   ├── instruction.rs
│   │   │   │   └── program.rs
│   │   │   ├── types/            # Type system
│   │   │   │   ├── mod.rs
│   │   │   │   ├── value.rs      # Runtime Value
│   │   │   │   ├── schema.rs     # Schema definitions
│   │   │   │   └── validator.rs
│   │   │   └── error.rs          # Error types
│   │   └── Cargo.toml
│   │
│   ├── corint-parser/            # YAML → AST parser
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── yaml_parser.rs    # YAML parsing
│   │   │   ├── rule_parser.rs    # Rule parsing
│   │   │   ├── ruleset_parser.rs
│   │   │   ├── pipeline_parser.rs
│   │   │   ├── expression_parser.rs
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   │
│   ├── corint-compiler/          # AST → IR compiler
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── compiler.rs       # Main compiler
│   │   │   ├── semantic/         # Semantic analysis
│   │   │   │   ├── mod.rs
│   │   │   │   ├── analyzer.rs
│   │   │   │   └── type_checker.rs
│   │   │   ├── codegen/          # IR generation
│   │   │   │   ├── mod.rs
│   │   │   │   ├── rule_codegen.rs
│   │   │   │   ├── ruleset_codegen.rs
│   │   │   │   └── pipeline_codegen.rs
│   │   │   ├── optimizer/        # Optimizer
│   │   │   │   ├── mod.rs
│   │   │   │   ├── constant_folding.rs
│   │   │   │   └── dead_code_elimination.rs
│   │   │   └── error.rs
│   │   └── Cargo.toml
│   │
│   ├── corint-runtime/           # Runtime execution engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── engine/           # Execution engine
│   │   │   │   ├── mod.rs
│   │   │   │   ├── pipeline_executor.rs
│   │   │   │   ├── rule_executor.rs
│   │   │   │   └── expression_evaluator.rs
│   │   │   ├── context/          # Context management
│   │   │   │   ├── mod.rs
│   │   │   │   ├── execution_context.rs
│   │   │   │   └── variable_store.rs
│   │   │   ├── feature/          # Feature engineering
│   │   │   │   ├── mod.rs
│   │   │   │   ├── extractor.rs
│   │   │   │   ├── aggregator.rs  # count_distinct, percentile, etc.
│   │   │   │   └── statistics.rs
│   │   │   ├── llm/              # LLM integration
│   │   │   │   ├── mod.rs
│   │   │   │   ├── provider.rs   # LLM Provider trait
│   │   │   │   ├── openai.rs
│   │   │   │   ├── anthropic.rs
│   │   │   │   └── cache.rs      # LLM caching
│   │   │   ├── service/          # Service integration
│   │   │   │   ├── mod.rs
│   │   │   │   ├── database.rs
│   │   │   │   ├── cache.rs
│   │   │   │   └── external_api.rs
│   │   │   └── observability/    # Observability
│   │   │       ├── mod.rs
│   │   │       ├── metrics.rs
│   │   │       ├── tracing.rs
│   │   │       └── audit.rs
│   │   └── Cargo.toml
│   │
│   ├── corint-sdk/               # User SDK
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── decision_engine.rs # Main API
│   │   │   ├── config.rs         # Configuration
│   │   │   ├── result.rs         # Decision result
│   │   │   └── builder.rs        # Builder pattern
│   │   └── Cargo.toml
│   │
│   └── corint-server/            # HTTP/gRPC Server
│       ├── src/
│       │   ├── main.rs
│       │   ├── api/
│       │   │   ├── mod.rs
│       │   │   ├── rest.rs       # REST API
│       │   │   └── grpc.rs       # gRPC API
│       │   └── config.rs
│       └── Cargo.toml
│
├── examples/                     # Examples
│   ├── simple_rule.rs
│   ├── account_takeover.rs
│   └── complete_pipeline.rs
│
└── tests/                        # Integration tests
    ├── compiler_tests.rs
    ├── runtime_tests.rs
    └── e2e_tests.rs
```

---

## 🔧 Core Implementation

### 1. Core Layer: AST Definitions

#### `crates/corint-core/src/ast/rule.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub when: WhenBlock,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhenBlock {
    pub event_type: Option<String>,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    Binary(BinaryCondition),
    LLM(LLMCondition),
    External(ExternalCondition),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryCondition {
    pub left: Expression,
    pub op: Operator,
    pub right: Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    // Field access: event.user.id
    FieldAccess(Vec<String>),

    // Literals
    Literal(Literal),

    // Function call: count_distinct(device.id, {ip == event.ip}, last_5h)
    FunctionCall {
        name: String,
        args: Vec<Expression>,
    },

    // Binary operation
    Binary {
        left: Box<Expression>,
        op: Operator,
        right: Box<Expression>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Literal {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operator {
    Eq, Ne, Lt, Gt, Le, Ge,
    In, NotIn,
    And, Or, Not,
    Add, Sub, Mul, Div,
}
```

#### `crates/corint-core/src/ast/ruleset.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ruleset {
    pub id: String,
    pub name: Option<String>,
    pub rules: Vec<String>, // Rule IDs
    pub decision_logic: Vec<DecisionRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRule {
    pub condition: Option<Expression>,
    pub default: bool,
    pub action: Action,
    pub reason: Option<String>,
    pub terminate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Action {
    Approve,
    Deny,
    Review,
    Infer { config: InferConfig },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferConfig {
    pub data_snapshot: Vec<String>,
}
```

#### `crates/corint-core/src/ast/pipeline.rs`

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pipeline {
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Step {
    Extract {
        id: String,
        features: Vec<FeatureDefinition>,
    },
    Reason {
        id: String,
        provider: String,
        model: String,
        prompt: PromptTemplate,
        output_schema: Option<Schema>,
    },
    Service {
        id: String,
        service: String,
        operation: String,
        params: HashMap<String, Expression>,
    },
    Include {
        ruleset: String,
    },
    Branch {
        branches: Vec<Branch>,
    },
    Parallel {
        steps: Vec<Step>,
        merge: MergeStrategy,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDefinition {
    pub name: String,
    pub value: Expression,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub condition: Expression,
    pub pipeline: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStrategy {
    All,
    Any,
    Fastest,
    Majority,
}
```

---

### 2. Parser Layer: YAML → AST

#### `crates/corint-parser/src/rule_parser.rs`

```rust
use corint_core::ast::{Rule, WhenBlock, Condition, BinaryCondition, Expression, Operator};
use serde_yaml::Value;
use anyhow::Result;

pub struct RuleParser;

impl RuleParser {
    pub fn parse(yaml_str: &str) -> Result<Rule> {
        let yaml: Value = serde_yaml::from_str(yaml_str)?;

        let rule_value = yaml.get("rule")
            .ok_or_else(|| anyhow::anyhow!("Missing 'rule' key"))?;

        // Parse id, name, description
        let id = rule_value.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing rule id"))?
            .to_string();

        let name = rule_value.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing rule name"))?
            .to_string();

        // Parse when block
        let when = Self::parse_when_block(rule_value)?;

        // Parse score
        let score = rule_value.get("score")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid score"))?
            as i32;

        Ok(Rule {
            id,
            name,
            description: rule_value.get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            when,
            score,
        })
    }

    fn parse_when_block(rule_value: &Value) -> Result<WhenBlock> {
        let when = rule_value.get("when")
            .ok_or_else(|| anyhow::anyhow!("Missing 'when' block"))?;

        // Parse event type
        let event_type = when.as_mapping()
            .and_then(|m| m.iter().find(|(k, _)| {
                k.as_str() == Some("event.type")
            }))
            .and_then(|(_, v)| v.as_str())
            .map(|s| s.to_string());

        // Parse conditions
        let conditions = when.get("conditions")
            .and_then(|v| v.as_sequence())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid conditions"))?
            .iter()
            .map(|c| Self::parse_condition(c))
            .collect::<Result<Vec<_>>>()?;

        Ok(WhenBlock {
            event_type,
            conditions,
        })
    }

    fn parse_condition(value: &Value) -> Result<Condition> {
        // Parse condition expression
        // Example: "device.id not_in user.known_devices"
        let condition_str = value.as_str()
            .ok_or_else(|| anyhow::anyhow!("Condition must be a string"))?;

        // Use expression parser
        ExpressionParser::parse(condition_str)
    }
}
```

#### `crates/corint-parser/src/expression_parser.rs`

```rust
use corint_core::ast::{Expression, Operator, Literal, Condition, BinaryCondition};
use anyhow::Result;

pub struct ExpressionParser;

impl ExpressionParser {
    pub fn parse(expr_str: &str) -> Result<Condition> {
        // Use nom or pest for parsing
        // Simplified example

        let parts: Vec<&str> = expr_str.split_whitespace().collect();

        if parts.len() == 3 {
            let left = Self::parse_operand(parts[0])?;
            let op = Self::parse_operator(parts[1])?;
            let right = Self::parse_operand(parts[2])?;

            Ok(Condition::Binary(BinaryCondition {
                left,
                op,
                right,
            }))
        } else {
            Err(anyhow::anyhow!("Invalid expression format"))
        }
    }

    fn parse_operand(s: &str) -> Result<Expression> {
        // Check if field access (contains '.')
        if s.contains('.') {
            let parts = s.split('.').map(String::from).collect();
            Ok(Expression::FieldAccess(parts))
        }
        // Check if number
        else if let Ok(num) = s.parse::<f64>() {
            Ok(Expression::Literal(Literal::Number(num)))
        }
        // Check if string (quoted)
        else if s.starts_with('"') && s.ends_with('"') {
            let s = s[1..s.len()-1].to_string();
            Ok(Expression::Literal(Literal::String(s)))
        }
        // Check if function call
        else if s.contains('(') {
            Self::parse_function_call(s)
        }
        else {
            Err(anyhow::anyhow!("Unknown operand type: {}", s))
        }
    }

    fn parse_operator(s: &str) -> Result<Operator> {
        match s {
            "==" => Ok(Operator::Eq),
            "!=" => Ok(Operator::Ne),
            "<" => Ok(Operator::Lt),
            ">" => Ok(Operator::Gt),
            "<=" => Ok(Operator::Le),
            ">=" => Ok(Operator::Ge),
            "in" => Ok(Operator::In),
            "not_in" => Ok(Operator::NotIn),
            _ => Err(anyhow::anyhow!("Unknown operator: {}", s)),
        }
    }

    fn parse_function_call(s: &str) -> Result<Expression> {
        // Parse function calls like: count_distinct(device.id, {ip == event.ip}, last_5h)
        // Actual implementation requires complex parsing logic
        todo!("Implement function call parsing")
    }
}
```

---

### 3. Compiler Layer: AST → IR

#### `crates/corint-core/src/ir/instruction.rs`

```rust
use corint_core::ast::{Operator, Action};
use corint_core::types::Value;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Instruction {
    // Data loading
    LoadField { path: Vec<String> },
    LoadConst { value: Value },

    // Operations
    BinaryOp { op: Operator },
    Compare { op: Operator },

    // Control flow
    Jump { offset: isize },
    JumpIfTrue { offset: isize },
    JumpIfFalse { offset: isize },

    // Event checking
    CheckEventType { expected: String },

    // Feature extraction
    CallFeature {
        feature_type: FeatureType,
        field: Vec<String>,
        filter: Option<Box<Expression>>,
        time_window: TimeWindow,
    },

    // LLM call
    CallLLM {
        provider: String,
        model: String,
        prompt: String,
    },

    // Service call
    CallService {
        service: String,
        operation: String,
        params: HashMap<String, Value>,
    },

    // Decision
    SetScore { value: i32 },
    SetAction { action: Action },

    // Other
    Return,
}

#[derive(Debug, Clone)]
pub enum FeatureType {
    Count,
    CountDistinct,
    Sum,
    Avg,
    Percentile { p: f64 },
    StdDev,
}

#[derive(Debug, Clone)]
pub enum TimeWindow {
    Last1Hour,
    Last24Hours,
    Last7Days,
    Last30Days,
    Custom { duration: chrono::Duration },
}
```

#### `crates/corint-compiler/src/compiler.rs`

```rust
use corint_core::ast::{Rule, Ruleset, Pipeline, Expression, Condition, BinaryCondition};
use corint_core::ir::{Program, Instruction, FeatureType};
use anyhow::Result;

pub struct Compiler {
    // Symbol table: store rules, rulesets, etc.
    symbol_table: SymbolTable,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            symbol_table: SymbolTable::new(),
        }
    }

    pub fn compile_rule(&mut self, rule: Rule) -> Result<Program> {
        // 1. Semantic analysis
        self.analyze_rule(&rule)?;

        // 2. Type checking
        self.type_check_rule(&rule)?;

        // 3. Generate IR
        let instructions = self.codegen_rule(&rule)?;

        // 4. Optimize
        let optimized = self.optimize(instructions)?;

        Ok(Program {
            instructions: optimized,
            metadata: ProgramMetadata {
                rule_id: rule.id.clone(),
                ..Default::default()
            },
        })
    }

    fn codegen_rule(&self, rule: &Rule) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Generate event type check instruction
        if let Some(event_type) = &rule.when.event_type {
            instructions.push(Instruction::CheckEventType {
                expected: event_type.clone(),
            });
        }

        // Generate condition check instructions
        for condition in &rule.when.conditions {
            let condition_instructions = self.codegen_condition(condition)?;
            instructions.extend(condition_instructions);
        }

        // Generate score instruction
        instructions.push(Instruction::SetScore {
            value: rule.score,
        });

        Ok(instructions)
    }

    fn codegen_condition(&self, condition: &Condition) -> Result<Vec<Instruction>> {
        match condition {
            Condition::Binary(binary) => {
                self.codegen_binary_condition(binary)
            }
            Condition::LLM(llm) => {
                self.codegen_llm_condition(llm)
            }
            Condition::External(ext) => {
                self.codegen_external_condition(ext)
            }
        }
    }

    fn codegen_binary_condition(&self, cond: &BinaryCondition) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // 1. Calculate left operand
        let left_instructions = self.codegen_expression(&cond.left)?;
        instructions.extend(left_instructions);

        // 2. Calculate right operand
        let right_instructions = self.codegen_expression(&cond.right)?;
        instructions.extend(right_instructions);

        // 3. Execute comparison
        instructions.push(Instruction::Compare {
            op: cond.op.clone(),
        });

        // 4. If condition not met, skip subsequent instructions
        instructions.push(Instruction::JumpIfFalse {
            offset: 2, // Skip SetScore and Return
        });

        Ok(instructions)
    }

    fn codegen_expression(&self, expr: &Expression) -> Result<Vec<Instruction>> {
        match expr {
            Expression::FieldAccess(path) => {
                Ok(vec![Instruction::LoadField {
                    path: path.clone(),
                }])
            }

            Expression::Literal(lit) => {
                Ok(vec![Instruction::LoadConst {
                    value: self.literal_to_value(lit),
                }])
            }

            Expression::FunctionCall { name, args } => {
                self.codegen_function_call(name, args)
            }

            Expression::Binary { left, op, right } => {
                let mut instructions = Vec::new();
                instructions.extend(self.codegen_expression(left)?);
                instructions.extend(self.codegen_expression(right)?);
                instructions.push(Instruction::BinaryOp {
                    op: op.clone(),
                });
                Ok(instructions)
            }
        }
    }

    fn codegen_function_call(&self, name: &str, args: &[Expression]) -> Result<Vec<Instruction>> {
        match name {
            "count_distinct" => self.codegen_count_distinct(args),
            "percentile" => self.codegen_percentile(args),
            "count" => self.codegen_count(args),
            _ => Err(anyhow::anyhow!("Unknown function: {}", name)),
        }
    }

    fn codegen_count_distinct(&self, args: &[Expression]) -> Result<Vec<Instruction>> {
        // count_distinct(device.id, {ip == event.ip}, last_5h)
        // Generate:
        // 1. Query historical data
        // 2. Filter condition
        // 3. Distinct count

        Ok(vec![
            Instruction::CallFeature {
                feature_type: FeatureType::CountDistinct,
                field: /* extract from args[0] */,
                filter: /* extract from args[1] */,
                time_window: /* extract from args[2] */,
            },
        ])
    }
}
```

---

### 4. Runtime Layer: IR Execution

#### `crates/corint-runtime/src/engine/pipeline_executor.rs`

```rust
use corint_core::ir::{Program, Instruction};
use corint_core::types::Value;
use std::collections::HashMap;
use anyhow::Result;

pub struct PipelineExecutor {
    // Feature extractor
    feature_extractor: FeatureExtractor,

    // LLM client
    llm_client: LLMClient,

    // Service client
    service_client: ServiceClient,

    // Cache
    cache: Cache,
}

impl PipelineExecutor {
    pub async fn execute(
        &self,
        program: &Program,
        event: &Event,
    ) -> Result<DecisionResult> {
        let mut context = ExecutionContext::new(event.clone());
        let mut pc = 0; // Program Counter

        while pc < program.instructions.len() {
            let instruction = &program.instructions[pc];

            match instruction {
                Instruction::LoadField { path } => {
                    let value = self.load_field(&context, path)?;
                    context.stack.push(value);
                    pc += 1;
                }

                Instruction::LoadConst { value } => {
                    context.stack.push(value.clone());
                    pc += 1;
                }

                Instruction::BinaryOp { op } => {
                    let right = context.stack.pop()
                        .ok_or_else(|| anyhow::anyhow!("Stack underflow"))?;
                    let left = context.stack.pop()
                        .ok_or_else(|| anyhow::anyhow!("Stack underflow"))?;

                    let result = self.execute_binary_op(&left, op, &right)?;
                    context.stack.push(result);
                    pc += 1;
                }

                Instruction::Compare { op } => {
                    let right = context.stack.pop().unwrap();
                    let left = context.stack.pop().unwrap();

                    let result = self.compare(&left, op, &right)?;
                    context.stack.push(Value::Boolean(result));
                    pc += 1;
                }

                Instruction::JumpIfFalse { offset } => {
                    let condition = context.stack.pop().unwrap();

                    if let Value::Boolean(false) = condition {
                        pc = (pc as isize + offset) as usize;
                    } else {
                        pc += 1;
                    }
                }

                Instruction::CallFeature { feature_type, field, filter, time_window } => {
                    let value = self.feature_extractor
                        .extract(feature_type, field, filter, time_window, &context)
                        .await?;

                    context.stack.push(value);
                    pc += 1;
                }

                Instruction::CallLLM { provider, model, prompt } => {
                    // Check cache
                    let cache_key = format!("llm:{}:{}:{}", provider, model, prompt);

                    let result = if let Some(cached) = self.cache.get(&cache_key).await? {
                        cached
                    } else {
                        let result = self.llm_client
                            .call(provider, model, prompt, &context)
                            .await?;

                        self.cache.set(&cache_key, &result, Duration::from_secs(3600)).await?;
                        result
                    };

                    context.stack.push(result);
                    pc += 1;
                }

                Instruction::SetScore { value } => {
                    context.score = *value;
                    pc += 1;
                }

                Instruction::Return => {
                    break;
                }

                _ => {
                    pc += 1;
                }
            }
        }

        Ok(DecisionResult {
            action: context.action,
            score: context.score,
            triggered_rules: context.triggered_rules,
            explanation: context.build_explanation(),
            context: context.variables,
        })
    }

    fn load_field(&self, context: &ExecutionContext, path: &[String]) -> Result<Value> {
        // Load field from event or context
        // Example: event.user.id → context.event_data["user"]["id"]

        let mut current = &context.event_data;

        for segment in path {
            current = current.get(segment)
                .ok_or_else(|| anyhow::anyhow!("Field not found: {}", segment))?;
        }

        Ok(current.clone())
    }
}
```

#### `crates/corint-runtime/src/feature/extractor.rs`

```rust
use corint_core::ir::{FeatureType, TimeWindow};
use corint_core::types::Value;
use anyhow::Result;
use std::sync::Arc;

pub struct FeatureExtractor {
    // Data storage client (for querying historical data)
    storage: Arc<dyn Storage>,
}

impl FeatureExtractor {
    pub async fn extract(
        &self,
        feature_type: &FeatureType,
        field: &[String],
        filter: &Option<Box<Expression>>,
        time_window: &TimeWindow,
        context: &ExecutionContext,
    ) -> Result<Value> {
        match feature_type {
            FeatureType::CountDistinct => {
                self.count_distinct(field, filter, time_window, context).await
            }

            FeatureType::Percentile { p } => {
                self.percentile(field, filter, time_window, *p, context).await
            }

            _ => todo!(),
        }
    }

    async fn count_distinct(
        &self,
        field: &[String],
        filter: &Option<Box<Expression>>,
        time_window: &TimeWindow,
        context: &ExecutionContext,
    ) -> Result<Value> {
        // 1. Calculate time range
        let (start_time, end_time) = self.calculate_time_range(time_window, context)?;

        // 2. Query historical data
        let events = self.storage.query_events(
            start_time,
            end_time,
            /* event_type */ None,
        ).await?;

        // 3. Apply filter
        let filtered_events: Vec<_> = if let Some(filter_expr) = filter {
            events.into_iter()
                .filter(|event| {
                    // Evaluate filter expression
                    self.evaluate_filter(filter_expr, event, context)
                        .unwrap_or(false)
                })
                .collect()
        } else {
            events
        };

        // 4. Extract field values and deduplicate
        let mut unique_values = std::collections::HashSet::new();

        for event in filtered_events {
            if let Some(value) = self.extract_field(field, &event) {
                unique_values.insert(value);
            }
        }

        Ok(Value::Number(unique_values.len() as f64))
    }

    fn calculate_time_range(
        &self,
        time_window: &TimeWindow,
        context: &ExecutionContext,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
        let now = context.event.timestamp;

        let start_time = match time_window {
            TimeWindow::Last1Hour => now - Duration::hours(1),
            TimeWindow::Last24Hours => now - Duration::hours(24),
            TimeWindow::Last7Days => now - Duration::days(7),
            TimeWindow::Last30Days => now - Duration::days(30),
            TimeWindow::Custom { duration } => now - *duration,
        };

        Ok((start_time, now))
    }
}
```

---

### 5. SDK Layer: User API

#### `crates/corint-sdk/src/decision_engine.rs`

```rust
use corint_parser::RuleParser;
use corint_compiler::Compiler;
use corint_runtime::PipelineExecutor;
use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Result;

pub struct DecisionEngine {
    compiler: Compiler,
    executor: Arc<PipelineExecutor>,
    programs: HashMap<String, Program>,
}

impl DecisionEngine {
    pub fn builder() -> DecisionEngineBuilder {
        DecisionEngineBuilder::new()
    }

    /// Load rule file
    pub fn load_rule_file(&mut self, path: &str) -> Result<()> {
        let yaml_content = std::fs::read_to_string(path)?;
        self.load_rule_yaml(&yaml_content)
    }

    /// Load rule YAML
    pub fn load_rule_yaml(&mut self, yaml: &str) -> Result<()> {
        // 1. Parse YAML → AST
        let rule = RuleParser::parse(yaml)?;

        // 2. Compile AST → IR
        let program = self.compiler.compile_rule(rule)?;

        // 3. Store compiled program
        self.programs.insert(program.metadata.rule_id.clone(), program);

        Ok(())
    }

    /// Execute decision
    pub async fn decide(&self, event: Event) -> Result<DecisionResult> {
        // Find matching pipeline
        let pipeline = self.find_matching_pipeline(&event)?;

        // Execute decision flow
        self.executor.execute(pipeline, &event).await
    }
}

// Builder Pattern
pub struct DecisionEngineBuilder {
    config: EngineConfig,
}

impl DecisionEngineBuilder {
    pub fn new() -> Self {
        Self {
            config: EngineConfig::default(),
        }
    }

    pub fn with_cache(mut self, cache: Arc<dyn Cache>) -> Self {
        self.config.cache = Some(cache);
        self
    }

    pub fn with_llm_provider(mut self, provider: Arc<dyn LLMProvider>) -> Self {
        self.config.llm_provider = Some(provider);
        self
    }

    pub fn with_storage(mut self, storage: Arc<dyn Storage>) -> Self {
        self.config.storage = Some(storage);
        self
    }

    pub fn build(self) -> Result<DecisionEngine> {
        let compiler = Compiler::new();

        let executor = Arc::new(PipelineExecutor::new(
            self.config.cache.ok_or_else(|| anyhow::anyhow!("Cache required"))?,
            self.config.llm_provider.ok_or_else(|| anyhow::anyhow!("LLM provider required"))?,
            self.config.storage.ok_or_else(|| anyhow::anyhow!("Storage required"))?,
        ));

        Ok(DecisionEngine {
            compiler,
            executor,
            programs: HashMap::new(),
        })
    }
}
```

---

### 6. Usage Example

#### `examples/simple_rule.rs`

```rust
use corint_sdk::{DecisionEngine, Event};
use serde_json::json;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Create decision engine
    let mut engine = DecisionEngine::builder()
        .with_cache(create_redis_cache()?)
        .with_llm_provider(create_openai_provider()?)
        .with_storage(create_postgres_storage()?)
        .build()?;

    // 2. Load rules
    engine.load_rule_file("rules/account_takeover.yml")?;
    engine.load_rule_file("rules/fraud_detection.yml")?;

    // 3. Create event
    let event = Event {
        event_type: "login".to_string(),
        timestamp: Utc::now(),
        data: json!({
            "user": {
                "id": "user_12345",
                "email": "alice@example.com"
            },
            "device": {
                "id": "device_new_001",
                "type": "iPhone",
                "os": "iOS 17"
            },
            "geo": {
                "ip": "192.168.1.100",
                "country": "US",
                "city": "San Francisco"
            },
            "failed_attempts": 7
        }),
    };

    // 4. Execute decision
    let result = engine.decide(event).await?;

    // 5. Handle result
    match result.action {
        Action::Deny => {
            println!("❌ Login denied");
            println!("Score: {}", result.score);
            println!("Reason: {}", result.explanation);
        }
        Action::Approve => {
            println!("✅ Login approved");
        }
        Action::Review => {
            println!("⚠️  Needs manual review");
            println!("Score: {}", result.score);
        }
        Action::Infer { .. } => {
            println!("🤖 AI analysis requested");
        }
    }

    Ok(())
}
```

---

## 🎯 Implementation Roadmap

### Phase 1: Core Foundation (1-2 months)

**Milestone 1.1: Core Types & AST**
- ✅ corint-core crate
- ✅ AST definitions (Rule, Ruleset, Pipeline, Expression)
- ✅ Type system (Value, Schema)

**Milestone 1.2: Parser**
- ✅ corint-parser crate
- ✅ YAML → AST parser
- ✅ Expression parser
- ✅ Unit tests

**Milestone 1.3: Simple Compiler**
- ✅ corint-compiler crate
- ✅ AST → IR compilation
- ✅ Basic optimization

### Phase 2: Runtime (2-3 months)

**Milestone 2.1: Expression Evaluator**
- ✅ Expression evaluation engine
- ✅ Basic operator support
- ✅ Field access

**Milestone 2.2: Rule Executor**
- ✅ Rule executor
- ✅ Condition evaluation
- ✅ Score calculation

**Milestone 2.3: Feature Engineering**
- ✅ count_distinct
- ✅ percentile
- ✅ Time-window queries

### Phase 3: Advanced Features (2-3 months)

**Milestone 3.1: LLM Integration**
- ✅ OpenAI provider
- ✅ Anthropic provider
- ✅ Caching mechanism

**Milestone 3.2: Service Integration**
- ✅ Database connectors
- ✅ Cache (Redis)
- ✅ External APIs

**Milestone 3.3: Pipeline Executor**
- ✅ Complete Pipeline execution
- ✅ Parallel execution
- ✅ Branch logic

### Phase 4: Production Ready (1-2 months)

**Milestone 4.1: Observability**
- ✅ Metrics (Prometheus)
- ✅ Tracing (OpenTelemetry)
- ✅ Audit logs

**Milestone 4.2: Performance Optimization**
- ✅ Multi-level caching
- ✅ Connection pooling
- ✅ Concurrency optimization

**Milestone 4.3: API Server**
- ✅ REST API
- ✅ gRPC API
- ✅ WebSocket (real-time rule updates)

---

## 📚 Key Technology Stack

### Workspace `Cargo.toml`

```toml
[workspace]
members = [
    "crates/corint-core",
    "crates/corint-parser",
    "crates/corint-compiler",
    "crates/corint-runtime",
    "crates/corint-sdk",
    "crates/corint-server",
]

[workspace.dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
anyhow = "1.0"
thiserror = "1.0"

# Async
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"

# Parsing
nom = "7.1"      # or pest = "2.7"

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres"] }

# Cache
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# HTTP
axum = "0.7"
tower = "0.4"

# gRPC
tonic = "0.10"
prost = "0.12"

# Observability
tracing = "0.1"
tracing-subscriber = "0.3"
opentelemetry = "0.21"
prometheus = "0.13"

# LLM
reqwest = { version = "0.11", features = ["json"] }

# Time
chrono = "0.4"

# Other
uuid = { version = "1.6", features = ["v4"] }
dashmap = "5.5"  # Concurrent HashMap
```

---

## 💡 Key Design Decisions

### 1. Why Use IR (Intermediate Representation)?

- **Optimization space**: IR is easier to optimize (constant folding, dead code elimination, etc.)
- **Multiple backends**: Can generate different target code in the future (JIT, WASM, etc.)
- **Debug friendly**: IR is closer to execution logic than AST

### 2. Why Stack-based Virtual Machine?

- **Simple and efficient**: Easy to implement, good performance
- **Easy to debug**: Execution process is traceable
- **Suitable for expression evaluation**: Naturally supports nested expressions

### 3. Async Execution Model

```rust
// Key point: All I/O operations are async
pub async fn execute(&self, program: &Program, event: &Event) -> Result<DecisionResult>

// Support concurrent feature extraction
let (feature1, feature2, llm_result) = tokio::join!(
    self.extract_feature_1(),
    self.extract_feature_2(),
    self.call_llm(),
);
```

### 4. Caching Strategy

```
L1: In-Memory Cache (DashMap)
    - Compiled programs
    - Recent execution results

L2: Redis Cache
    - LLM responses
    - Feature calculation results
    - External API responses
```

### 5. Error Handling

Use `anyhow` for application errors and `thiserror` for library errors:

```rust
// Application code (SDK, Server)
use anyhow::Result;

pub async fn decide(&self, event: Event) -> Result<DecisionResult> {
    // ...
}

// Library code (Core, Compiler, Runtime)
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CompilerError {
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
}
```

### 6. Observability Integration

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(self, event))]
pub async fn execute(&self, program: &Program, event: &Event) -> Result<DecisionResult> {
    info!("Starting execution for rule: {}", program.metadata.rule_id);

    // Execution logic

    info!(
        score = result.score,
        action = ?result.action,
        "Execution completed"
    );

    Ok(result)
}
```

---

## 🧪 Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let yaml = r#"
rule:
  id: test_rule
  name: Test Rule
  when:
    event.type: login
    conditions:
      - user.age > 18
  score: 50
"#;

        let rule = RuleParser::parse(yaml).unwrap();
        assert_eq!(rule.id, "test_rule");
        assert_eq!(rule.score, 50);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_decision() {
    let mut engine = DecisionEngine::builder()
        .with_cache(Arc::new(InMemoryCache::new()))
        .with_llm_provider(Arc::new(MockLLMProvider::new()))
        .with_storage(Arc::new(MockStorage::new()))
        .build()
        .unwrap();

    engine.load_rule_yaml(TEST_RULE_YAML).unwrap();

    let event = create_test_event();
    let result = engine.decide(event).await.unwrap();

    assert_eq!(result.action, Action::Deny);
    assert!(result.score >= 100);
}
```

---

## 🚀 Getting Started

### 1. Initialize Workspace

```bash
# Create workspace
mkdir corint-decision
cd corint-decision

# Initialize Cargo workspace
cargo init --lib crates/corint-core
cargo init --lib crates/corint-parser
cargo init --lib crates/corint-compiler
cargo init --lib crates/corint-runtime
cargo init --lib crates/corint-sdk
cargo init --bin crates/corint-server

# Create workspace Cargo.toml
cat > Cargo.toml << 'EOF'
[workspace]
members = [
    "crates/corint-core",
    "crates/corint-parser",
    "crates/corint-compiler",
    "crates/corint-runtime",
    "crates/corint-sdk",
    "crates/corint-server",
]
EOF
```

### 2. Start with Core

```bash
cd crates/corint-core

# Add dependencies
cargo add serde --features derive
cargo add serde_json
cargo add serde_yaml
cargo add anyhow
cargo add thiserror

# Create AST modules
mkdir -p src/ast
touch src/ast/mod.rs
touch src/ast/rule.rs
touch src/ast/ruleset.rs
touch src/ast/pipeline.rs
touch src/ast/expression.rs
```

### 3. Implement Step by Step

Follow the phase plan and implement incrementally:

1. Start with Phase 1 - Core Foundation
2. Add comprehensive tests for each module
3. Move to Phase 2 - Runtime
4. Continue with advanced features
5. Optimize for production

---

## 📖 Additional Resources

### Documentation

- [RDL Overall Specification](./dsl/overall.md)
- [Architecture Design](./dsl/ARCHITECTURE.md)
- [Rule Specification](./dsl/rule.md)
- [Pipeline Specification](./dsl/pipeline.md)

### Learning Resources

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Async Book](https://rust-lang.github.io/async-book/)
- [Writing an Interpreter in Rust](https://github.com/tdimitrov/ruste-interpreter)

---

## 🤝 Contributing

Contributions are welcome! Please follow these guidelines:

1. Start by implementing core foundation
2. Write comprehensive tests
3. Follow Rust best practices
4. Document public APIs
5. Submit PRs with clear descriptions

---

**This guide provides a complete blueprint for implementing CORINT Decision Engine in Rust. Start with the core foundation and iterate through each phase systematically.**
