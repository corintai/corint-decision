# CORINT Risk Definition Language (RDL)
## List (Custom Lists) Specification (v0.1)

**Custom lists** (blocklists, allowlists, watchlists) enable efficient membership checks against predefined sets of values during rule evaluation.

---

## 1. Overview

### 1.1 What are Lists?

Lists are named collections of values that can be checked in rule conditions. Common use cases:

- **Blocklists** - Block known fraudulent entities (emails, IPs, devices)
- **Allowlists** - Bypass checks for trusted entities (VIP users, partners)
- **Watchlists** - Flag for manual review (sanctions, PEP lists)
- **Greylists** - Temporary restrictions (rate-limited IPs)

### 1.2 List Types by Purpose

| List Type | Purpose | Example |
|-----------|---------|---------|
| Blocklist | Deny access | `email_blocklist`, `ip_blocklist` |
| Allowlist | Bypass checks | `trusted_users`, `vip_customers` |
| Watchlist | Flag for review | `ofac_sanctions`, `pep_list` |
| Greylist | Temporary restriction | `rate_limited_ips` |

---

## 2. DSL Syntax

### 2.1 Basic Syntax

Lists are referenced using `list.<list_id>` with membership operators:

```yaml
<field> in list.<list_id>
<field> not in list.<list_id>
```

**Syntax Rules**:
- **Format**: `list.<list_id>` where `<list_id>` is defined in configuration
- **Operators**: `in` (membership), `not in` (non-membership)
- **Field**: Any valid field path (e.g., `user.email`, `event.ip`)

### 2.2 Simple Examples

**Blocklist Check**:
```yaml
rule:
  id: blocked_email
  name: Email Blocklist Check
  when:
    all:
      - user.email in list.email_blocklist
  score: 500
```

### 2.3 Negative Checks (not in)

```yaml
rule:
  id: untrusted_large_transaction
  name: Untrusted User High Value Transaction
  when:
    all:
      - user.id not in list.trusted_users
      - transaction.amount > 10000
  score: 100
```

### 2.4 Multiple List Checks

**OR Logic** (any match):
```yaml
rule:
  id: sanctions_check
  name: Sanctions and High Risk Check
  when:
    any:
      - user.name in list.ofac_sanctions
      - user.country in list.high_risk_countries
  score: 200
```

**AND Logic** (all must match):
```yaml
rule:
  id: suspicious_new_user
  name: Suspicious New User Pattern
  when:
    all:
      - user.ip in list.suspicious_ips
      - user.email not in list.verified_emails
      - user.account_age_days < 30
  score: 150
```

---

## 3. List Configuration

### 3.1 Configuration File Structure

Lists are defined in YAML files under `repository/configs/lists/`:

```
repository/
└── configs/
    └── lists/
        ├── blocklists.yaml
        ├── allowlists.yaml
        ├── watchlists.yaml
        └── custom.yaml
```

### 3.2 List Definition Format

**Single List File**:
```yaml
# repository/configs/lists/email_blocklist.yaml
id: email_blocklist
description: "Blocked email addresses"
backend: postgresql
```

**Multiple Lists in One File**:
```yaml
# repository/configs/lists/fraud_lists.yaml
lists:
  - id: email_blocklist
    description: "Blocked email addresses"
    backend: postgresql

  - id: ip_blocklist
    description: "Blocked IP addresses"
    backend: redis
    redis_key: "lists:ip_blocklist"

  - id: device_blocklist
    description: "Known fraud devices"
    backend: postgresql
```

### 3.3 Backend Types

Lists support multiple storage backends:

| Backend | Use Case | Example |
|---------|----------|---------|
| `postgresql` | Persistent storage with metadata | User blocklists, device fingerprints |
| `redis` | High-performance lookups | IP blocklists, rate limiting |
| `file` | Static read-only lists | Country codes, disposable domains |
| `memory` | Small frequently-used lists | Test data, config values |
| `api` | External list providers | Third-party sanctions lists |

**PostgreSQL Backend**:
```yaml
id: email_blocklist
backend: postgresql
# Uses default list_entries table
```

**Redis Backend**:
```yaml
id: ip_blocklist
backend: redis
redis_key: "lists:ip_blocklist"
cache_ttl: 60
```

