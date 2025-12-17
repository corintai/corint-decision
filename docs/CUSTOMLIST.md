# Custom List Feature Design

## Overview

Custom lists (blocklists, allowlists, watchlists) are essential for fraud detection and risk management. This document outlines the design for implementing list-based lookups in the Corint DSL and runtime engine.

## Use Cases

1. **Blocklist** - Block known fraudulent users, IPs, devices, emails
2. **Allowlist** - Skip checks for trusted users, partners, internal IPs
3. **Watchlist** - Flag for manual review (sanctions, PEP lists)
4. **Greylist** - Temporary restrictions (rate limiting, cooling-off)

## DSL Syntax Design

### Recommended: `list.xxx` Prefix Syntax

Use `list.<list_id>` as a reference to a configured list, where `<list_id>` is defined in the lists configuration.

```yaml
# Lists are defined in repository/configs/lists/*.yaml (each file can contain multiple lists)
# Then referenced in rules using list.<list_id> syntax

rules:
  # Check if email is in blocklist
  - id: blocked_email_check
    name: Blocked Email Check
    when:
      all:
        - user.email in list.email_blocklist
    score: 500

  # Check if user is NOT in trusted list
  - id: untrusted_user_check
    name: Untrusted User Check
    when:
      all:
        - user.id not in list.trusted_users
        - event.transaction.amount > 10000
    score: 100

  # Multiple list checks with OR logic
  - id: sanctions_check
    name: Sanctions Watchlist
    when:
      any:
        - user.name in list.ofac_sanctions
        - user.country in list.high_risk_countries
    score: 200

  # Combine list checks with other conditions
  - id: suspicious_new_user
    name: Suspicious New User
    when:
      all:
        - user.ip in list.suspicious_ips
        - user.account_age_days < 30
        - user.email not in list.verified_emails
    score: 150
```

### Syntax Rules

1. **Format**: `<field> in list.<list_id>` or `<field> not in list.<list_id>`
2. **List ID**: Must match a list `id` defined in `repository/configs/lists/*.yaml`
3. **Field**: Any valid field path (e.g., `user.email`, `event.ip`, `device.fingerprint`)
4. **Operators**: `in` (membership) and `not in` (non-membership)

### Example Use Cases

```yaml
rules:
  # IP-based blocking
  - id: blocked_ip
    when:
      all:
        - event.ip in list.ip_blocklist
    score: 500

  # VIP user bypass
  - id: vip_bypass
    when:
      all:
        - user.id in list.vip_users
    score: -100

  # Device fingerprint check
  - id: known_fraud_device
    when:
      all:
        - device.fingerprint in list.fraud_devices
    score: 300

  # Email domain check
  - id: disposable_email
    when:
      all:
        - user.email_domain in list.disposable_domains
    score: 50

  # BIN (card prefix) check
  - id: high_risk_bin
    when:
      all:
        - payment.card_bin in list.high_risk_bins
    score: 75
```

## List Configuration

Lists are defined in YAML files under `repository/configs/lists/`. Each file can contain multiple list definitions.

```
repository/
└── configs/
    └── lists/
        ├── blocklist.yaml      # Contains multiple blocklist definitions
        ├── allowlist.yaml      # Contains multiple allowlist definitions
        ├── watchlist.yaml      # Contains watchlist definitions
        └── custom.yaml         # Custom lists
```

### Example: Multiple Lists in One File

```yaml
# repository/configs/lists/blocklist.yaml
lists:
  # 默认使用 list_entries 表 (推荐)
  - id: email_blocklist
    description: "Blocked email addresses"
    backend: postgresql
    # 不指定 table，默认使用 list_entries 表
    # 通过 API 管理: POST /v1/lists/email_blocklist/entries

  # Redis 后端 (高性能场景)
  - id: ip_blocklist
    description: "Blocked IP addresses"
    backend: redis
    redis_key: "lists:ip_blocklist"
    cache_ttl: 60

  # 自定义表 (已有数据表)
  - id: fraud_devices
    description: "Known fraud device fingerprints"
    backend: postgresql
    table: fraud_devices       # 自定义表名
    value_column: fingerprint  # 值列名
    match_type: exact          # exact | prefix | regex
```

