# Corint Decision Repository

This is the production rule and policy repository for Corint Decision Engine.

## Directory Structure

```
repository/
├── registry.yaml           # Event-to-pipeline routing (entry point)
│
├── library/                 # Reusable components library
│   ├── rules/               # Individual rule definitions
│   │   ├── account/         # Account security rules
│   │   ├── device/          # Device fingerprinting rules
│   │   ├── fraud/           # Fraud detection rules
│   │   ├── geography/       # Geography-based rules
│   │   └── payment/         # Payment risk rules
│   └── rulesets/            # Reusable ruleset definitions
│
├── pipelines/               # Business scenario orchestration
│   ├── fraud_detection.yaml
│   ├── payment_pipeline.yaml
│   ├── login_risk_pipeline.yaml
│   └── supabase_feature_ruleset.yaml
│
├── configs/                 # Runtime configurations
│   ├── apis/                # External API configs
│   ├── features/            # Feature definitions
│   ├── lists/               # Custom lists (blocklists, allowlists)
│   └── services/            # Internal service configs (microservices, message queues)
│
│   Note: Datasources are now defined in config/server.yaml (not in repository/configs/datasources/)
│
└── test_data/               # Test data and scripts
```

## Design Philosophy

### Three-Layer Decision Architecture

RDL uses a three-layer decision architecture with clear separation of concerns:

```
┌─────────────────────────────────────┐
│ Layer 1: Rules (Pattern Detectors)  │
│ - Detect individual risk factors    │
│ - Produce risk scores (+/-)         │
│ - No decision-making                │
└─────────────────────────────────────┘
              ↓ scores
┌─────────────────────────────────────┐
│ Layer 2: Rulesets (Signal Generators)│
│ - Combine and evaluate rule results │
│ - Aggregate scores (total_score)    │
│ - Produce signals via conclusion    │
│   (approve/decline/review/hold/pass)│
└─────────────────────────────────────┘
              ↓ signals
┌─────────────────────────────────────┐
│ Layer 3: Pipeline (Final Decision)  │
│ - Orchestrate execution flow        │
│ - Make final decisions via decision │
│ - Map signals to results + actions  │
└─────────────────────────────────────┘
```

**Decision Flow:**
```
Rules (detect) → Scores → Ruleset (conclude) → Signals → Pipeline (decide) → Results + Actions
```

### Repository Structure

1. **Library Layer** (`library/`): Reusable rules and rulesets
   - Rules are atomic detection units
   - Rulesets combine rules with decision logic
   - All dependencies are explicitly declared via `import`

2. **Pipeline Layer** (`pipelines/`): Business scenario orchestration
   - Pipelines import and use rulesets
   - Define event routing and step execution
   - Make final decisions via `decision` block

3. **Config Layer** (`configs/`): Runtime configurations
   - Data sources, features, lists, and external APIs
   - Separate from business logic for flexibility

4. **Registry** (`registry.yaml`): Event-to-pipeline routing
   - Top-to-bottom matching (first match wins)
   - Priority-based routing for event types

### Key Principles

- **Explicit Dependencies**: Every file declares its dependencies via `import`
- **No Duplication**: Rules and rulesets defined once, reused everywhere
- **Compile-Time Merging**: Dependencies resolved during compilation
- **ID Uniqueness**: All Rule IDs and Ruleset IDs are globally unique
- **Rulesets Suggest, Pipelines Decide**: Clear separation of concerns

## Usage Examples

### Using Existing Pipelines

```bash
# Run fraud detection
cargo run --example fraud_detection

# Run payment pipeline
cargo run --example payment_pipeline

# Run feature-based risk assessment (requires database)
cargo run --example supabase_feature_example
```

### Creating a Custom Pipeline

```yaml
# my_custom_pipeline.yaml
version: "0.1"

import:
  rulesets:
    - library/rulesets/fraud_detection_core.yaml

---

pipeline:
  id: my_custom_pipeline
  name: My Custom Risk Pipeline
  entry: fraud_check

  when:
    all:
      - event.type == "transaction"

  steps:
    - step:
        id: fraud_check
        type: ruleset
        ruleset: fraud_detection_core

  decision:
    - when: results.fraud_detection_core.signal == "decline"
      result: decline
      reason: "${results.fraud_detection_core.reason}"
    - default: true
      result: approve
      reason: "Low risk"
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
    all:
      - custom_field > 100

  score: 50

  metadata:
    category: custom
    severity: medium
    version: "1.0.0"
```

### Creating Custom Rulesets

```yaml
# library/rulesets/my_custom_ruleset.yaml
version: "0.1"

# Explicitly import rule dependencies
import:
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

### Using Features in Rules

```yaml
# Feature-based rule example
rule:
  id: high_velocity_detection
  name: High Velocity Detection

  when:
    all:
      - features.transaction_count_24h > 20
      - features.transaction_sum_7d > 5000
      - features.unique_devices_7d >= 3

  score: 80
