# corint-repository

Repository abstraction layer for the CORINT Decision Engine.

This crate provides a unified interface for loading rules, rulesets, templates, and pipelines from different storage backends (file system, database, etc.).

## Features

- **File System Repository**: Load from YAML files on disk
- **PostgreSQL Repository**: Store artifacts in PostgreSQL database with versioning
- **Caching**: Built-in caching with TTL support for performance
- **Async API**: Non-blocking I/O operations using Tokio
- **ID-based lookup**: Load artifacts by ID or file path
- **Write Operations**: Save, update, and delete artifacts (PostgreSQL only)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
corint-repository = { path = "crates/corint-repository" }

# For database support
corint-repository = { path = "crates/corint-repository", features = ["postgres"] }
```

## Usage

### Basic File System Repository

```rust
use corint_repository::{Repository, FileSystemRepository};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create repository pointing to the repository directory
    let repo = FileSystemRepository::new("repository")?;

    // Load a rule by path
    let (rule, content) = repo
        .load_rule("library/rules/fraud/fraud_farm.yaml")
        .await?;
    println!("Loaded rule: {} (score: {})", rule.id, rule.score);

    // Load a rule by ID (searches standard locations)
    let (rule, _) = repo.load_rule("fraud_farm_pattern").await?;
    println!("Found rule by ID: {}", rule.id);

    // Load a ruleset
    let (ruleset, _) = repo
        .load_ruleset("library/rulesets/fraud_detection_core.yaml")
        .await?;
    println!("Loaded ruleset: {} with {} rules",
        ruleset.id, ruleset.rules.len());

    // Load a decision logic template
    let (template, _) = repo
        .load_template("library/templates/score_based_decision.yaml")
        .await?;
    println!("Loaded template: {}", template.id);

    Ok(())
}
```

### PostgreSQL Repository

```rust
use corint_repository::{Repository, PostgresRepository, WritableRepository};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Connect to PostgreSQL database
    let database_url = "postgresql://user:password@localhost/corint";
    let mut repo = PostgresRepository::new(database_url).await?;

    // Save a rule to database
    repo.save_rule(&rule).await?;

    // Load rule from database (uses cache automatically)
    let (loaded_rule, content) = repo.load_rule("fraud_farm_pattern").await?;
    println!("Loaded rule: {}", loaded_rule.id);

    // Update a rule (increments version automatically)
    rule.score = 75;
    repo.save_rule(&rule).await?;

    // Delete a rule
    repo.delete_rule("fraud_farm_pattern").await?;

    // List all rules in database
    let rules = repo.list_rules().await?;
    for rule_id in rules {
        println!("Rule: {}", rule_id);
    }

    Ok(())
}
```

### Database Migrations

Before using the PostgreSQL repository, run the database migrations:

```bash
# Connect to your database
export DATABASE_URL="postgresql://user:password@localhost/corint"

# Run migrations in order
psql $DATABASE_URL < docs/schema/001_create_rules_table.sql
psql $DATABASE_URL < docs/schema/002_create_rulesets_table.sql
psql $DATABASE_URL < docs/schema/003_create_templates_table.sql
psql $DATABASE_URL < docs/schema/004_create_pipelines_table.sql
psql $DATABASE_URL < docs/schema/005_create_audit_log.sql
```

See the [docs/schema/README.md](../../docs/schema/README.md) for detailed schema documentation.

### Custom Cache Configuration

```rust
use corint_repository::{FileSystemRepository, CacheConfig};
use std::time::Duration;

let config = CacheConfig::new()
    .with_ttl(Duration::from_secs(600)) // 10 minutes
    .with_max_entries(500);

let repo = FileSystemRepository::with_cache_config("repository", config)?;
```

### Disable Caching

```rust
let config = CacheConfig::disabled();
let repo = FileSystemRepository::with_cache_config("repository", config)?;
```

### Cache Management

```rust
use corint_repository::CacheableRepository;

let mut repo = FileSystemRepository::new("repository")?;

// Get cache statistics
let stats = repo.cache_stats();
println!("Cache hit rate: {:.2}%", stats.hit_rate() * 100.0);
println!("Cache size: {} entries", stats.size);

