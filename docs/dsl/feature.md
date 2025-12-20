# CORINT Risk Definition Language (RDL)
## Feature Engineering and Statistical Analysis Specification (v0.1)

Feature engineering is the foundation of risk control decision-making. This document defines the statistical analysis, feature computation, and aggregation query capabilities in CORINT.

---

## 1. Overview

### 1.1 Feature Engineering in Risk Management

In risk control scenarios, extensive statistical analysis computations are needed to identify risk patterns:

- **Behavioral Features**: Login count in the past 7 days, transaction frequency, etc.
- **Association Features**: Number of device IDs associated with the same IP address, number of accounts associated with the same device
- **Temporal Features**: Time since last transaction, average transaction interval, etc.
- **Aggregation Features**: Average transaction amount in the past 30 days, maximum single transaction, etc.

### 1.2 Feature Access Syntax

**There are two ways to access features in rule conditions:**

#### Method 1: Direct Feature Access (features.xxx)

**All registered features must be accessed using the `features.` namespace prefix:**

```yaml
rule:
  id: high_transaction_volume
  when:
    all:
      - features.transaction_sum_7d > 5000      # ✅ Direct feature access
      - features.transaction_count_24h > 10     # ✅ Direct feature access
      - event.amount > 1000                      # ✅ Event field (no prefix)
```

**Why use `features.` prefix?**
- ✅ **Clear distinction**: Immediately identifies calculated features vs event fields
- ✅ **No conflicts**: Completely avoids naming collisions
- ✅ **Better debugging**: Easy to trace value sources
- ✅ **Type safety**: Prevents accidental use of event fields

#### Method 2: Context Feature Groups (context.feature_group.xxx)

**Features extracted in pipeline steps and stored in context can be accessed via context:**

```yaml
# In pipeline
- type: extract
  id: statistical_features
  output: context.statistical_features  # Store in context
  
# In rule
rule:
  when:
    all:
      - context.statistical_features.devices_per_ip_5h > 10  # ✅ Context feature group
      - context.statistical_features.users_per_ip_5h > 5
```

**When to use which?**
- Use `features.xxx` for **registered features** defined in feature YAML files
- Use `context.feature_group.xxx` for **pipeline-extracted features** stored in context

### 1.2 Feature Types

| Type | Description | Example |
|------|-------------|---------|
| `behavioral` | User behavior patterns | Login frequency, transaction velocity |
| `association` | Entity relationships | IP-to-device mapping, device-to-user mapping |
| `temporal` | Time-based features | Time since last action, hour of day |
| `statistical` | Aggregated statistics | Average, median, percentile |
| `derived` | Computed from other features | Velocity ratio, deviation score |

---

## 2. Statistical Aggregation Functions

### 2.1 Basic Aggregations

Extends the basic aggregation functions from `expression.md`:

```yaml
# Count
- count(user.logins, last_7d) > 100
- count(user.transactions, last_24h) >= 5

# Sum
- sum(transaction.amounts, last_7d) > 50000
- sum(withdrawal.amounts, last_month) > user.monthly_limit

# Average
- avg(transaction.amount, last_30d) < 100
- avg(login.session_duration, last_week) > 3600

# Min/Max
- max(transaction.amount, last_month) > 100000
- min(account.balance, last_quarter) < 0

# Median
- median(transaction.amount, last_30d) > 1000

# Standard Deviation
- stddev(transaction.amount, last_30d) > 5000
```

### 2.2 Distinct Count (Unique Value Counting)

**Core Capability**: Count unique values - the most commonly used method for association analysis in risk control.

```yaml
# Distinct count - unique value counting
# Syntax: count_distinct(field, filters, time_window)

# How many device IDs are associated with the same IP address in the past 5 hours
- count_distinct(
    field: device.id,
    where: {geo.ip == event.geo.ip},
    window: last_5h
  ) > 10

# How many users are associated with the same device in the past 24 hours
- count_distinct(
    field: user.id,
    where: {device.id == event.device.id},
    window: last_24h
  ) > 5

# How many different IP addresses has this user logged in from in the past 7 days
- count_distinct(
    field: geo.ip,
    where: {user.id == event.user.id},
    window: last_7d
  ) > 20

# How many different devices has this user used in the past 1 hour
- count_distinct(
    field: device.id,
    where: {user.id == event.user.id},
    window: last_1h
  ) >= 3
```

