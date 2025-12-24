# Corint Decision Repository

This is the production rule and policy repository for Corint Decision Engine.

## 📁 Directory Structure

```
repository/
├── library/                # Reusable components library
│   ├── rules/              # Individual rule definitions
│   │   ├── fraud/          # Fraud detection rules
│   │   ├── geography/      # Geography-based rules
│   │   └── payment/        # Payment risk rules
│   ├── rulesets/           # Reusable ruleset definitions
│   └── templates/          # Decision logic templates (future)
│
├── pipelines/              # Business scenario orchestration
│   ├── fraud_detection.yaml
│   └── payment_pipeline.yaml
│
└── configs/                # Runtime configurations
    ├── apis/               # External API configs
    ├── datasources/        # Data source configs
    └── features/           # Feature definitions
```

## 🎯 Design Philosophy

### Three-Layer Architecture

1. **Library Layer** (`library/`): Reusable rules and rulesets
   - Rules are atomic detection units
   - Rulesets combine rules with decision logic
   - All dependencies are explicitly declared via `imports`

2. **Pipeline Layer** (`pipelines/`): Business scenario orchestration
   - Pipelines import and use rulesets
   - Define event routing and step execution
   - Focus on business logic, not repetition

3. **Config Layer** (`configs/`): Runtime configurations
   - Data sources, features, and external APIs
   - Separate from business logic for flexibility

### Key Principles

- **Explicit Dependencies**: Every file declares its dependencies via `imports`
- **No Duplication**: Rules and rulesets defined once, reused everywhere
- **Compile-Time Merging**: Dependencies resolved during compilation
- **ID Uniqueness**: All Rule IDs and Ruleset IDs are globally unique

## 📝 Usage Examples

### Using Existing Pipelines

```bash
# Run fraud detection
cargo run --example fraud_detection

# Run payment pipeline
cargo run --example payment_pipeline

# Run Supabase feature-based risk assessment (requires feature support)
cargo run --example supabase_feature_example --features sqlx
```

### Using Supabase Features

The repository includes a Supabase-based risk assessment pipeline that demonstrates real-time feature calculation from PostgreSQL:

**Files:**
- `configs/datasources/supabase_events.yaml` - Supabase PostgreSQL connection config
- `pipelines/supabase_feature_ruleset.yaml` - Risk assessment pipeline with feature-based rules

**Feature Calculation:**
Rules reference features using the `features.` prefix (e.g., `features.transaction_sum_7d > 5000`), and the engine automatically calculates them from Supabase during rule execution.

**Prerequisites:**
1. Set up Supabase database with events table
2. Configure connection string in `configs/datasources/supabase_events.yaml`
3. Load feature definitions from `configs/features/user_features.yaml`

**Example Rules:**
- `high_transaction_volume` - Detects `transaction_sum_7d > 5000 AND transaction_count_24h > 10`
- `high_value_transaction` - Detects `max_transaction_7d > 400`
- `rapid_transactions` - Detects `transaction_count_24h > 20`
- `suspicious_device_pattern` - Detects `unique_devices_7d >= 2 AND transaction_sum_7d > 3000`

### Creating a Custom Pipeline

```yaml
# my_custom_pipeline.yaml
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

### Creating Custom Rules

```yaml
# library/rules/custom/my_rule.yaml
version: "0.1"

rule:
  id: my_custom_rule_pattern
  name: My Custom Rule
  description: Detect specific custom pattern

  when:
    conditions:
      - custom_field > 100

  score: 50

  metadata:
    category: custom
    severity: medium
    rule_version: "1.0.0"
```

### Creating Custom Rulesets

```yaml
# library/rulesets/my_custom_ruleset.yaml
version: "0.1"

# Explicitly import rule dependencies
imports:
  rules:
    - library/rules/fraud/fraud_farm.yaml
    - library/rules/custom/my_rule.yaml

---

ruleset:
  id: my_custom_ruleset
  name: My Custom Ruleset

  rules:
    - fraud_farm_pattern
    - my_custom_rule_pattern

  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: "High risk"
    - default: true
      signal: approve
      reason: "Low risk"
```

## 📋 Available Rules

### Fraud Detection Rules (`library/rules/fraud/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `fraud_farm_pattern` | 100 | Organized fraud farms detection |
| `account_takeover_pattern` | 85 | Account takeover detection |
| `velocity_abuse_pattern` | 70 | Transaction frequency abuse |
| `amount_outlier_pattern` | 75 | Statistical amount anomaly |
| `new_user_fraud_pattern` | 50 | New account suspicious behavior |

### Payment Risk Rules (`library/rules/payment/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `card_testing` | 80 | Card testing pattern detection |
| `velocity_check` | 50 | High transaction frequency |
| `new_account_risk` | 60 | New account high-value purchase |
| `suspicious_email` | 35 | Disposable email detection |

### Geography Rules (`library/rules/geography/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `suspicious_geography_pattern` | 60 | Impossible travel detection |
| `suspicious_ip` | 40 | Non-trusted country IP |

## 📦 Available Rulesets

### `fraud_detection_core`
Complete fraud detection with 6 patterns:
- fraud_farm_pattern
- account_takeover_pattern
- velocity_abuse_pattern
- amount_outlier_pattern
- suspicious_geography_pattern
- new_user_fraud_pattern

**Decision Thresholds**:
- Score >= 200: Deny (critical risk)
- Score >= 150: Deny (very high risk)
- Score >= 100: Deny (high risk)
- Score >= 60: Review (medium-high risk)
- Score >= 30: Review (medium risk)
- triggered_count >= 3: Review

### `payment_standard`
Standard payment risk assessment (<= $1000):
- Uses all 5 payment/geography rules
- Balanced thresholds
- Score >= 100: Deny
- Score >= 60: Challenge (3DS)
- Score >= 40: Review

### `payment_high_value`
High-value payment risk (> $1000):
- Stricter thresholds than standard
- Score >= 60: Deny
- triggered_count >= 2: Review
- triggered_count >= 1: Challenge (3DS)

## 🔧 ID Naming Conventions

### Rule IDs
Format: `<category>_<specific_pattern>`

Examples:
- `fraud_farm_pattern`
- `payment_card_testing`
- `geo_impossible_travel_pattern`

### Ruleset IDs
Format: `<domain>_<purpose>_<variant?>`

Examples:
- `fraud_detection_core`
- `payment_standard`
- `payment_high_value`

## 🚀 Next Steps

See [RULE_REFACTOR.md](../RULE_REFACTOR.md) for:
- Complete architecture documentation
- Implementation guidelines
- Advanced features (inheritance, parameters, templates)
- Repository Pattern for database support

## 📚 Resources

- [Examples Directory](../docs/dsl/examples/): Tutorial and learning resources
- [Documentation](../docs/): API and usage guides
- [Tests](../tests/): Integration tests and fixtures