// Clear cache
repo.clear_cache();

// Clear specific entry
repo.clear_cache_entry("library/rules/fraud/fraud_farm.yaml");

// Enable/disable caching at runtime
repo.set_cache_enabled(false);
```

### List Available Artifacts

```rust
// List all rules
let rules = repo.list_rules().await?;
for rule_path in rules {
    println!("Rule: {}", rule_path);
}

// List all rulesets
let rulesets = repo.list_rulesets().await?;
for ruleset_path in rulesets {
    println!("Ruleset: {}", ruleset_path);
}

// List all templates
let templates = repo.list_templates().await?;
for template_path in templates {
    println!("Template: {}", template_path);
}
```

### Check If Artifact Exists

```rust
if repo.exists("library/rules/fraud/fraud_farm.yaml").await? {
    println!("Rule exists!");
}
```

## Repository Directory Structure

The file system repository expects the following structure:

```
repository/
├── library/
│   ├── rules/          # Individual rule definitions
│   │   ├── fraud/
│   │   ├── payment/
│   │   └── geography/
│   ├── rulesets/       # Reusable ruleset definitions
│   └── templates/      # Decision logic templates
└── pipelines/          # Business scenario orchestration
```

## Architecture

### Overview

```text
┌────────────────────────────────────────┐
│        Application Layer               │
│  (Compiler, Runtime, Server)           │
└──────────────┬─────────────────────────┘
               │ Repository Trait
               ↓
┌────────────────────────────────────────┐
│    Repository Abstraction Layer        │
│  - Repository (read operations)        │
│  - CacheableRepository (cache mgmt)    │
│  - WritableRepository (CRUD)           │
└──────────────┬─────────────────────────┘
               │
      ┌────────┴────────┐
      ↓                 ↓
┌──────────────┐  ┌──────────────────┐
│ FileSystem   │  │  PostgreSQL      │
│ Repository   │  │  Repository      │
│ - YAML files │  │  - Versioning    │
│ - Caching    │  │  - Audit log     │
└──────────────┘  └──────────────────┘
```

### Core Traits

#### 1. `Repository` Trait

Core read-only interface for loading decision artifacts.

```rust
#[async_trait]
pub trait Repository: Send + Sync {
    /// Load a rule by path or ID
    async fn load_rule(&self, identifier: &str) -> RepositoryResult<(Rule, String)>;

    /// Load a ruleset by path or ID
    async fn load_ruleset(&self, identifier: &str) -> RepositoryResult<(Ruleset, String)>;

    /// Load a decision logic template by path or ID
    async fn load_template(&self, identifier: &str) -> RepositoryResult<(DecisionTemplate, String)>;

    /// Load a pipeline by path or ID
    async fn load_pipeline(&self, identifier: &str) -> RepositoryResult<(Pipeline, String)>;

    /// List all available rules
    async fn list_rules(&self) -> RepositoryResult<Vec<String>>;

    /// List all available rulesets
    async fn list_rulesets(&self) -> RepositoryResult<Vec<String>>;

    /// List all available templates
    async fn list_templates(&self) -> RepositoryResult<Vec<String>>;

    /// List all available pipelines
    async fn list_pipelines(&self) -> RepositoryResult<Vec<String>>;

    /// Check if an artifact exists
    async fn exists(&self, identifier: &str) -> RepositoryResult<bool>;
}
```

#### 2. `CacheableRepository` Trait

Extension for repositories that support caching.

```rust
pub trait CacheableRepository: Repository {
    /// Clear all caches
    fn clear_cache(&mut self);

    /// Clear cache entry for specific identifier
    fn clear_cache_entry(&mut self, identifier: &str);

    /// Get cache statistics
    fn cache_stats(&self) -> CacheStats;

    /// Set cache enabled/disabled
    fn set_cache_enabled(&mut self, enabled: bool);

    /// Check if caching is enabled
    fn is_cache_enabled(&self) -> bool;
}
```

#### 3. `WritableRepository` Trait

Extension for repositories that support write operations. Only implemented by database backends.

```rust
#[async_trait]
pub trait WritableRepository: Repository {
    /// Save or update a rule
    async fn save_rule(&mut self, rule: &Rule) -> RepositoryResult<()>;