### 2.3 Percentile

```yaml
# Percentile calculation
# Useful for detecting outliers

# Is the current transaction amount above the user's 95th percentile for the past 30 days
- event.transaction.amount > percentile(
    transaction.amount,
    where: {user.id == event.user.id},
    window: last_30d,
    p: 95
  )

# Is the current login duration below the 10th percentile (abnormally fast)
- event.login.duration < percentile(
    login.duration,
    where: {user.id == event.user.id},
    window: last_90d,
    p: 10
  )
```

---

## 3. Feature Definition

### 3.1 Defining Features in Pipeline

Define reusable features in the Pipeline:

```yaml
pipeline:
  # Feature extraction step
  - type: extract
    id: user_behavior_features
    
    # Define features
    features:
      # Simple field extraction
      - name: user_tier
        value: event.user.profile.tier
        
      # Time-based aggregation
      - name: login_count_7d
        value: count(user.logins, last_7d)
        
      # Distinct count
      - name: ip_count_7d
        value: count_distinct(
            field: geo.ip,
            where: {user.id == event.user.id},
            window: last_7d
          )
          
      # Statistical aggregation
      - name: avg_transaction_30d
        value: avg(transaction.amount, last_30d)
        
      # Derived feature
      - name: transaction_velocity
        value: count(transactions, last_24h) / count(transactions, last_7d)
        
      # Complex condition
      - name: is_high_risk_pattern
        value: |
          count_distinct(device.id, {user.id == event.user.id}, last_24h) > 3 &&
          count_distinct(geo.ip, {user.id == event.user.id}, last_24h) > 5
    
    # Output to context
    output: context.user_features
```

### 3.2 Feature Groups

Organize related features into groups:

```yaml
- type: extract
  id: extract_features
  
  feature_groups:
    # User behavioral features
    - group: user_behavior
      features:
        - name: login_frequency_7d
          value: count(user.logins, last_7d)
          
        - name: failed_login_ratio_7d
          value: count(user.failed_logins, last_7d) / count(user.logins, last_7d)
          
        - name: avg_session_duration_30d
          value: avg(session.duration, last_30d)
    
    # Association features
    - group: associations
      features:
        - name: devices_per_user_24h
          value: count_distinct(device.id, {user.id == event.user.id}, last_24h)
          
        - name: users_per_device_24h
          value: count_distinct(user.id, {device.id == event.device.id}, last_24h)
          
        - name: users_per_ip_5h
          value: count_distinct(user.id, {geo.ip == event.geo.ip}, last_5h)
          
        - name: devices_per_ip_5h
          value: count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h)
    
    # Transaction features
    - group: transaction_stats
      features:
        - name: transaction_count_24h
          value: count(transactions, last_24h)
          
        - name: transaction_sum_24h
          value: sum(transaction.amount, last_24h)
          
        - name: avg_transaction_30d
          value: avg(transaction.amount, last_30d)
          
        - name: max_transaction_90d
          value: max(transaction.amount, last_90d)
```

---

## 4. Association Analysis

### 4.1 Entity Relationship Features

Association analysis is a core capability in risk control:

```yaml
# IP → Device Association
- name: devices_per_ip
  description: Number of devices associated with the same IP address
  formula: count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h)
  risk: High if > 10

# IP → User Association
- name: users_per_ip
  description: Number of users associated with the same IP address
  formula: count_distinct(user.id, {geo.ip == event.geo.ip}, last_5h)
  risk: High if > 5

# Device → User Association
- name: users_per_device
  description: Number of users associated with the same device
  formula: count_distinct(user.id, {device.id == event.device.id}, last_24h)
  risk: High if > 3

# User → Device Association
- name: devices_per_user
  description: Number of devices used by the same user
  formula: count_distinct(device.id, {user.id == event.user.id}, last_7d)
  risk: Medium if > 5

# Email Domain → User Association
- name: users_per_email_domain
  description: Number of users with the same email domain
  formula: count_distinct(user.id, {email_domain(user.email) == email_domain(event.user.email)}, last_30d)
  risk: High if > 100

# Phone → User Association
- name: users_per_phone
  description: Number of users associated with the same phone number
  formula: count_distinct(user.id, {user.phone == event.user.phone}, last_90d)
  risk: High if > 3
```

