# corint-compiler

**AST to IR Compiler for CORINT Decision Engine**

## Overview

`corint-compiler` compiles Abstract Syntax Tree (AST) representations of decision rules into executable Intermediate Representation (IR). It handles semantic analysis, type checking, and optimization before generating runtime-executable code.

## Key Responsibilities

### 1. Semantic Analysis
- Validates rule and ruleset definitions
- Type checking for expressions and conditions
- Reference resolution (rule IDs, feature names, etc.)
- Dependency analysis

### 2. Import Resolution
- Resolves `import` statements from repositories
- Handles ruleset inheritance (`extends`)
- Compiles parameterized rules with compile-time parameter inlining

### 3. Code Generation
- Transforms AST into executable IR instructions
- Optimizes condition evaluation order
- Generates efficient decision trees
- Inlines constants and parameters

## Architecture

```
┌─────────────────────────────────────┐
│   AST (from corint-parser)         │
│   - Rules, Rulesets, Pipelines     │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   Semantic Analyzer                │
│   - Validate structure             │
│   - Resolve references             │
│   - Type checking                  │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   Import Resolver (Phase 3)        │
│   - Load referenced artifacts      │
│   - Resolve inheritance            │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   Code Generator                   │
│   - Generate IR instructions       │
│   - Optimize execution order       │
│   - Inline parameters              │
└─────────────┬───────────────────────┘
              │
              ▼
┌─────────────────────────────────────┐
│   IR (for corint-runtime)          │
│   - Executable instructions        │
│   - Optimized decision logic       │
└─────────────────────────────────────┘
```

## Core Components

### CompilerContext
Manages compilation state and configuration:
- Symbol table for rules and features
- Type information
- Error accumulation
- Repository access for import resolution

### SemanticAnalyzer
Validates AST before code generation:
```rust
pub struct SemanticAnalyzer;

impl SemanticAnalyzer {
    pub fn analyze_rule(rule: &Rule) -> Result<()>;
    pub fn analyze_ruleset(ruleset: &Ruleset) -> Result<()>;
    pub fn analyze_pipeline(pipeline: &Pipeline) -> Result<()>;
}
```

### ImportResolver (Phase 3)
Resolves external dependencies:
```rust
pub struct ImportResolver<R: Repository> {
    repository: R,
    cache: HashMap<String, ResolvedArtifact>,
}

impl<R: Repository> ImportResolver<R> {
    /// Resolve ruleset inheritance chain
    pub async fn resolve_ruleset(&mut self, id: &str) -> Result<Ruleset>;

    /// Inline rule parameters at compile time
    pub fn inline_parameters(rule: &Rule) -> Result<Rule>;
}
```

### RuleCodegen / RulesetCodegen
Generates IR from validated AST:
```rust
pub struct RuleCodegen;

impl RuleCodegen {
    /// Compile a rule into IR instructions
    pub fn compile(rule: &Rule) -> Result<CompiledRule>;
}

pub struct RulesetCodegen;

impl RulesetCodegen {
    /// Compile a ruleset with all its rules
    pub fn compile(ruleset: &Ruleset) -> Result<CompiledRuleset>;
}
```

## Usage

### Basic Compilation

```rust
use corint_compiler::Compiler;
use corint_parser::RuleParser;

// Parse YAML rule
let yaml = r#"
rule:
  id: high_risk
  when:
    conditions:
      - amount > 1000
  score: 80
"#;

let rule = RuleParser::parse(yaml)?;

// Compile to IR
let compiler = Compiler::new();
let compiled = compiler.compile_rule(&rule)?;
```

### Compilation with Repository (Phase 3)

```rust
use corint_compiler::Compiler;
use corint_repository::FileSystemRepository;

// Create compiler with repository access
let repo = FileSystemRepository::new("repository")?;
let mut compiler = Compiler::with_repository(repo);

// Compile ruleset with inheritance
let ruleset = compiler.compile_ruleset_by_id("payment_fraud").await?;
// Automatically resolves: payment_fraud -> fraud_detection_base -> ...
```

### Parameter Inlining

```rust
// Before compilation (parameterized rule)
rule:
  id: velocity_check
  params:
    time_window: 3600
    max_count: 10
  when:
    conditions:
      - transaction_count(last_n_seconds: params.time_window) > params.max_count

// After compilation (parameters inlined)
rule:
  id: velocity_check
  when:
    conditions:
      - transaction_count(last_n_seconds: 3600) > 10
```

## Error Handling

The compiler provides detailed error messages:

```rust
pub enum CompilerError {
    /// Undefined reference (rule, feature)
    UndefinedReference { id: String, kind: String },

    /// Type mismatch in expression
    TypeMismatch { expected: Type, found: Type },

    /// Circular dependency detected
    CircularDependency { path: Vec<String> },

    /// Invalid parameter configuration
    InvalidParameter { name: String, reason: String },

    /// Import resolution failed
    ImportError { source: RepositoryError },
}
```

## Optimization

The compiler applies several optimizations:

1. **Constant Folding**: Evaluates constant expressions at compile time
2. **Dead Code Elimination**: Removes unreachable conditions
3. **Short-circuit Optimization**: Orders conditions for early termination
4. **Parameter Inlining**: Replaces parameter references with actual values

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_simple_rule() {
        let rule = Rule {
            id: "test".to_string(),
            when: WhenBlock {
                conditions: vec![/* ... */],
                ..Default::default()
            },
            score: 50,
            ..Default::default()
        };

        let compiled = Compiler::new().compile_rule(&rule).unwrap();
        assert!(compiled.instructions.len() > 0);
    }

    #[tokio::test]
    async fn test_ruleset_inheritance() {
        let repo = FileSystemRepository::new("test_data")?;
        let mut compiler = Compiler::with_repository(repo);

        let compiled = compiler.compile_ruleset_by_id("child_ruleset").await?;
        // Should include rules from parent ruleset
    }
}
```

## Dependencies

- `corint-core`: AST and IR type definitions
- `corint-parser`: YAML to AST parsing
- `corint-repository` (optional): For import resolution

## Features

- Default: Basic compilation without repository support
- `repository`: Enable import resolution and repository integration

## Related Documentation

- [Ruleset Inheritance](../../docs/dsl/ruleset.md#inheritance)
- [Parameterized Rules](../../docs/dsl/rule.md#parameters)
- [Import System](../../docs/dsl/imports.md)