    /// Save or update a ruleset
    async fn save_ruleset(&mut self, ruleset: &Ruleset) -> RepositoryResult<()>;

    /// Save or update a template
    async fn save_template(&mut self, template: &DecisionTemplate) -> RepositoryResult<()>;

    /// Save or update a pipeline
    async fn save_pipeline(&mut self, pipeline: &Pipeline) -> RepositoryResult<()>;

    /// Delete a rule
    async fn delete_rule(&self, identifier: &str) -> RepositoryResult<()>;

    /// Delete a ruleset
    async fn delete_ruleset(&self, identifier: &str) -> RepositoryResult<()>;

    /// Delete a template
    async fn delete_template(&self, identifier: &str) -> RepositoryResult<()>;

    /// Delete a pipeline
    async fn delete_pipeline(&self, identifier: &str) -> RepositoryResult<()>;
}
```

### Implementations

#### 1. FileSystemRepository

Loads artifacts from YAML files on disk.

**Features:**
- Read-only operations
- Implements `Repository` and `CacheableRepository`
- ID-based lookup with automatic directory searching
- Built-in TTL-based caching
- Async I/O with Tokio (non-blocking)
- Ideal for development and read-only production deployments

**✅ Best Use Cases:**
- Local development and prototyping
- CI/CD pipelines
- Rules checked into Git and deployed with application
- Read-only production deployments
- Git-based version control of rules
- Offline testing and validation
- Disaster recovery via Git

**❌ Not Suitable For:**
- Dynamic rule updates in production
- Multi-tenant scenarios requiring isolation
- Audit logging requirements
- Version history tracking

**Limitations:**
- Cannot update rules at runtime
- No version tracking
- No audit logging

#### 2. PostgresRepository

Stores artifacts in PostgreSQL database with advanced features.

**Features:**
- Full read-write operations
- Implements all three traits: `Repository`, `CacheableRepository`, `WritableRepository`
- Automatic version tracking (increment on each update)
- Optional audit logging via database triggers
- Connection pooling for performance
- JSONB storage for metadata and parameters
- Foreign key constraints for referential integrity
- PostgreSQL 12+ required
- Ideal for production deployments requiring runtime updates

**✅ Best Use Cases:**
- Production environments requiring runtime rule updates
- Multi-service deployments sharing the same rules
- Environments requiring audit trails and compliance
- Version history tracking and rollback capabilities
- Multi-tenant scenarios with centralized rule management
- A/B testing with rule variations
- Dynamic rule management via API or admin interface

**❌ Not Suitable For:**
- Simple read-only scenarios (use file system instead)
- Offline/air-gapped environments
- Git-based workflows (use file system instead)

**Advanced Features:**
- **Versioning**: Each `save_*()` operation increments the version number
- **Audit Log**: Optional triggers capture all changes (who, what, when)
- **JSONB Queries**: Efficient metadata and parameter queries
- **Connection Pooling**: Automatic via sqlx for high concurrency
- **Transactions**: All operations support PostgreSQL transactions

### ID-based Lookup

The repository supports loading artifacts by either:

1. **Relative path**: `library/rules/fraud/fraud_farm.yaml`
2. **Artifact ID**: `fraud_farm_pattern`

When loading by ID, the repository automatically searches in standard locations:
- Rules: `library/rules/**/*.yaml`
- Rulesets: `library/rulesets/**/*.yaml`
- Templates: `library/templates/**/*.yaml`
- Pipelines: `pipelines/**/*.yaml`

### Caching Strategy

Both FileSystemRepository and PostgresRepository include built-in caching:

**Cache Features:**
- **TTL (Time-To-Live)**: Configurable expiration (default: 5 minutes)
- **Hit/Miss Tracking**: Performance statistics and hit rate calculation
- **Max Entries**: Limit cache size to prevent memory issues
- **Per-artifact Type**: Separate caches for rules, rulesets, templates, pipelines
- **Thread-Safe**: Uses `Arc<RwLock<>>` for concurrent access

**Cache Behavior:**
- First load: Cache miss → Loads from storage → Stores in cache
- Subsequent loads: Cache hit → Returns from cache (if not expired)
- Cache invalidation: Manual via `clear_cache()` or automatic on TTL expiration

**Cache Statistics:**
```rust
pub struct CacheStats {
    pub hits: usize,        // Number of cache hits
    pub misses: usize,      // Number of cache misses
    pub size: usize,        // Current number of cached entries
    pub max_entries: usize, // Maximum cache size
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}
```

**Performance Impact:**
- Typical cache hit: **<1ms** (in-memory lookup)
- Typical cache miss: **5-50ms** (file I/O or database query)
- Recommended for production: Enable caching with 5-10 minute TTL

## Choosing a Backend

### Decision Matrix

**Choose FileSystemRepository if:**
- ✅ Rules are part of your application deployment (checked into Git)
- ✅ Rules rarely change after deployment
- ✅ Development or testing environment
- ✅ Read-only production deployment
- ✅ Want simplicity and minimal dependencies

**Choose PostgresRepository if:**
- ✅ Need to update rules at runtime without redeployment
- ✅ Multiple services/instances sharing the same rules
- ✅ Require audit trails for compliance
- ✅ Need version tracking and rollback capabilities
- ✅ Managing rules via API or admin interface
- ✅ A/B testing with rule variations

### Hybrid Approach

You can use both backends in a hybrid setup:

```rust
// Primary: Database for writable rules
let mut primary_repo = PostgresRepository::new(database_url).await?;