### 4.2 Association Matrix

Define a complete association matrix:

```yaml
- type: extract
  id: association_analysis
  
  associations:
    # IP-based associations
    - entity: ip_address
      metrics:
        - devices_count: count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h)
        - users_count: count_distinct(user.id, {geo.ip == event.geo.ip}, last_5h)
        - transactions_count: count(transactions, {geo.ip == event.geo.ip}, last_5h)
        - countries_count: count_distinct(geo.country, {geo.ip == event.geo.ip}, last_24h)
    
    # Device-based associations
    - entity: device
      metrics:
        - users_count: count_distinct(user.id, {device.id == event.device.id}, last_24h)
        - ips_count: count_distinct(geo.ip, {device.id == event.device.id}, last_24h)
        - transactions_count: count(transactions, {device.id == event.device.id}, last_24h)
    
    # User-based associations
    - entity: user
      metrics:
        - devices_count: count_distinct(device.id, {user.id == event.user.id}, last_7d)
        - ips_count: count_distinct(geo.ip, {user.id == event.user.id}, last_7d)
        - email_domains_count: count_distinct(email_domain(user.email), {user.id == event.user.id}, last_30d)
```

---

## 5. Temporal Features

### 5.1 Time Since Last Event

```yaml
# Time since last event
- name: hours_since_last_login
  value: hours_since(user.last_login_time)
  
- name: days_since_last_transaction
  value: days_since(user.last_transaction_time)
  
- name: minutes_since_last_failed_attempt
  value: minutes_since(user.last_failed_login_time)

# Usage in conditions
when:
  all:
  # Login again less than 5 minutes after last login (possibly a bot)
    - hours_since_last_login < 0.08  # 5 minutes
  
  # User with no transactions for a long time suddenly makes a large transaction
    - days_since_last_transaction > 90 && event.transaction.amount > 10000
```

### 5.2 Time Pattern Features

```yaml
# Time of day patterns
- name: is_off_hours
  value: hour(event.timestamp) >= 22 || hour(event.timestamp) <= 6
  
- name: is_weekend
  value: day_of_week(event.timestamp) in ["saturday", "sunday"]

# Transaction timing patterns
- name: avg_transaction_interval_7d
  value: avg_interval(transactions, last_7d)
  
- name: transaction_interval_deviation
  value: |
    abs(current_interval - avg_interval(transactions, last_30d)) / 
    stddev_interval(transactions, last_30d)
```

### 5.3 Velocity Features

```yaml
# Velocity = rate of change over time
- name: login_velocity_change
  description: Login frequency change rate
  value: |
    (count(logins, last_24h) / count(logins, last_7d)) * 7
    
- name: transaction_velocity_change
  description: Transaction frequency change rate
  value: |
    (count(transactions, last_24h) / count(transactions, last_30d)) * 30
    
- name: spending_velocity
  description: Spending speed
  value: sum(transaction.amounts, last_24h) / 24  # per hour

# Usage
when:
  all:
  # Login frequency suddenly increased by 10 times (abnormal behavior)
    - login_velocity_change > 10
  
  # Spending speed exceeds historical average by 5 times
    - spending_velocity > avg(spending_velocity, last_90d) * 5
```

---

## 6. Window Functions

### 6.1 Time Windows

Supported time windows:

```yaml
# Minutes
last_1m, last_5m, last_15m, last_30m

# Hours
last_1h, last_3h, last_5h, last_6h, last_12h, last_24h

# Days
last_1d, last_3d, last_7d, last_14d, last_30d, last_60d, last_90d

# Weeks/Months/Years
last_week, last_month, last_quarter, last_year

# Custom windows
"5m", "3h", "7d", "30d", "180d"
```