```

Features are defined in `configs/features/` and calculated on-demand from datasources during rule execution.

## Available Rules

### Fraud Detection Rules (`library/rules/fraud/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `fraud_farm_pattern` | 100 | Organized fraud farms detection |
| `account_takeover_pattern` | 85 | Account takeover detection |
| `velocity_abuse_pattern` | 70 | Transaction frequency abuse |
| `amount_outlier_pattern` | 75 | Statistical amount anomaly |
| `new_user_fraud_pattern` | 50 | New account suspicious behavior |
| `velocity_pattern` | 60 | General velocity pattern |

### Payment Risk Rules (`library/rules/payment/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `card_testing` | 80 | Card testing pattern detection |
| `velocity_check` | 50 | High transaction frequency |
| `new_account_risk` | 60 | New account high-value purchase |
| `suspicious_email` | 35 | Disposable email detection |

### Account Security Rules (`library/rules/account/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `impossible_travel_pattern` | 70 | Impossible travel detection |
| `off_hours_activity` | 40 | Unusual time-based patterns |
| `password_change_risk` | 55 | Risky password change behavior |

### Device Rules (`library/rules/device/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `device_emulator` | 90 | Emulator/simulator detection |
| `device_spoofing` | 85 | Device fingerprint spoofing |

### Geography Rules (`library/rules/geography/`)

| Rule ID | Score | Description |
|---------|-------|-------------|
| `suspicious_geography_pattern` | 60 | Geographic anomaly detection |
| `suspicious_ip` | 40 | Non-trusted country IP |

## Available Rulesets

### `fraud_detection_core`
Complete fraud detection with 6 patterns:
- fraud_farm_pattern
- account_takeover_pattern
- velocity_abuse_pattern
- amount_outlier_pattern
- suspicious_geography_pattern
- new_user_fraud_pattern

**Decision Thresholds**:
- Score >= 200: Decline (critical risk)
- Score >= 150: Decline (very high risk)
- Score >= 100: Decline (high risk)
- Score >= 60: Review (medium-high risk)
- Score >= 30: Review (medium risk)
- triggered_count >= 3: Review

### `payment_standard`
Standard payment risk assessment (<= $1000):
- Uses payment and geography rules
- Score >= 100: Decline
- Score >= 60: Challenge (3DS)
- Score >= 40: Review

### `payment_high_value`
High-value payment risk (> $1000):
- Stricter thresholds than standard
- Score >= 60: Decline
- triggered_count >= 2: Review
- triggered_count >= 1: Challenge (3DS)

### `login_risk`
Login security assessment:
- Account takeover detection
- Device fingerprinting checks
- Geographic anomaly detection

## Registry Configuration

The `registry.yaml` file defines event-to-pipeline routing:

```yaml
registry:
  # Supabase transactions - specific pipeline
  - pipeline: supabase_transaction_pipeline
    when:
      all:
        - event.type == "transaction"
        - event.source == "supabase"

  # General transactions - fraud detection
  - pipeline: fraud_detection_pipeline
    when: event.type == "transaction"

  # Payment events
  - pipeline: payment_pipeline
    when: event.type == "payment"

  # Login events
  - pipeline: login_risk_pipeline
    when: event.type == "login"
```

**Matching Behavior:**
- Top-to-bottom evaluation
- First match wins
- More specific conditions should be placed first

## Naming Conventions

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

## Configuration Files

### Datasources (defined in `config/server.yaml`)
All datasources are now defined in `config/server.yaml` under the `datasource` section, including:
- `sqlite_events` - SQLite for local development and testing
- `postgres_events` - PostgreSQL for event storage (production-ready)
- `clickhouse_events` - ClickHouse for high-performance OLAP queries
- `redis_features` - Redis for feature lookups and caching
- `supabase_events` - Supabase PostgreSQL (with Session Pooler)

**Note:** The `repository/configs/datasources/` directory is deprecated. All datasource configurations should be defined in `config/server.yaml` to avoid duplication and confusion.

### Features (`configs/features/`)
- `user_features.yaml` - User behavior aggregations
- `device_features.yaml` - Device fingerprinting features
- `ip_features.yaml` - IP reputation and geolocation
- `statistical_features.yaml` - Statistical analysis features

### Lists (`configs/lists/`)
- `example.yaml` - Example blocklist/allowlist configuration

### Services (`configs/services/`)
- `kyc_service.yaml` - KYC verification service
- `risk_scoring_service.yaml` - Risk scoring gRPC service
- `event_bus.yaml` - Message queue configuration

## Resources

- [DSL Documentation](../docs/dsl/) - Complete DSL specification
- [Feature Engineering](../docs/FEATURE_ENGINEERING.md) - Feature definition guide
- [Examples](../docs/dsl/examples/) - Tutorial and learning resources