// Fallback: File system for built-in rules
let fallback_repo = FileSystemRepository::new("repository")?;

// Try database first, fall back to file system
let (rule, content) = match primary_repo.load_rule(rule_id).await {
    Ok(result) => result,
    Err(_) => fallback_repo.load_rule(rule_id).await?,
};
```

## Cargo Features

### `postgres` (optional)

Enables PostgreSQL database backend support with the following capabilities:

- **Connection Pooling**: Built-in connection pool via sqlx (default: 10 connections)
- **Automatic Versioning**: Each update increments the version number
- **Audit Logging**: Optional triggers to track all changes (see docs/schema/005_create_audit_log.sql)
- **JSONB Storage**: Efficient storage of metadata and parameters
- **Full CRUD Operations**: Create, read, update, and delete artifacts
- **Foreign Keys**: Referential integrity for `extends` relationships

To enable:

```toml
[dependencies]
corint-repository = { path = "crates/corint-repository", features = ["postgres"] }
```

### Database Schema

The PostgreSQL backend includes five tables:

#### 1. `rules` Table
```sql
CREATE TABLE rules (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),              -- Optional file path reference
    content TEXT NOT NULL,          -- Full YAML content
    version INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    metadata JSONB,                 -- Flexible metadata storage
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),

    INDEX idx_rules_path (path),
    INDEX idx_rules_updated_at (updated_at)
);
```

#### 2. `rulesets` Table
```sql
CREATE TABLE rulesets (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    extends VARCHAR(255),           -- Parent ruleset ID (Phase 3)
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),

    FOREIGN KEY (extends) REFERENCES rulesets(id) ON DELETE SET NULL,
    INDEX idx_rulesets_path (path),
    INDEX idx_rulesets_extends (extends),
    INDEX idx_rulesets_updated_at (updated_at)
);
```

#### 3. `templates` Table
```sql
CREATE TABLE templates (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    params JSONB,                   -- Default parameters (Phase 3)
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),

    INDEX idx_templates_path (path),
    INDEX idx_templates_updated_at (updated_at)
);
```

#### 4. `pipelines` Table
```sql
CREATE TABLE pipelines (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),

    INDEX idx_pipelines_path (path),
    INDEX idx_pipelines_updated_at (updated_at)
);
```

#### 5. `artifact_audit_log` Table (Optional)
```sql
CREATE TABLE artifact_audit_log (
    id SERIAL PRIMARY KEY,
    artifact_type VARCHAR(50) NOT NULL,  -- 'rule', 'ruleset', 'template', 'pipeline'
    artifact_id VARCHAR(255) NOT NULL,
    operation VARCHAR(50) NOT NULL,      -- 'create', 'update', 'delete'
    old_content TEXT,                    -- Previous version
    new_content TEXT,                    -- New version
    changed_by VARCHAR(255),
    changed_at TIMESTAMP NOT NULL DEFAULT NOW(),

    INDEX idx_audit_artifact (artifact_type, artifact_id),
    INDEX idx_audit_changed_at (changed_at)
);
```

### Automatic Versioning

PostgreSQL repository automatically tracks versions:

```rust
// First save: version = 1
repo.save_rule(&rule).await?;