### 6.2 Rolling Windows

Rolling window calculations:

```yaml
# Rolling average
- name: rolling_avg_transaction_7d
  value: rolling_avg(transaction.amount, window=7d, step=1d)
  
# Rolling sum
- name: rolling_sum_transactions_24h
  value: rolling_sum(transaction.amount, window=24h, step=1h)

# Detect sudden changes
when:
  all:
  # Current 24-hour transaction amount is 3 times the 7-day rolling average
    - sum(transaction.amounts, last_24h) > 
    rolling_avg(sum(transaction.amounts, 24h), window=7d) * 3
```

### 6.3 Session Windows

Session-based windows:

```yaml
# Session-based aggregation
- name: current_session_actions
  value: count(actions, current_session)
  
- name: current_session_duration
  value: session_duration(current_session)

when:
  all:
  # Too many actions in a single session
    - current_session_actions > 50
  
  # Abnormally short session duration
    - current_session_duration < 10 && current_session_actions > 20
```

---

## 7. Feature Store Integration

### 7.1 Pre-computed Features

For computationally expensive features, they can be pre-computed and stored:

```yaml
- type: extract
  id: load_precomputed_features
  
  feature_store:
    enabled: true
    namespace: user_features
    
    # Load pre-computed features
    load:
      - feature: user_30d_transaction_stats
        key: user.id
        cache_ttl: 3600
        
      - feature: user_device_association_7d
        key: user.id
        cache_ttl: 1800
        
    # Fallback if feature store unavailable
    on_miss:
      action: compute
      fallback_ttl: 300
```

### 7.2 Feature Caching

Feature computation caching strategy:

```yaml
- type: extract
  id: compute_features
  
  features:
    - name: expensive_feature
      value: count_distinct(device.id, {user.id == event.user.id}, last_30d)
      
      # Cache configuration
      cache:
        enabled: true
        ttl: 3600  # 1 hour
        key: "feature:expensive_feature:{event.user.id}"
        
        # Cache strategy
        strategy: lazy  # lazy | eager | refresh
```

---

## 8. Feature Engineering Patterns

### 8.1 Account Takeover Detection Features

```yaml
- type: extract
  id: account_takeover_features
  
  features:
    # Device change
    - name: new_device_indicator
      value: device.id not_in user.known_devices
      
    - name: device_change_frequency_7d
      value: count_distinct(device.id, {user.id == event.user.id}, last_7d)
    
    # Geographic location change
    - name: geo_change_indicator
      value: geo.country != user.home_country
      
    - name: impossible_travel
      value: |
        distance(geo.location, user.last_login_location) / 
        hours_since(user.last_login_time) > 800  # km/h
    
    # Behavior change
    - name: login_pattern_deviation
      value: |
        abs(hour(event.timestamp) - user.typical_login_hour) > 6
        
    - name: failed_attempts_before_success
      value: count(user.failed_logins, last_1h)
    
    # Association risk
    - name: shared_ip_risk
      value: count_distinct(user.id, {geo.ip == event.geo.ip}, last_5h) > 5
      
    - name: shared_device_risk
      value: count_distinct(user.id, {device.id == event.device.id}, last_24h) > 3
```

### 8.2 Payment Fraud Detection Features

```yaml
- type: extract
  id: payment_fraud_features
  
  features:
    # Transaction patterns
    - name: transaction_amount_deviation
      value: |
        abs(event.transaction.amount - avg(transaction.amount, last_30d)) /
        stddev(transaction.amount, last_30d)
        
    - name: high_value_transaction_indicator
      value: event.transaction.amount > percentile(transaction.amount, last_90d, p=95)
    
    # Velocity checks
    - name: transaction_velocity_24h
      value: count(transactions, last_24h)
      
    - name: rapid_transactions_5m
      value: count(transactions, last_5m) > 3
      
    - name: spending_velocity_change
      value: |
        sum(transaction.amounts, last_24h) / 
        avg(sum(transaction.amounts, 24h), last_30d)
    
    # Merchant patterns
    - name: new_merchant_indicator
      value: transaction.merchant.id not_in user.known_merchants
      
    - name: high_risk_merchant_category
      value: transaction.merchant.category in ["gambling", "crypto", "prepaid"]
    
    # Payment method risks
    - name: new_payment_method
      value: transaction.payment_method.id not_in user.known_payment_methods
      
    - name: payment_method_change_frequency
      value: count_distinct(payment_method.id, {user.id == event.user.id}, last_7d)
```