```yaml
# repository/configs/lists/allowlist.yaml
lists:
  # 默认使用 list_entries 表 (推荐)
  - id: trusted_users
    description: "Verified trusted users"
    backend: postgresql
    # 不指定 table，默认使用 list_entries 表
    # 通过 API 管理条目: POST /v1/lists/trusted_users/entries

  # 自定义表 (适用于已有数据表的场景)
  - id: vip_users
    description: "VIP user IDs"
    backend: postgresql
    table: vip_users           # 指定自定义表名
    value_column: user_id      # 指定值列
    expiration_column: expires_at  # 可选：过期时间列

  - id: verified_emails
    description: "Verified email addresses"
    backend: redis
    redis_key: "lists:verified_emails"
```

```yaml
# repository/configs/lists/watchlist.yaml
lists:
  - id: high_risk_countries
    description: "High risk country codes"
    backend: file
    path: "repository/configs/lists/data/high_risk_countries.txt"
    reload_interval: 3600

  - id: suspicious_ips
    description: "Suspicious IP addresses"
    backend: memory
    initial_values:
      - "192.168.1.100"
      - "10.0.0.50"

  - id: ofac_sanctions
    description: "OFAC sanctions list"
    backend: api
    url: "https://api.sanctions.io/check"
    method: POST
    cache_ttl: 86400
    timeout_ms: 5000
    fallback: allow
```

### Single List File (Also Supported)

```yaml
# repository/configs/lists/disposable_domains.yaml
# Single list without the 'lists:' wrapper
id: disposable_domains
description: "Disposable email domains"
backend: file
path: "repository/configs/lists/data/disposable_domains.txt"
```

## Architecture Design

### Component Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Pipeline YAML                              │
│  rules:                                                              │
│    - when: user.email in list.email_blocklist                       │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Parser (corint-parser)                       │
│  - Recognizes list.xxx as ListReference expression                  │
│  - Parses "in list.xxx" as InList operator                          │
│  - Creates Expression::Binary with Operator::InList                 │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Compiler (corint-compiler)                     │
│  - Compiles InList expression to IR::ListLookup instruction         │
│  - Validates list names exist in configuration                       │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Runtime (corint-runtime)                      │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐             │
│  │ ListService │───▶│ ListBackend │───▶│   Cache     │             │
│  └─────────────┘    └─────────────┘    └─────────────┘             │
│         │                  │                                         │
│         ▼                  ▼                                         │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐             │
│  │   Redis     │    │ PostgreSQL  │    │    File     │             │
│  └─────────────┘    └─────────────┘    └─────────────┘             │
└─────────────────────────────────────────────────────────────────────┘
```

### AST Changes

```rust
// crates/corint-core/src/ast/expression.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expression {
    // ... existing variants ...

    /// List reference: list.email_blocklist
    ListReference {
        list_id: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operator {
    // ... existing operators ...

    /// "in list.xxx" - Check if value exists in list
    InList,

    /// "not in list.xxx" - Check if value does NOT exist in list
    NotInList,
}
```

### New IR Instruction

```rust
// crates/corint-core/src/ir/instruction.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    // ... existing instructions ...

    /// List membership lookup
    /// Pops value from stack, checks if it exists in the named list
    /// Pushes boolean result onto stack
    ListLookup {
        list_id: String,
        /// Negate the result (for "not in list")
        negate: bool,
    },
}
```

### List Service Interface

```rust
// crates/corint-runtime/src/list/mod.rs

use async_trait::async_trait;
use std::collections::HashSet;