// Update: version = 2 (automatic increment)
rule.score = 100;
repo.save_rule(&rule).await?;

// Query version history (requires custom query)
let versions = sqlx::query!(
    "SELECT version, updated_at FROM rules WHERE id = $1 ORDER BY version",
    rule_id
).fetch_all(&pool).await?;
```

## Integration with Compiler

The repository abstraction integrates seamlessly with the CORINT compiler:

```rust
use corint_compiler::{Compiler, ImportResolver};
use corint_repository::{Repository, FileSystemRepository, PostgresRepository};
use std::sync::Arc;

// Option 1: File system repository
let repo = Arc::new(FileSystemRepository::new("repository")?);
let mut resolver = ImportResolver::with_repository(repo);

// Option 2: PostgreSQL repository
let repo = Arc::new(PostgresRepository::new(database_url).await?);
let mut resolver = ImportResolver::with_repository(repo);

// Compiler uses repository to load dependencies
let mut compiler = Compiler::new_with_resolver(resolver);
let program = compiler.compile_pipeline_file(Path::new("pipeline.yaml")).await?;
```

The compiler automatically:
- Loads imported rules and rulesets via the repository
- Resolves inheritance chains (Phase 3 `extends`)
- Instantiates templates (Phase 3 `decision_template`)
- Validates circular dependencies
- Ensures ID uniqueness

## Testing

### File System Tests

Run all file system tests:

```bash
cargo test --package corint-repository
```

Run specific test:

```bash
cargo test --package corint-repository test_file_system_load_rule
```

### PostgreSQL Tests

PostgreSQL tests require a running database:

```bash
# Set up test database
createdb corint_test
export DATABASE_URL="postgresql://localhost/corint_test"

