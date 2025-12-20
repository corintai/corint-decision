# CORINT Risk Definition Language (RDL)
## Expression Language Specification (v0.1)

The **Expression Language** is the foundation of RDL's condition evaluation system.  
It provides a rich set of operators, functions, and logical combinators for defining sophisticated risk conditions.

---

## 1. Overview

RDL expressions are used in:
- Rule `conditions`
- Pipeline `if` statements
- Branch `condition` clauses
- Dynamic score calculations

Expressions evaluate to boolean (for conditions) or numeric/string values (for calculations).

---

## 2. Basic Syntax

### 2.1 Field Access

Access nested fields using dot notation:

```yaml
user.profile.age
transaction.amount
device.fingerprint.hash
geo.location.country
```

**Feature Access**: All calculated features must be accessed using the `features.` namespace prefix:

```yaml
features.transaction_sum_7d
features.login_count_24h
features.unique_devices_7d
```

**Examples:**
```yaml
# Event fields (no prefix)
- event.amount > 1000
- user.profile.tier == "premium"

# Calculated features (features. prefix required)
- features.transaction_sum_7d > 5000
- features.login_count_24h > 10
```

### 2.2 Array/List Indexing

```yaml
transaction.items[0].price
user.addresses[1].city
```

### 2.3 Literals

```yaml
# Numbers
42
3.14
-100

# Strings
"hello"
'world'

# Booleans
true
false

# Null
null

# Arrays
["RU", "NG", "UA"]
[1, 2, 3, 4, 5]
```

---

## 3. Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `==` | Equal | `user.age == 25` |
| `!=` | Not equal | `device.type != "mobile"` |
| `<` | Less than | `amount < 1000` |
| `>` | Greater than | `score > 80` |
| `<=` | Less than or equal | `attempts <= 3` |
| `>=` | Greater than or equal | `balance >= 100` |

---

## 4. Logical Operators

### 4.1 AND (Implicit and Explicit)

Implicit AND (default for condition lists):

```yaml
conditions:
  - user.age > 18
  - geo.country == "US"
  # Both must be true
```

Explicit AND:

```yaml
conditions:
  - all:
      - user.age > 18
      - geo.country == "US"
      - device.is_trusted == true
```

### 4.2 OR

```yaml
conditions:
  - any:
      - geo.country in ["RU", "NG"]
      - user.risk_score > 80
      - device.is_new == true
  # At least one must be true
```

### 4.3 NOT

```yaml
conditions:
  - not:
      user.is_verified == true
      
  # Alternative syntax
  - user.is_verified != true
```

### 4.4 Complex Nested Logic

```yaml
conditions:
  # (A AND B) OR (C AND D)
  - any:
      - all:
          - user.age < 25
          - transaction.amount > 5000
      - all:
          - geo.country in ["RU", "CN"]
          - device.is_new == true
          
  # A AND (B OR C) AND NOT D
  - all:
      - user.is_active == true
      - any:
          - user.tier == "premium"
          - user.balance > 10000
      - not:
          user.is_blocked == true
```

---

## 5. Membership Operators

### 5.1 `in` - List Membership

```yaml
- geo.country in ["RU", "NG", "UA"]
- user.id in blocked_user_list
- transaction.category in ["gambling", "crypto"]
```

### 5.2 `not_in` - Exclusion

```yaml
- geo.country not_in ["US", "UK", "CA"]
- device.type not_in ["bot", "emulator"]
```

### 5.3 `contains` - String/Array Contains

```yaml
# String contains
- user.email contains "@suspicious.com"
- transaction.description contains "test"

# Array contains element
- user.roles contains "admin"
```

### 5.4 `in list.<list_id>` / `not in list.<list_id>` - Custom List Membership

Check membership in configured custom lists (supports PostgreSQL, Redis, File, Memory, and API backends):

```yaml
# Check if value is in a list
- user.email in list.email_blocklist
- event.ip in list.ip_blocklist
- user.id in list.vip_users

# Check if value is NOT in a list
- user.email not in list.email_allowlist
- device.id not in list.trusted_devices
```

**Note:** Lists must be configured in the list configuration file with a unique ID and backend type.

📖 **See [list.md](list.md)** for complete specification including:
- List configuration and backend types
- API endpoints for list management
- Performance considerations
- Best practices

---

## 6. Existence Operators

### 6.1 `exists` - Field Existence Check

```yaml
- user.phone exists
- device.fingerprint exists
```

### 6.2 `missing` - Field Absence Check