/// Result of a list lookup
#[derive(Debug, Clone)]
pub struct ListLookupResult {
    pub found: bool,
    pub list_id: String,
    pub matched_value: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// List service - manages all list backends
pub struct ListService {
    backends: HashMap<String, Arc<dyn ListBackend>>,
    cache: Option<ListCache>,
    config: ListConfig,
}

impl ListService {
    /// Check if a value exists in a list
    pub async fn contains(&self, list_id: &str, value: &str) -> Result<ListLookupResult> {
        // 1. Check local cache
        if let Some(cache) = &self.cache {
            if let Some(result) = cache.get(list_id, value).await {
                return Ok(result);
            }
        }

        // 2. Get backend for this list
        let backend = self.backends.get(list_id)
            .ok_or_else(|| ListError::UnknownList(list_id.to_string()))?;

        // 3. Query backend
        let result = backend.contains(list_id, value).await?;

        // 4. Update cache
        if let Some(cache) = &self.cache {
            cache.set(list_id, value, &result).await;
        }

        Ok(result)
    }
}

/// List backend trait - abstracts different storage backends
#[async_trait]
pub trait ListBackend: Send + Sync {
    /// Check if a single value exists in a list
    async fn contains(&self, list_id: &str, value: &str) -> Result<ListLookupResult>;

    /// Batch check multiple values (optimization)
    async fn contains_batch(&self, list_id: &str, values: &[&str]) -> Result<Vec<ListLookupResult>>;

    /// Add a value to a list
    async fn add(&self, list_id: &str, value: &str, metadata: Option<serde_json::Value>) -> Result<()>;

    /// Remove a value from a list
    async fn remove(&self, list_id: &str, value: &str) -> Result<()>;

    /// Get list info
    async fn info(&self, list_id: &str) -> Result<ListInfo>;
}

#[derive(Debug, Clone)]
pub struct ListInfo {
    pub id: String,
    pub description: Option<String>,
    pub size: usize,
    pub backend: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
```

## Parser Implementation

```rust
// crates/corint-parser/src/expression_parser.rs

impl ExpressionParser {
    /// Parse a primary expression
    fn parse_primary(&mut self) -> Result<Expression> {
        // ... existing code ...

        // Check for list.xxx reference
        if self.current_token_is("list") {
            self.advance(); // consume "list"
            self.expect(".")?;
            let list_id = self.parse_identifier()?;
            return Ok(Expression::ListReference { list_id });
        }

        // ... rest of existing code ...
    }

    /// Parse binary expression - handle "in list.xxx"
    fn parse_comparison(&mut self) -> Result<Expression> {
        let left = self.parse_additive()?;

        // Check for "in" or "not in"
        if self.current_token_is("in") {
            self.advance();
            let right = self.parse_primary()?;

            // Check if right side is a list reference
            if let Expression::ListReference { list_id } = right {
                return Ok(Expression::Binary {
                    left: Box::new(left),
                    op: Operator::InList,
                    right: Box::new(Expression::ListReference { list_id }),
                });
            } else {
                // Regular "in" for arrays
                return Ok(Expression::Binary {
                    left: Box::new(left),
                    op: Operator::In,
                    right: Box::new(right),
                });
            }
        }

        if self.current_token_is("not") && self.peek_token_is("in") {
            self.advance(); // consume "not"
            self.advance(); // consume "in"
            let right = self.parse_primary()?;

            if let Expression::ListReference { list_id } = right {
                return Ok(Expression::Binary {
                    left: Box::new(left),
                    op: Operator::NotInList,
                    right: Box::new(Expression::ListReference { list_id }),
                });
            } else {
                return Ok(Expression::Binary {
                    left: Box::new(left),
                    op: Operator::NotIn,
                    right: Box::new(right),
                });
            }
        }

        // ... rest of comparison operators ...
        Ok(left)
    }
}
```

## Compiler Implementation

```rust
// crates/corint-compiler/src/codegen/expression_codegen.rs

impl ExpressionCompiler {
    pub fn compile(expr: &Expression) -> Result<Vec<Instruction>> {
        match expr {
            // ... existing cases ...

            Expression::Binary { left, op, right } => {
                match op {
                    Operator::InList => Self::compile_list_lookup(left, right, false),
                    Operator::NotInList => Self::compile_list_lookup(left, right, true),
                    _ => Self::compile_binary(left, op, right),
                }
            }

            Expression::ListReference { list_id } => {
                // List reference alone is an error - must be used with "in"
                Err(CompileError::InvalidExpression(
                    format!("list.{} must be used with 'in' or 'not in' operator", list_id)
                ))
            }
        }
    }