# Run migrations
for file in docs/schema/*.sql; do psql $DATABASE_URL < "$file"; done

# Run PostgreSQL tests (marked with #[ignore] by default)
cargo test --package corint-repository --features postgres -- --ignored

# Run specific database test
cargo test --package corint-repository --features postgres test_postgres_save_rule -- --ignored
```

### Integration Tests

Run end-to-end tests with real repository:

```bash
# Test with file system repository
cargo test --package corint-compiler integration_test_with_repository

# Test with PostgreSQL repository
export DATABASE_URL="postgresql://localhost/corint_test"
cargo test --package corint-compiler --features postgres integration_test_with_database -- --ignored
```

## Production Best Practices

### 1. Connection String Security

```rust
// ❌ Don't hardcode credentials
let repo = PostgresRepository::new("postgresql://user:pass@host/db").await?;

// ✅ Use environment variables
let database_url = env::var("DATABASE_URL")
    .expect("DATABASE_URL must be set");
let repo = PostgresRepository::new(&database_url).await?;
```

### 2. Connection Pooling

```rust
// ✅ Reuse repository instance (connection pool inside)
use once_cell::sync::Lazy;

static REPO: Lazy<PostgresRepository> = Lazy::new(|| {
    let database_url = env::var("DATABASE_URL").unwrap();
    PostgresRepository::new(&database_url)
        .await
        .expect("Failed to connect to database")
});
```

### 3. Error Handling

```rust
use corint_repository::RepositoryError;

match repo.load_rule("unknown_rule").await {
    Ok((rule, _)) => println!("Found: {}", rule.id),
    Err(RepositoryError::NotFound { path }) => {
        eprintln!("Rule not found: {}", path);
        // Fallback or default behavior
    }
    Err(e) => {
        eprintln!("Database error: {}", e);
        // Retry logic or alert
    }
}
```

### 4. Health Checks

```rust
// Check database connectivity
pub async fn health_check(repo: &PostgresRepository) -> Result<(), String> {
    repo.exists("health_check_rule")
        .await
        .map_err(|e| format!("Database health check failed: {}", e))?;
    Ok(())
}
```

### 5. Monitoring

Monitor these metrics in production:
- **Cache hit rate**: Should be > 90% for optimal performance
- **Load latency**: Track p50, p95, p99 percentiles
- **Database connection pool utilization**: Avoid pool exhaustion
- **Failed queries**: Alert on repeated failures
- **Cache memory usage**: Monitor cache size and evictions

### 6. Cache Warming

Pre-warm cache on application startup:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let repo = FileSystemRepository::new("repository")?;

    // Pre-load frequently used artifacts
    let critical_rules = ["fraud_farm_pattern", "velocity_check", "geo_mismatch"];
    for rule_id in critical_rules {
        let _ = repo.load_rule(rule_id).await?;
    }

    println!("Cache warmed up, hit rate: {:.2}%",
        repo.cache_stats().hit_rate() * 100.0);

    Ok(())
}
```

## Performance Considerations

### FileSystemRepository Performance

**Typical Latencies:**
- Load rule (cached): **<1ms**
- Load rule (uncached): **5-15ms** (depends on file size and disk I/O)
- List artifacts: **50-200ms** (depends on directory size)
- Cold start: **10-50ms** for initial loads

**Optimization Tips:**
- Enable caching with appropriate TTL (default: 5 minutes)
- Use ID-based lookup for faster searches
- Avoid frequent `list_*()` calls (they scan directories)
- Pre-warm cache on application startup
- Use SSD storage for repository directory

### PostgresRepository Performance

**Typical Latencies:**
- Load rule (cached): **<1ms**
- Load rule (uncached): **10-30ms** (includes network + query time)
- Save rule: **15-50ms** (includes transaction commit)
- List artifacts: **5-20ms** (indexed query)

**Optimization Tips:**
- Use connection pooling (default pool size: 10)
- Enable caching with 5-10 minute TTL
- Batch operations when possible
- Use database indexes for metadata queries
- Consider read replicas for high-traffic production

### Benchmarks

Run performance benchmarks:

```bash
# Benchmark file system repository
cargo bench --package corint-repository -- file_system

# Benchmark PostgreSQL repository
export DATABASE_URL="postgresql://localhost/corint_test"
cargo bench --package corint-repository --features postgres -- postgres
```

## Troubleshooting

### File System Issues

**Problem 1: Rule Not Found**
```
Error: RepositoryError::NotFound("library/rules/fraud/velocity_check.yaml")
```

**Solutions:**
1. Check file exists: `ls repository/library/rules/**/*.yaml | grep velocity`
2. Verify ID in YAML matches: `grep "id: velocity_check" file.yaml`
3. Check file path is relative to repository root
4. Check file permissions: `ls -la repository/library/rules/`

**Problem 2: Slow Initial Loads**

**Solutions:**
1. Enable caching (enabled by default)
2. Warm up cache at application startup
3. Use SSD storage for repository directory
4. Check disk I/O performance

**Problem 3: Parse Error**
```
Error: RepositoryError::ParseError("invalid YAML syntax")
```

**Solutions:**
1. Validate YAML syntax: `yamllint file.yaml`
2. Check for proper indentation (use spaces, not tabs)
3. Verify YAML structure matches expected schema
4. Check for duplicate keys in YAML

### PostgreSQL Issues

**Problem 1: Connection Failed**
```
Error: RepositoryError::DatabaseError("connection refused")
Error: Database(PoolTimedOut)
```

**Solutions:**
1. Check DATABASE_URL is correct
2. Verify PostgreSQL is running: `pg_isready -h localhost`
3. Check firewall/network settings
4. Increase connection pool size if needed
5. Verify credentials and database exists

**Problem 2: Migration Failed**
```
Error: relation "rules" already exists
```

**Solutions:**
1. Check migration status: `psql $DATABASE_URL -c "\dt"`
2. If tables exist, skip failed migration
3. Or drop tables and recreate manually
4. Run migrations in order: 001 → 005

**Problem 3: Version Conflict**
```
Error: Version mismatch (expected 5, got 6)
```

**Solutions:**
1. This indicates concurrent updates
2. Reload the artifact and retry
3. Implement optimistic locking if needed
4. Use transactions for atomic updates

**Problem 4: Cache Not Working**
```
Cache hit rate: 0.00%
```

**Solutions:**
1. Check `is_cache_enabled()` returns true
2. Verify TTL is not too short (default: 5 minutes)
3. Check if artifacts are being loaded with same identifier
4. Monitor cache statistics with `cache_stats()`

**Problem 5: Duplicate Key Violation**
```
Error: Duplicate key violation on id
```

**Solutions:**
1. Use `save_*()` for both inserts and updates
2. Check artifact doesn't already exist before saving
3. Use unique IDs for each artifact
4. Consider using upsert logic

### Debug Logging

Enable detailed logging:

```rust
env_logger::builder()
    .filter_module("corint_repository", log::LevelFilter::Debug)
    .init();