```yaml
- user.kyc_document missing
- transaction.receipt_id missing
```

### 6.3 `is_null` / `is_not_null`

```yaml
- user.last_login is_null
- device.id is_not_null
```

---

## 7. String Operators

### 7.1 `regex` - Regular Expression Match

```yaml
- user.email regex "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
- transaction.id regex "^TX-[0-9]{8}$"
```

### 7.2 `starts_with` / `ends_with`

```yaml
- user.email starts_with "test"
- user.phone ends_with "0000"
```

### 7.3 String Functions

```yaml
- lower(user.email) == "admin@company.com"
- upper(geo.country) in ["US", "UK"]
- trim(user.name) != ""
- length(user.password) >= 8
```

---

## 8. Arithmetic Operations

### 8.1 Basic Arithmetic

```yaml
- transaction.amount + transaction.fee > 1000
- user.balance - transaction.amount < 0
- transaction.items[0].price * transaction.items[0].quantity > 500
- transaction.total / transaction.items_count > 100
- user.age % 10 == 0
```

### 8.2 Mathematical Functions

```yaml
- abs(account.balance_change) > 10000
- max(transaction.amounts) > 50000
- min(user.credit_scores) < 300
- round(transaction.fee_rate, 2) == 0.03
- floor(user.age / 10) == 6
- ceil(loan.interest_rate) == 5
```

---

## 9. Aggregation Functions

### 9.1 Time-Based Aggregations

```yaml
# Count events in time window
- count(login.attempts, last_hour) > 5
- count(transactions, last_24h) > 100

# Sum over time window
- sum(transaction.amounts, last_24h) > 10000
- sum(withdrawal.amounts, last_7d) > user.monthly_limit

# Average over time window
- avg(transaction.amount, last_30d) < 100
- avg(login.duration, last_week) > 300

# Max/Min over time window
- max(transaction.amount, last_month) > 50000
- min(account.balance, last_year) < 0
```

### 9.2 Time Windows

Supported time window expressions:

- `last_hour` / `last_1h`
- `last_24h` / `last_day`
- `last_7d` / `last_week`
- `last_30d` / `last_month`
- `last_90d` / `last_quarter`
- `last_365d` / `last_year`

Custom windows:

```yaml
- count(events, "2h") > 10        # last 2 hours
- sum(amounts, "15m") > 5000      # last 15 minutes
- avg(scores, "6M") > 0.8         # last 6 months
```

### 9.3 Advanced Statistical Analysis

For more complex statistical analysis and feature engineering (such as distinct count, association analysis, percentiles, etc.), see the dedicated **Feature Engineering** specification:

📖 **See [feature.md](feature.md)** for:
- **Distinct count** - `count_distinct()` for unique value counting
- **Association analysis** - IP/Device/User relationship metrics
- **Percentile calculations** - Outlier detection with percentiles
- **Velocity features** - Rate of change and pattern detection
- **Statistical anomaly detection** - Z-scores, standard deviation
- **Feature groups** - Organized feature extraction

**Example preview:**

```yaml
# Distinct count - count unique values
- count_distinct(
    field: device.id,
    where: {geo.ip == event.geo.ip},
    window: last_5h
  ) > 10

# Percentile-based outlier detection
- event.transaction.amount > percentile(
    transaction.amount,
    where: {user.id == event.user.id},
    window: last_30d,
    p: 95
  )
```

---

## 10. Array Functions

```yaml
# Array length
- length(user.addresses) > 1
- size(transaction.items) >= 5

# Array contains
- contains(user.roles, "admin")
- contains(transaction.flags, "suspicious")

# Array operations
- first(user.login_history).timestamp > "2024-01-01"
- last(transactions).amount > 1000
- unique(transaction.recipient_ids).length > 10
```

---

## 11. Date/Time Functions

```yaml
# Current time
- now() - user.created_at > duration("30d")

# Date parsing
- parse_date(user.birth_date) < parse_date("2000-01-01")

# Date arithmetic
- days_between(user.last_login, now()) > 90
- hours_since(transaction.timestamp) < 24
- age_in_years(user.birth_date) >= 18

# Date components
- year(transaction.date) == 2024
- month(user.created_at) == 12
- day_of_week(transaction.date) in ["saturday", "sunday"]
- hour(transaction.timestamp) >= 22 || hour(transaction.timestamp) <= 6
```

---

## 12. Type Conversion Functions

