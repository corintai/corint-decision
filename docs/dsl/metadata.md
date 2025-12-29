# CORINT Risk Definition Language (RDL)
## Metadata Specification (v0.1)

Metadata provides versioning, authorship, and management information for all RDL components (Rules, Rulesets, and Pipelines).

---

## 1. Overview

All RDL components **should** include a `metadata` field containing:
- **Required fields**: `version`, `author`, `updated`
- **Custom fields**: Any additional key/value pairs for business-specific needs

---

## 2. Required Fields

### 2.1 `version` (string)

**Purpose**: Track the version of the component

**Format**: Semantic versioning (semver) recommended
- `MAJOR.MINOR.PATCH` (e.g., "1.0.0", "2.1.3")
- `MAJOR`: Breaking changes
- `MINOR`: New features (backward compatible)
- `PATCH`: Bug fixes (backward compatible)

**Examples**:
```yaml
version: "1.0.0"    # Initial release
version: "1.1.0"    # Added new features
version: "2.0.0"    # Breaking changes
```

### 2.2 `author` (string)

**Purpose**: Identify who created or maintains the component

**Format**: Free text
- Individual name: "John Doe"
- Team name: "Risk Engineering Team"
- Organization: "Security Operations"

**Examples**:
```yaml
author: "John Doe"
author: "Risk Engineering Team"
author: "Fraud Prevention Team"
```

### 2.3 `updated` (string)

**Purpose**: Track when the component was last modified

**Format**: `YYYY-MM-DD HH:mm:ss` (24-hour format)
- Year: 4 digits
- Month: 2 digits (01-12)
- Day: 2 digits (01-31)
- Hour: 2 digits (00-23)
- Minute: 2 digits (00-59)
- Second: 2 digits (00-59)

**Examples**:
```yaml
updated: "2025-12-20 14:30:00"
updated: "2025-01-15 09:00:00"
updated: "2025-06-30 23:59:59"
```

---

## 3. Custom Fields

Beyond the three required fields, you can add any custom key/value pairs to support your business needs.

### 3.1 Common Custom Fields

**Organizational**:
- `owner` (string) - Responsible team or individual
- `department` (string) - Department or division
- `contact` (string) - Contact email or Slack channel

**Classification**:
- `category` (string) - Functional category
- `domain` (string) - Business domain
- `severity` (string) - Risk severity (low, medium, high, critical)
- `priority` (number) - Priority ranking

**Management**:
- `tags` (array) - Tags for search and organization
- `environment` (string) - Target environment (dev, staging, production)
- `status` (string) - Development status (draft, active, deprecated)
- `deprecated` (boolean) - Deprecation flag
- `replacement` (string) - ID of replacement component

**References**:
- `documentation` (string) - Link to detailed documentation
- `ticket` (string) - Associated ticket or issue ID
- `parent` (string) - Parent component ID (for inheritance)

### 3.2 Custom Field Examples

```yaml
metadata:
  # Required
  version: "2.1.0"
  author: "Risk Engineering Team"
  updated: "2025-12-20 14:30:00"

  # Organizational
  owner: "fraud_team"
  department: "Risk Management"
  contact: "#fraud-alerts"

  # Classification
  category: "fraud_detection"
  domain: "payments"
  severity: "high"
  priority: 1

  # Management
  tags: [fraud, real-time, high-priority]
  environment: "production"
  status: "active"

  # References
  documentation: "https://wiki.example.com/fraud-rules"
  ticket: "RISK-1234"
```

---

## 4. Component-Specific Guidelines

### 4.1 Rule Metadata

**Recommended custom fields**:
- `category` - Rule classification (e.g., fraud, compliance, velocity)
- `severity` - Risk level (low, medium, high, critical)
- `tags` - Searchable tags

**Example**:
```yaml
rule:
  id: high_value_transaction
  name: High Value Transaction Detection
  when:
    all:
      - event.transaction.amount > 10000
  score: 75

  metadata:
    # Required
    version: "1.0.0"
    author: "John Doe"
    updated: "2025-12-20 10:30:00"

    # Custom
    category: "transaction_monitoring"
    severity: "high"
    tags: [high_value, aml, compliance]
```

### 4.2 Ruleset Metadata

**Recommended custom fields**:
- `domain` - Business domain (e.g., transactions, user_behavior)
- `owner` - Responsible team
- `parent` - Parent ruleset for inheritance

**Example**:
```yaml
ruleset:
  id: fraud_detection_core
  name: Core Fraud Detection
  rules:
    - rule1
    - rule2
  conclusion:
    - when: total_score >= 100
      signal: decline
      reason: "High risk score"

  metadata:
    # Required
    version: "2.0.0"
    author: "Fraud Team"
    updated: "2025-12-20 11:00:00"

    # Custom
    domain: "fraud"
    owner: "fraud_prevention_team"
    strictness: "high"
```

### 4.3 Pipeline Metadata

**Recommended custom fields**:
- `owner` - Responsible team
- `environment` - Deployment environment
- `tags` - Searchable tags

**Example**:
```yaml
pipeline:
  id: fraud_detection_pipeline
  name: Fraud Detection Pipeline
  entry: blacklist_check
  when: event.type == "transaction"
  steps:
    - step: ...

  metadata:
    # Required
    version: "1.5.0"
    author: "Risk Engineering Team"
    updated: "2025-12-20 15:45:00"

    # Custom
    owner: "risk_team"
    environment: "production"
    tags: [fraud, payments, real-time]
    sla_ms: 100
```