```

Or set environment variable:

```bash
export RUST_LOG=corint_repository=debug
cargo run
```

## Migration Guide

### From Monolithic Files to Repository Pattern

**Before:**
```rust
// Load everything from one file
let yaml = std::fs::read_to_string("pipeline.yaml")?;
let pipeline = PipelineParser::parse(&yaml)?;
```

**After:**
```rust
// Use repository abstraction
let repo = FileSystemRepository::new("repository")?;
let (pipeline, _) = repo.load_pipeline("pipelines/fraud_detection.yaml").await?;

// Or compile with automatic dependency resolution
let mut compiler = Compiler::new();
let program = compiler.compile_pipeline_file(Path::new("fraud_detection.yaml")).await?;
```

### From File System to PostgreSQL

**Step 1**: Set up database and run migrations

```bash
createdb corint_production
export DATABASE_URL="postgresql://localhost/corint_production"
for file in docs/schema/*.sql; do psql $DATABASE_URL < "$file"; done
```

**Step 2**: Import existing rules to database

```rust
let fs_repo = FileSystemRepository::new("repository")?;
let mut db_repo = PostgresRepository::new(database_url).await?;

// Migrate all rules
for rule_path in fs_repo.list_rules().await? {
    let (rule, _) = fs_repo.load_rule(&rule_path).await?;
    db_repo.save_rule(&rule).await?;
}

// Migrate all rulesets
for ruleset_path in fs_repo.list_rulesets().await? {
    let (ruleset, _) = fs_repo.load_ruleset(&ruleset_path).await?;
    db_repo.save_ruleset(&ruleset).await?;
}
```

**Step 3**: Update application code

```diff
- let repo = FileSystemRepository::new("repository")?;
+ let repo = PostgresRepository::new(database_url).await?;
```

## Future Enhancements

### Planned Features (Phase 5+)

- [ ] **MySQL Repository**: Support for MySQL/MariaDB backends
- [ ] **Advanced Queries**: Filter, pagination, sorting on metadata fields
- [ ] **Multi-tenancy**: Isolated artifact namespaces per tenant
- [ ] **Repository Middleware**: Logging, metrics, rate limiting
- [ ] **Automatic Migrations**: Schema version tracking and auto-migration
- [ ] **Import/Export**: Bulk import/export utilities for migration
- [ ] **Lazy Loading**: Load artifacts on-demand during execution
- [ ] **Parallel Loading**: Load multiple imports concurrently
- [ ] **Compilation Cache**: Cache compiled programs to avoid recompilation
- [ ] **Rule Versioning**: Query and rollback to previous rule versions
- [ ] **GraphQL API**: Query artifacts via GraphQL interface
- [ ] **Webhook Support**: Trigger notifications on artifact changes
- [ ] **Rule Marketplace**: Community-contributed rule library

### Experimental Features

Some features from the original Phase 4 plan that may be explored:

- **Redis Cache Backend**: Use Redis instead of in-memory cache
- **S3 Repository**: Store artifacts in AWS S3 or compatible object storage
- **Git Repository**: Load artifacts directly from Git repositories
- **HTTP Repository**: Fetch artifacts from HTTP/REST endpoints

## License

Licensed under the Elastic License 2.0 (ELv2).