**File Backend**:
```yaml
id: high_risk_countries
backend: file
path: "repository/configs/lists/data/high_risk_countries.txt"
reload_interval: 3600
```

**Memory Backend**:
```yaml
id: test_emails
backend: memory
initial_values:
  - "test@example.com"
  - "fraud@test.com"
```

**API Backend**:
```yaml
id: ofac_sanctions
backend: api
url: "https://api.sanctions.io/check"
method: POST
cache_ttl: 86400
timeout_ms: 5000
```

---

## 4. Common Use Cases

### 4.1 IP-Based Blocking

```yaml
rule:
  id: blocked_ip
  name: IP Blocklist
  when:
    all:
      - event.ip in list.ip_blocklist
  score: 500
```

### 4.2 Device Fingerprint Check

```yaml
rule:
  id: fraud_device
  name: Known Fraud Device
  when:
    all:
      - device.fingerprint in list.fraud_devices
  score: 300
```

### 4.3 Email Domain Check

```yaml
rule:
  id: disposable_email
  name: Disposable Email Domain
  when:
    all:
      - user.email_domain in list.disposable_domains
  score: 50
```

### 4.4 VIP User Bypass

```yaml
rule:
  id: vip_bypass
  name: VIP User Transaction Bypass
  when:
    all:
      - user.id in list.vip_users
  score: -200  # Negative score to reduce total risk score
```

### 4.5 Country-Based Risk

```yaml
rule:
  id: high_risk_country
  name: High Risk Country Check
  when:
    all:
      - geo.country in list.high_risk_countries
      - user.id not in list.verified_international_users
  score: 100
```

### 4.6 Payment Card BIN Check

```yaml
rule:
  id: high_risk_bin
  name: High Risk Card BIN
  when:
    all:
      - payment.card_bin in list.high_risk_bins
  score: 75
```

---

## 5. Integration with Rules and Pipelines

### 5.1 Rule Example

```yaml
rule:
  id: comprehensive_blocklist_check
  name: Comprehensive Blocklist Check
  description: Check multiple blocklists for fraud indicators

  when:
    any:
      - user.email in list.email_blocklist
      - event.ip in list.ip_blocklist
      - device.fingerprint in list.device_blocklist
      - user.phone in list.phone_blocklist

  score: 500

  metadata:
    version: "1.0.0"
    author: "Fraud Prevention Team"
    updated: "2025-12-20 10:00:00"
    category: blocklist
    severity: critical
```

### 5.2 Ruleset Example

```yaml
ruleset:
  id: list_based_checks
  name: List-Based Risk Checks
  description: Risk assessment using various lists

  rules:
    - blocked_email_check
    - blocked_ip_check
    - trusted_user_bypass
    - high_risk_country_check

  conclusion:
    - when: total_score >= 500
      signal: decline
      reason: "Found in critical blocklist"

    - when: total_score < 0
      signal: approve
      reason: "Trusted user bypass"

    - default: true
      signal: review

  metadata:
    version: "1.0.0"
    author: "Risk Engineering Team"
    updated: "2025-12-20 10:00:00"
```

### 5.3 Pipeline Example

```yaml
pipeline:
  id: fraud_detection_with_lists
  name: Fraud Detection with List Checks
  description: Multi-stage fraud detection using lists

  entry: blocklist_check

  steps:
    - step:
        id: blocklist_check
        name: Initial Blocklist Check
        type: router
        routes:
          # Immediate denial for blocklisted entities
          - next: deny_step
            when:
              any:
                - user.email in list.email_blocklist
                - event.ip in list.ip_blocklist

          # Bypass for trusted users
          - next: approve_step
            when:
              all:
                - user.id in list.vip_users

          # Continue to detailed checks
          - next: fraud_rules
        default: fraud_rules

    - step:
        id: fraud_rules
        name: Fraud Detection Rules
        type: ruleset
        ruleset: fraud_detection_core
        next: decision

    - step:
        id: decision
        name: Final Decision
        type: router
        routes:
          - next: deny_step
            when:
              all:
                - results.fraud_detection_core.total_score >= 200
          - next: approve_step
        default: approve_step

  metadata:
    version: "1.0.0"
    author: "Fraud Prevention Team"
    updated: "2025-12-20 10:00:00"
```

---