### 8.3 Loan Application Fraud Features

```yaml
- type: extract
  id: loan_fraud_features
  
  features:
    # Application velocity
    - name: application_count_24h
      value: count(loan_applications, {user.id == event.user.id}, last_24h)
      
    - name: applications_per_device_24h
      value: count(loan_applications, {device.id == event.device.id}, last_24h)
      
    - name: applications_per_ip_24h
      value: count(loan_applications, {geo.ip == event.geo.ip}, last_24h)
    
    # Identity reuse
    - name: users_with_same_phone
      value: count_distinct(user.id, {user.phone == event.user.phone}, last_90d)
      
    - name: users_with_same_email
      value: count_distinct(user.id, {user.email == event.user.email}, last_90d)
      
    - name: users_with_same_address
      value: count_distinct(user.id, {user.address == event.user.address}, last_90d)
    
    # Document reuse
    - name: users_with_same_id_number
      value: count_distinct(user.id, {user.id_number == event.user.id_number}, last_365d)
      
    - name: duplicate_document_hash
      value: count(applications, {document.hash == event.document.hash}, last_90d) > 1
```

---

## 9. Feature Dependencies and DAG

### 9.1 Feature Dependency Graph

Some features depend on other features, forming a DAG:

```yaml
- type: extract
  id: dependent_features
  
  features:
    # Level 1: Base features
    - name: transaction_count_24h
      value: count(transactions, last_24h)
      level: 1
      
    - name: transaction_count_7d
      value: count(transactions, last_7d)
      level: 1
    
    # Level 2: Derived features (depend on level 1)
    - name: transaction_velocity_change
      value: features.transaction_count_24h / features.transaction_count_7d * 7
      depends_on: [transaction_count_24h, transaction_count_7d]
      level: 2
      
    - name: avg_daily_transactions
      value: features.transaction_count_7d / 7
      depends_on: [transaction_count_7d]
      level: 2
    
    # Level 3: Higher-level features
    - name: velocity_risk_score
      value: |
        features.transaction_velocity_change > 5 ? 100 :
        features.transaction_velocity_change > 3 ? 60 :
        features.transaction_velocity_change > 2 ? 30 : 0
      depends_on: [transaction_velocity_change]
      level: 3
```

---

## 10. Performance Optimization

### 10.1 Query Optimization

```yaml
# Bad: Multiple queries for same time window
when:
  all:
    - count(transactions, last_24h) > 10
    - sum(transaction.amounts, last_24h) > 1000
    - max(transaction.amount, last_24h) > 500

# Good: Batch query with single scan
- type: extract
  id: transaction_stats_24h
  batch_query:
    entity: transactions
    window: last_24h
    metrics:
      - count
      - sum: amount
      - max: amount
    output: context.tx_stats_24h

when:
  all:
    - context.tx_stats_24h.count > 10
    - context.tx_stats_24h.sum_amount > 1000
    - context.tx_stats_24h.max_amount > 500
```

### 10.2 Materialized Features

```yaml
# For frequently used expensive features
- type: extract
  id: load_materialized_features
  
  materialized_views:
    # Pre-aggregated user statistics
    - view: user_7d_stats
      refresh: hourly
      features:
        - login_count_7d
        - transaction_count_7d
        - devices_used_7d
        - ips_used_7d
    
    # Pre-computed association metrics
    - view: ip_association_5h
      refresh: every_5m
      features:
        - devices_per_ip
        - users_per_ip
```

---

## 11. Feature Validation

### 11.1 Feature Schema Definition