    fn compile_list_lookup(
        value_expr: &Expression,
        list_expr: &Expression,
        negate: bool,
    ) -> Result<Vec<Instruction>> {
        let mut instructions = Vec::new();

        // Compile the value expression (pushes value onto stack)
        instructions.extend(Self::compile(value_expr)?);

        // Get the list ID
        let list_id = match list_expr {
            Expression::ListReference { list_id } => list_id.clone(),
            _ => return Err(CompileError::InvalidExpression(
                "'in list.xxx' requires a list reference on the right side".to_string()
            )),
        };

        // Add the list lookup instruction
        instructions.push(Instruction::ListLookup {
            list_id,
            negate,
        });

        Ok(instructions)
    }
}
```

## VM Execution

```rust
// crates/corint-runtime/src/engine/pipeline_executor.rs

impl PipelineExecutor {
    async fn execute_instruction(
        &self,
        instruction: &Instruction,
        ctx: &mut ExecutionContext,
    ) -> Result<()> {
        match instruction {
            // ... existing cases ...

            Instruction::ListLookup { list_id, negate } => {
                // Pop the value to check
                let value = ctx.pop()?;
                let value_str = Self::value_to_string(&value);

                // Perform the lookup
                let result = self.list_service
                    .contains(list_id, &value_str)
                    .await
                    .map_err(|e| RuntimeError::ListLookupFailed {
                        list_id: list_id.clone(),
                        error: e.to_string(),
                    })?;

                // Apply negation if needed
                let found = if *negate { !result.found } else { result.found };

                // Push result onto stack
                ctx.push(Value::Bool(found));

                // Record for tracing
                if result.found {
                    ctx.trace_list_match(list_id, &value_str, result.metadata);
                }

                Ok(())
            }
        }
    }

    fn value_to_string(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "".to_string(),
            _ => serde_json::to_string(value).unwrap_or_default(),
        }
    }
}
```

## Database Schema

```sql
-- Lists metadata table
CREATE TABLE lists (
    id VARCHAR(255) PRIMARY KEY,  -- matches list_id in config
    description TEXT,
    backend VARCHAR(50) NOT NULL,  -- redis, postgresql, file, api, memory
    config JSONB,  -- backend-specific configuration
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    is_active BOOLEAN DEFAULT TRUE
);

-- List entries table (for PostgreSQL-backed lists)
CREATE TABLE list_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id VARCHAR(255) REFERENCES lists(id) ON DELETE CASCADE,
    value VARCHAR(1024) NOT NULL,
    value_type VARCHAR(50) DEFAULT 'exact',  -- exact, prefix, regex, cidr
    metadata JSONB,
    added_at TIMESTAMPTZ DEFAULT NOW(),
    added_by VARCHAR(255),
    expires_at TIMESTAMPTZ,  -- NULL = never expires
    reason TEXT,
    UNIQUE(list_id, value)
);

-- Indexes
CREATE INDEX idx_list_entries_lookup ON list_entries(list_id, value) WHERE expires_at IS NULL OR expires_at > NOW();
CREATE INDEX idx_list_entries_expires ON list_entries(expires_at) WHERE expires_at IS NOT NULL;

-- Audit log
CREATE TABLE list_audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    list_id VARCHAR(255) REFERENCES lists(id),
    action VARCHAR(50) NOT NULL,  -- add, remove, update, bulk_import
    value VARCHAR(1024),
    performed_by VARCHAR(255),
    performed_at TIMESTAMPTZ DEFAULT NOW(),
    details JSONB
);

-- Insert default lists
INSERT INTO lists (id, description, backend) VALUES
('email_blocklist', 'Blocked email addresses', 'postgresql'),
('trusted_users', 'Trusted user IDs', 'postgresql'),
('high_risk_countries', 'High risk country codes', 'file'),
('suspicious_ips', 'Suspicious IP addresses', 'redis');
```

## API Endpoints

```
# List Management
GET    /v1/lists                         # Get all configured lists
GET    /v1/lists/{list_id}               # Get list details and stats
POST   /v1/lists/{list_id}/check         # Check if value is in list