## 6. Best Practices

### 6.1 Naming Conventions

**List ID Naming**:
- Use descriptive, lowercase names with underscores
- Include purpose suffix: `_blocklist`, `_allowlist`, `_watchlist`
- Examples: `email_blocklist`, `vip_users`, `ofac_sanctions`

**Good**:
```yaml
- email_blocklist
- ip_blocklist
- trusted_users
- high_risk_countries
```

**Bad**:
```yaml
- list1
- BlockedEmails
- temp_list
```

### 6.2 Performance Optimization

**Choose Appropriate Backend**:
```yaml
# High-frequency lookups → Redis
- id: ip_blocklist
  backend: redis

# Metadata-rich lists → PostgreSQL
- id: fraud_investigation_cases
  backend: postgresql

# Static reference data → File
- id: country_codes
  backend: file
```

**Cache Strategy**:
- Use Redis for hot data (frequent lookups)
- Use file backend for static data
- Set appropriate `cache_ttl` for API backends

### 6.3 List Organization

**Organize by Domain**:
```
repository/configs/lists/
├── fraud/
│   ├── email_blocklist.yaml
│   ├── ip_blocklist.yaml
│   └── device_blocklist.yaml
├── compliance/
│   ├── ofac_sanctions.yaml
│   └── pep_list.yaml
└── business/
    ├── vip_users.yaml
    └── partner_domains.yaml
```

### 6.4 Maintenance

**Regular Updates**:
- Review and update blocklists regularly
- Set expiration for temporary entries
- Monitor list growth and performance
- Archive old entries periodically

**Documentation**:
- Add clear descriptions to each list
- Document the purpose and update frequency
- Track data sources for external lists

---

## 7. Error Handling

### 7.1 Missing List Reference

If a rule references a non-existent list, compilation will fail:

```yaml
rule:
  when:
    all:
      - user.email in list.nonexistent_list  # Error: Unknown list
```

**Compiler Error**:
```
Error: Unknown list 'nonexistent_list'
Referenced in: rule 'email_check' at line 5
Available lists: email_blocklist, ip_blocklist, vip_users
```

### 7.2 Runtime Errors

If a list backend is unavailable:

```yaml
# Configure fallback behavior in list definition
id: external_sanctions
backend: api
url: "https://api.sanctions.io/check"
fallback: allow  # Options: allow, deny, error
```

---

## 8. Performance Considerations

### 8.1 Lookup Performance

Expected latency by backend type:

| Backend | P50 | P99 | Use Case |
|---------|-----|-----|----------|
| Memory | <0.1ms | <0.5ms | Small static lists |
| File | <1ms | <5ms | Medium static lists |
| Redis | 1-2ms | 5-10ms | High-frequency lookups |
| PostgreSQL | 5-10ms | 20-50ms | Complex queries, metadata |
| API | 50-200ms | 500ms-5s | External providers |

### 8.2 Optimization Tips

**1. Use Redis for Hot Lists**:
```yaml
# Frequently-checked IPs → Redis
id: ip_blocklist
backend: redis
```

**2. File Backend for Static Data**:
```yaml
# Country codes rarely change → File
id: country_codes
backend: file
```

**3. Batch Lookups When Possible**:
```yaml
# Check multiple lists in one rule
rule:
  when:
    any:
      - user.email in list.email_blocklist
      - user.ip in list.ip_blocklist
      # Runtime can optimize batch lookups
```

---

## 9. Summary

Lists provide an efficient way to check membership against predefined sets:

- **Simple syntax**: `field in list.list_id` and `field not in list.list_id`
- **Multiple backends**: PostgreSQL, Redis, File, Memory, API
- **Flexible use cases**: Blocklists, allowlists, watchlists, greylists
- **Performance**: Sub-millisecond to milliseconds depending on backend
- **Integration**: Works seamlessly with rules, rulesets, and pipelines

Lists are essential for production fraud detection systems, enabling rapid membership checks against millions of values with minimal latency.

---

## 10. Related Documentation

- **[expression.md](expression.md)** - List operators in expression language
- **[rule.md](rule.md)** - Using lists in rule conditions
- **[context.md](context.md)** - Accessing list lookup results in context

For implementation details, backend configuration, and API management, see `docs/CUSTOMLIST.md`.