---

## 5. Best Practices

### 5.1 Versioning Strategy

**Semantic Versioning**:
- Start with `1.0.0` for production-ready components
- Use `0.x.y` for pre-release/experimental versions
- Increment:
  - **MAJOR**: Breaking changes (logic changes, field removals)
  - **MINOR**: New features (new rules, enhanced logic)
  - **PATCH**: Bug fixes, typo corrections

**Example progression**:
```yaml
0.1.0  # Initial draft
0.2.0  # Added new conditions
1.0.0  # Production release
1.1.0  # Added new rule to ruleset
1.1.1  # Fixed threshold typo
2.0.0  # Changed decision logic (breaking)
```

### 5.2 Update Timestamp

**When to update**:
- ✅ Logic changes (conditions, scores, actions)
- ✅ Configuration changes (thresholds, parameters)
- ✅ Metadata updates (except `updated` itself)
- ❌ Comments or documentation-only changes

**Consistency**:
- Use UTC timezone or consistent local timezone
- Automated tools should update `updated` field automatically
- Manual updates should be atomic with code changes

### 5.3 Author Field

**Team vs Individual**:
- Use team name for shared ownership: `"Risk Team"`
- Use individual name for personal rules: `"Jane Smith"`
- Consider using email format: `"risk-team@company.com"`

**Transitions**:
- When ownership changes, update `author` to new owner
- Consider adding `original_author` custom field if needed

### 5.4 Custom Field Naming

**Conventions**:
- Use `snake_case` for field names
- Use descriptive names (avoid abbreviations)
- Be consistent across all components
- Document custom fields in team wiki

**Good examples**:
```yaml
environment: "production"
transaction_limit: 10000
max_retry_count: 3
```

**Bad examples**:
```yaml
env: "prod"        # Too abbreviated
txnLmt: 10000      # Mixed case, unclear
max_retry: 3       # Inconsistent with count suffix
```

---

## 6. Validation

### 6.1 Required Field Validation

The compiler/runtime **may** validate that required fields are present:

```yaml
# ✅ Valid - all required fields present
metadata:
  version: "1.0.0"
  author: "Risk Team"
  updated: "2025-12-20 10:00:00"

# ❌ Invalid - missing required field
metadata:
  version: "1.0.0"
  author: "Risk Team"
  # Missing 'updated'
```

### 6.2 Format Validation

**Version format**:
- Semver pattern: `^\d+\.\d+\.\d+$`
- Examples: "1.0.0", "2.3.1", "10.20.30"

**Updated format**:
- Datetime pattern: `^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$`
- Examples: "2025-12-20 14:30:00"

### 6.3 Linting Rules

**Recommended linting checks**:
- ✅ Required fields present
- ✅ Version follows semver format
- ✅ Updated follows datetime format
- ⚠️  Updated is recent (warn if > 6 months old)
- ⚠️  Author is not empty/placeholder
- ⚠️  Version matches file history (if in git)

---

## 7. Examples

### 7.1 Minimal Example

```yaml
rule:
  id: simple_check
  name: Simple Check
  when:
    event.amount > 100
  score: 50

  metadata:
    version: "1.0.0"
    author: "Risk Team"
    updated: "2025-12-20 10:00:00"
```

### 7.2 Rich Example

```yaml
pipeline:
  id: comprehensive_fraud_detection
  name: Comprehensive Fraud Detection Pipeline
  entry: device_check
  when: event.type == "transaction"
  steps:
    - step: ...

  metadata:
    # === Required Fields ===
    version: "3.2.1"
    author: "Fraud Prevention Team"
    updated: "2025-12-20 16:30:45"

    # === Organizational ===
    owner: "fraud_team"
    department: "Risk Management"
    contact: "#fraud-alerts"

    # === Classification ===
    category: "fraud_detection"
    domain: "payments"
    severity: "critical"
    priority: 1

    # === Management ===
    tags: [fraud, real-time, production]
    environment: "production"
    status: "active"

    # === Performance ===
    sla_ms: 100
    max_throughput: 10000

    # === References ===
    documentation: "https://wiki.company.com/fraud-pipeline"
    ticket: "RISK-5678"
    changelog: "Added velocity checks"

    # === Custom Business Logic ===
    min_transaction_amount: 0.01
    max_transaction_amount: 1000000
    supported_currencies: [USD, EUR, GBP]
```

---

## 8. Related Documentation

- [Pipeline Specification](./pipeline.md) - Pipeline structure and metadata
- [Rule Specification](./rule.md) - Rule structure and metadata
- [Ruleset Specification](./ruleset.md) - Ruleset structure and metadata
- [Overall Specification](./overall.md) - RDL overview

---

## 9. Summary

**Metadata in CORINT RDL**:
- ✅ Three required fields: `version`, `author`, `updated`
- ✅ Flexible custom fields for business needs
- ✅ Consistent format across all component types
- ✅ Supports versioning, ownership, and lifecycle management
- ✅ Extensible for organization-specific requirements

**Key Points**:
- Use semver for versioning (`MAJOR.MINOR.PATCH`)
- Use `YYYY-MM-DD HH:mm:ss` for timestamps
- Add custom fields as needed
- Maintain consistency across your organization