# List Entries (for manageable lists)
GET    /v1/lists/{list_id}/entries       # Get entries (paginated)
POST   /v1/lists/{list_id}/entries       # Add entry
DELETE /v1/lists/{list_id}/entries/{id}  # Remove entry
POST   /v1/lists/{list_id}/import        # Bulk import
GET    /v1/lists/{list_id}/export        # Export all entries
```

### API Examples

```bash
# Check if email is in blocklist
curl -X POST http://localhost:8080/v1/lists/email_blocklist/check \
  -H "Content-Type: application/json" \
  -d '{"value": "test@example.com"}'

# Response
{
  "found": true,
  "list_id": "email_blocklist",
  "matched_value": "test@example.com",
  "metadata": {
    "reason": "Confirmed fraud",
    "added_at": "2024-01-15T10:30:00Z"
  }
}

# Add entry to blocklist
curl -X POST http://localhost:8080/v1/lists/email_blocklist/entries \
  -H "Content-Type: application/json" \
  -d '{
    "value": "fraud@example.com",
    "reason": "Confirmed fraud account",
    "expires_at": "2025-12-31T23:59:59Z"
  }'

# Bulk import
curl -X POST http://localhost:8080/v1/lists/suspicious_ips/import \
  -H "Content-Type: application/json" \
  -d '{
    "entries": [
      {"value": "192.168.1.100", "reason": "Bot traffic"},
      {"value": "10.0.0.50", "reason": "Suspicious activity"}
    ]
  }'
```

## Implementation Plan

### Phase 1: Core (MVP)

1. **AST & Parser**
   - Add `ListReference` expression variant
   - Add `InList`/`NotInList` operators
   - Parse `list.xxx` syntax

2. **IR & Compiler**
   - Add `ListLookup` instruction
   - Compile list lookups

3. **Runtime**
   - Create `ListService` with `MemoryBackend`
   - Execute `ListLookup` instruction

4. **Config**
   - Load lists from `repository/configs/lists/*.yaml`

### Phase 2: Backends

1. **Redis Backend** - For high-performance lookups
2. **PostgreSQL Backend** - With metadata and expiration
3. **File Backend** - For static lists
4. **Caching Layer** - Local cache with TTL

### Phase 3: Management

1. **REST API** - List management endpoints
2. **Audit Logging** - Track changes
3. **Bulk Operations** - Import/export

### Phase 4: Advanced

1. **Pattern Matching** - Regex, CIDR, prefix
2. **Analytics** - Hit rates, usage stats
3. **External APIs** - Third-party list providers

## Performance

| Operation | Latency (P99) |
|-----------|---------------|
| Memory lookup | <0.1ms |
| Local cache hit | <1ms |
| Redis lookup | 1-5ms |
| PostgreSQL lookup | 5-20ms |

## Example Pipeline

```yaml
# pipeline.yaml
pipeline:
  id: fraud_detection
  name: Fraud Detection with Lists

steps:
  - ruleset:
      id: list_checks
      name: List-based Checks
      rules:
        # Immediate block for blocklisted emails
        - id: email_blocklist
          name: Email Blocklist
          when:
            all:
              - user.email in list.email_blocklist
          score: 500

        # Skip checks for trusted users
        - id: trusted_user
          name: Trusted User Bypass
          when:
            all:
              - user.id in list.trusted_users
          score: -200

        # Flag high-risk countries
        - id: high_risk_country
          name: High Risk Country
          when:
            all:
              - user.country in list.high_risk_countries
              - user.id not in list.trusted_users
          score: 100

        # Suspicious IP check
        - id: suspicious_ip
          name: Suspicious IP
          when:
            all:
              - event.ip in list.suspicious_ips
          score: 75

# repository/configs/lists/fraud_lists.yaml
lists:
  - id: email_blocklist
    description: "Blocked emails"
    backend: postgresql

  - id: trusted_users
    description: "Trusted user IDs"
    backend: postgresql

  - id: high_risk_countries
    description: "High risk countries"
    backend: file
    path: "repository/configs/lists/data/high_risk_countries.txt"

  - id: suspicious_ips
    description: "Suspicious IPs"
    backend: redis
    cache_ttl: 60
```