```yaml
- type: extract
  id: validated_features
  
  output_schema:
    features:
      login_count_7d:
        type: integer
        min: 0
        max: 10000
        
      avg_transaction_30d:
        type: float
        min: 0.0
        max: 1000000.0
        
      devices_per_ip_5h:
        type: integer
        min: 0
        max: 1000
        
      is_high_risk_pattern:
        type: boolean
  
  # Validation on error
  on_validation_error:
    action: fallback
    fallback_values:
      login_count_7d: 0
      avg_transaction_30d: 0.0
```

---

## 12. Complete Example

### 12.1 Comprehensive Risk Feature Engineering Example

```yaml
version: "0.1"

pipeline:
  - type: extract
    id: comprehensive_risk_features
    
    feature_groups:
      # User behavior statistics group
      - group: user_behavior_stats
        features:
          # Login behavior
          - name: login_count_7d
            value: count(user.logins, last_7d)
            description: Login count in the past 7 days
            
          - name: login_count_24h
            value: count(user.logins, last_24h)
            description: Login count in the past 24 hours
            
          - name: failed_login_count_24h
            value: count(user.failed_logins, last_24h)
            description: Failed login count in the past 24 hours
            
          - name: failed_login_ratio_7d
            value: |
              count(user.failed_logins, last_7d) / 
              max(count(user.logins, last_7d), 1)
            description: Failed login ratio in the past 7 days
          
          # Transaction behavior
          - name: transaction_count_24h
            value: count(transactions, {user.id == event.user.id}, last_24h)
            description: Transaction count in the past 24 hours
            
          - name: transaction_sum_24h
            value: sum(transaction.amount, {user.id == event.user.id}, last_24h)
            description: Total transaction amount in the past 24 hours
            
          - name: avg_transaction_30d
            value: avg(transaction.amount, {user.id == event.user.id}, last_30d)
            description: Average transaction amount in the past 30 days
            
          - name: max_transaction_30d
            value: max(transaction.amount, {user.id == event.user.id}, last_30d)
            description: Maximum transaction amount in the past 30 days
      
      # Association analysis group
      - group: association_analysis
        features:
          # IP associations
          - name: devices_per_ip_5h
            value: count_distinct(
                field: device.id,
                where: {geo.ip == event.geo.ip},
                window: last_5h
              )
            description: Number of devices associated with the same IP in the past 5 hours
            risk_threshold: 10
            risk_level: high
            
          - name: users_per_ip_5h
            value: count_distinct(
                field: user.id,
                where: {geo.ip == event.geo.ip},
                window: last_5h
              )
            description: Number of users associated with the same IP in the past 5 hours
            risk_threshold: 5
            risk_level: high
            
          # Device associations
          - name: users_per_device_24h
            value: count_distinct(
                field: user.id,
                where: {device.id == event.device.id},
                window: last_24h
              )
            description: Number of users associated with the same device in the past 24 hours
            risk_threshold: 3
            risk_level: high
            
          # User associations
          - name: devices_per_user_7d
            value: count_distinct(
                field: device.id,
                where: {user.id == event.user.id},
                window: last_7d
              )
            description: Number of devices used by the user in the past 7 days
            
          - name: ips_per_user_7d
            value: count_distinct(
                field: geo.ip,
                where: {user.id == event.user.id},
                window: last_7d
              )
            description: Number of IP addresses used by the user in the past 7 days
      
      # Temporal features group
      - group: temporal_features
        features:
          - name: hours_since_last_login
            value: hours_since(user.last_login_time)
            description: Hours since last login
            
          - name: days_since_last_transaction
            value: days_since(user.last_transaction_time)
            description: Days since last transaction
            
          - name: is_off_hours
            value: hour(event.timestamp) >= 22 || hour(event.timestamp) <= 6
            description: Whether it is off-hours
            
          - name: login_velocity_change
            value: |
              (count(logins, {user.id == event.user.id}, last_24h) / 
               count(logins, {user.id == event.user.id}, last_7d)) * 7
            description: Login frequency change rate
      
      # Statistical features group
      - group: statistical_features
        features:
          - name: transaction_amount_zscore
            value: |
              (event.transaction.amount - avg(transaction.amount, last_30d)) /
              stddev(transaction.amount, last_30d)
            description: Transaction amount Z-Score
            
          - name: is_amount_outlier
            value: event.transaction.amount > percentile(transaction.amount, last_90d, p=95)
            description: Whether it is an abnormally large transaction
            
          - name: median_transaction_30d
            value: median(transaction.amount, {user.id == event.user.id}, last_30d)
            description: Median transaction amount in the past 30 days
    
    # Cache configuration
    cache:
      enabled: true
      ttl: 600  # 10 minutes
      key_template: "features:{event.user.id}:{event.type}"
    
    # Output
    output: context.risk_features
  
  # Use features in rules
  - include:
      ruleset: risk_detection_rules
```