```yaml
- to_number(user.age_string) > 18
- to_string(transaction.amount) contains ".99"
- to_bool(user.verified_flag) == true
```

---

## 13. LLM Expression Integration

### 13.1 LLM Result Access

```yaml
# Access LLM reasoning output
- LLM.reason(event) contains "suspicious"
- LLM.reason(user_profile) not_contains "trustworthy"

# Access LLM tags
- LLM.tags contains "device_mismatch"
- LLM.tags contains "behavior_anomaly"

# Access LLM scores
- LLM.score > 0.7
- LLM.confidence >= 0.8

# Access structured LLM output
- LLM.output.risk_score > 0.3
- LLM.output.employment_stability < 0.4
- LLM.output.is_fraudulent == true
```

### 13.2 Multiple LLM Results

```yaml
# Named LLM steps
- LLM.user_analysis.risk_level == "high"
- LLM.transaction_check.anomaly_score > 0.6

# Compare multiple LLM results
- LLM.model_a.score > 0.7 && LLM.model_b.score > 0.7
```

---

## 14. External API Expression Integration

```yaml
# Third-party risk scores
- external_api.Chainalysis.risk_score > 80
- external_api.MaxMind.risk_level == "high"

# Device fingerprinting
- external_api.FingerprintJS.confidence > 0.9
- external_api.DeviceAtlas.is_bot == true

# Email validation
- external_api.EmailRep.reputation < 50
- external_api.HaveIBeenPwned.breaches > 0

# Custom API fields
- external_api.CustomRiskService.fraud_probability >= 0.75
```

---

## 15. Context Variables

### 15.1 Pipeline Variables

```yaml
# Define variables in pipeline
vars:
  risk_threshold: 80
  max_transaction: 10000

# Use in conditions
conditions:
  - user.risk_score > vars.risk_threshold
  - transaction.amount <= vars.max_transaction
```

### 15.2 Previous Step Results

```yaml
# Reference results from previous steps
- context.extract_device.device_score > 50
- context.llm_analysis.risk_level == "high"
- context.ip_reputation.score < 30
```

### 15.3 System Variables

```yaml
# Built-in system variables
- sys.current_time > user.session_expires_at
- sys.request_id exists
- sys.environment == "production"
```

---

## 16. Conditional Expressions (Ternary)

```yaml
# Ternary operator: condition ? true_value : false_value
score: user.is_premium ? 0 : 50
weight: transaction.amount > 10000 ? 0.8 : 0.5

# In conditions
- (user.age < 18 ? user.guardian_verified : user.verified) == true
```

---

## 17. Performance Considerations

### 17.1 Short-Circuit Evaluation

Logical operators use short-circuit evaluation:

```yaml
# If first condition is false, second is not evaluated
- user.is_active == true && expensive_check(user.id) == true

# If first condition is true, second is not evaluated
- user.is_blocked == true || expensive_api_call() > 0.8
```

### 17.2 Caching Hints

```yaml
# Suggest caching for expensive operations
- @cache(ttl=3600) external_api.IPQuality.score > 80
- @cache(key="device:{device.id}") complex_calculation() > threshold
```

---

## 18. BNF Grammar (Formal)