---

## 13. Integration with Rules

### 13.1 Using Features in Rules

```yaml
rule:
  id: high_ip_device_association
  name: High IP-Device Association Risk
  description: Detect IP addresses associated with too many devices
  
  when:
    event.type: login
    all:
      # Use pre-computed features
      - context.risk_features.devices_per_ip_5h > 10
      - context.risk_features.users_per_ip_5h > 5
      
  score: 80

---

rule:
  id: abnormal_user_velocity
  name: Abnormal User Velocity Pattern
  description: Detect unusual user activity velocity
  
  when:
    event.type: transaction
    all:
      - context.risk_features.login_velocity_change > 5
      - context.risk_features.transaction_count_24h > context.risk_features.avg_transaction_30d * 3
      
  score: 70
```

---

## 14. Best Practices

### 14.1 Feature Design Principles

✅ **Good:**
```yaml
# Clear naming
- name: devices_per_ip_5h
  value: count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h)

# With description and threshold
- name: users_per_device_24h
  description: Number of users associated with the same device (past 24 hours)
  value: count_distinct(user.id, {device.id == event.device.id}, last_24h)
  risk_threshold: 3
  risk_level: high
```

❌ **Bad:**
```yaml
# Unclear naming
- name: metric1
  value: count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h)

# No documentation
- name: x
  value: count_distinct(user.id, {device.id == event.device.id}, last_24h)
```

### 14.2 Performance Optimization

✅ **Good:** Group related features to minimize queries
```yaml
feature_groups:
  - group: ip_associations
    # Single query for all IP-related associations
    features:
      - devices_per_ip_5h
      - users_per_ip_5h
      - transactions_per_ip_5h
```

❌ **Bad:** Scattered individual queries
```yaml
features:
  - name: feature1
    value: count_distinct(device.id, {geo.ip == event.geo.ip}, last_5h)
  
  # ... other unrelated features ...
  
  - name: feature2
    value: count_distinct(user.id, {geo.ip == event.geo.ip}, last_5h)
```

### 14.3 Time Window Selection

Choose appropriate time windows based on business scenarios:

| Scenario | Recommended Window | Reason |
|----------|-------------------|---------|
| Real-time fraud detection | last_5m, last_1h | Fast response |
| Login risk | last_5h, last_24h | Balance performance and accuracy |
| Transaction risk | last_24h, last_7d | Capture short-term patterns |
| User profiling | last_30d, last_90d | Stable long-term features |
| Device association | last_5h, last_24h | IP/device changes quickly |

---

## 15. Summary

This document defines CORINT RDL's feature engineering and statistical analysis capabilities:

- **Statistical Aggregations** - count, sum, avg, min, max, median, stddev, percentile
- **Distinct Count** - Unique value counting for association analysis
- **Association Analysis** - Relationships between IP/Device/User entities
- **Temporal Features** - Time-related features and velocity features
- **Feature Groups** - Organize and manage related features
- **Performance Optimization** - Caching, batch queries, materialized views
- **Integration** - Integration with Rules and Pipelines

These capabilities enable CORINT to support complex risk control scenarios and meet real-time decision-making requirements.

---

## 16. Related Documentation

- `expression.md` - Basic expression language and simple aggregation functions
- `context.md` - Context and variable management
- `rule.md` - Rule definition and feature usage
- `pipeline.md` - Feature extraction steps in Pipeline
- `performance.md` - Performance optimization best practices