```
EXPRESSION ::= LOGICAL_EXPR

LOGICAL_EXPR ::= 
    | OR_EXPR

OR_EXPR ::= AND_EXPR { "||" AND_EXPR }
          | "any:" LIST_OF_EXPR

AND_EXPR ::= NOT_EXPR { "&&" NOT_EXPR }
           | "all:" LIST_OF_EXPR

NOT_EXPR ::= "!" COMPARISON_EXPR
           | "not:" COMPARISON_EXPR
           | COMPARISON_EXPR

COMPARISON_EXPR ::=
    | ADDITIVE_EXPR COMPARE_OP ADDITIVE_EXPR
    | ADDITIVE_EXPR "in" (ARRAY_LITERAL | LIST_REFERENCE)
    | ADDITIVE_EXPR "not" "in" (ARRAY_LITERAL | LIST_REFERENCE)
    | ADDITIVE_EXPR "contains" ADDITIVE_EXPR
    | ADDITIVE_EXPR "regex" STRING_LITERAL
    | ADDITIVE_EXPR "exists"
    | ADDITIVE_EXPR "missing"
    | ADDITIVE_EXPR "is_null"
    | ADDITIVE_EXPR "is_not_null"
    | ADDITIVE_EXPR

COMPARE_OP ::= "==" | "!=" | "<" | ">" | "<=" | ">="

ADDITIVE_EXPR ::= MULTIPLICATIVE_EXPR { ("+" | "-") MULTIPLICATIVE_EXPR }

MULTIPLICATIVE_EXPR ::= UNARY_EXPR { ("*" | "/" | "%") UNARY_EXPR }

UNARY_EXPR ::= "-" PRIMARY_EXPR
             | PRIMARY_EXPR

PRIMARY_EXPR ::=
    | LITERAL
    | FIELD_ACCESS
    | FUNCTION_CALL
    | LLM_EXPR
    | EXTERNAL_API_EXPR
    | LIST_REFERENCE
    | VAR_EXPR
    | "(" LOGICAL_EXPR ")"
    | TERNARY_EXPR

TERNARY_EXPR ::= LOGICAL_EXPR "?" EXPRESSION ":" EXPRESSION

FIELD_ACCESS ::= IDENT { "." IDENT | "[" NUMBER "]" }

FUNCTION_CALL ::= IDENT "(" [ EXPRESSION { "," EXPRESSION } ] ")"

LLM_EXPR ::= 
    | "LLM.reason(" EXPRESSION ")"
    | "LLM.tags"
    | "LLM.score"
    | "LLM.confidence"
    | "LLM.output." FIELD_ACCESS
    | "LLM." IDENT "." FIELD_ACCESS

EXTERNAL_API_EXPR ::= "external_api." IDENT "." FIELD_ACCESS

LIST_REFERENCE ::= "list" "." IDENT

VAR_EXPR ::=
    | "vars." IDENT
    | "context." FIELD_ACCESS
    | "sys." IDENT

LITERAL ::= NUMBER | STRING_LITERAL | BOOLEAN | NULL | ARRAY_LITERAL

ARRAY_LITERAL ::= "[" [ LITERAL { "," LITERAL } ] "]"

STRING_LITERAL ::= '"' CHAR* '"' | "'" CHAR* "'"

BOOLEAN ::= "true" | "false"

NULL ::= "null"

NUMBER ::= DIGIT+ [ "." DIGIT+ ]

IDENT ::= (ALPHA | "_") (ALPHA | DIGIT | "_")*

LIST_OF_EXPR ::= "-" EXPRESSION { "-" EXPRESSION }
```

---

## 19. Error Handling in Expressions

### 19.1 Null-Safe Navigation

```yaml
# Use ?. for null-safe access
- user.profile?.age > 18          # Returns false if profile is null
- transaction.metadata?.risk exists
```

### 19.2 Default Values

```yaml
# Use ?? for default values
- (user.risk_score ?? 50) > 80
- (device.trust_score ?? 0) >= 70
```

### 19.3 Try-Catch in Expressions

```yaml
# Graceful degradation for expression errors
- try(external_api.SlowService.score, default=50) > 80
```

---

## 20. Best Practices

### 20.1 Readability

✅ **Good:**
```yaml
conditions:
  - all:
      - user.age >= 18
      - user.kyc_verified == true
      - user.risk_score < 50
```

❌ **Bad:**
```yaml
conditions:
  - user.age>=18&&user.kyc_verified==true&&user.risk_score<50
```

### 20.2 Performance

✅ **Good:** Put cheap checks first
```yaml
conditions:
  - user.is_blocked == false        # Fast: simple field check
  - device.is_new == true            # Fast: simple field check
  - expensive_api_call() > 0.8       # Slow: only evaluated if above pass
```

❌ **Bad:** Expensive checks first
```yaml
conditions:
  - expensive_api_call() > 0.8       # Always evaluated
  - user.is_blocked == false
```

### 20.3 Maintainability

Use variables for complex expressions:

✅ **Good:**
```yaml
vars:
  is_high_risk_country: geo.country in ["RU", "NG", "UA", "CN"]
  is_large_transaction: transaction.amount > 10000
  is_new_user: days_since(user.created_at) < 30

conditions:
  - all:
      - vars.is_high_risk_country
      - vars.is_large_transaction
      - vars.is_new_user
```

---

## 21. Summary

The RDL Expression Language provides:

- **Rich operator set** for flexible condition building
- **Logical combinators** (AND/OR/NOT) for complex logic
- **Built-in functions** for common operations
- **Time-based aggregations** for behavioral analysis
- **LLM and external API integration** for hybrid intelligence
- **Type-safe evaluation** with null handling
- **Performance optimizations** through short-circuit evaluation

This expression system enables sophisticated risk logic while maintaining readability and maintainability.

