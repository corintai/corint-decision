# Database Schema

This directory contains SQL schema files for setting up the PostgreSQL database backend for CORINT Decision Engine.

## Overview

The database schema supports storing rules, rulesets, templates, and pipelines with:
- Version tracking
- Audit logging
- Inheritance support (for rulesets)
- Parameter storage (for templates)
- Full-text YAML content storage

## Schema Files

| File | Description |
|------|-------------|
| `001_create_rules_table.sql` | Creates the `rules` table for storing rule definitions |
| `002_create_rulesets_table.sql` | Creates the `rulesets` table with inheritance support (`extends` field) |
| `003_create_templates_table.sql` | Creates the `templates` table for decision logic templates |
| `004_create_pipelines_table.sql` | Creates the `pipelines` table for pipeline definitions |
| `005_create_audit_log.sql` | Creates audit logging table and triggers |

## Setup Instructions

### Option 1: Manual Execution (psql)

```bash
# Set database connection
export DATABASE_URL="postgresql://user:password@localhost:5432/corint"

# Run each schema file in order
psql $DATABASE_URL < docs/schema/001_create_rules_table.sql
psql $DATABASE_URL < docs/schema/002_create_rulesets_table.sql
psql $DATABASE_URL < docs/schema/003_create_templates_table.sql
psql $DATABASE_URL < docs/schema/004_create_pipelines_table.sql
psql $DATABASE_URL < docs/schema/005_create_audit_log.sql
```

### Option 2: All at Once

```bash
# Run all schema files in sequence
for file in docs/schema/*.sql; do
  echo "Running $file..."
  psql $DATABASE_URL < "$file"
done
```

### Option 3: Using sqlx-cli (Recommended for Development)

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Create database
sqlx database create --database-url $DATABASE_URL

# Run migrations
sqlx migrate run --database-url $DATABASE_URL --source ./migrations
```

## Database Schema

### Tables

#### `rules`
Stores individual rule definitions.

```sql
CREATE TABLE rules (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255)
);
```

**Indexes:**
- `idx_rules_path` - On `path` column
- `idx_rules_updated_at` - On `updated_at` (DESC)

#### `rulesets`
Stores ruleset definitions with inheritance support.

```sql
CREATE TABLE rulesets (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    extends VARCHAR(255),  -- Parent ruleset ID
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255),

    FOREIGN KEY (extends) REFERENCES rulesets(id) ON DELETE SET NULL
);
```

**Indexes:**
- `idx_rulesets_path` - On `path` column
- `idx_rulesets_extends` - On `extends` column
- `idx_rulesets_updated_at` - On `updated_at` (DESC)

#### `templates`
Stores decision logic templates with default parameters.

```sql
CREATE TABLE templates (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    params JSONB,  -- Default parameter values
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255)
);
```

**Indexes:**
- `idx_templates_path` - On `path` column
- `idx_templates_params` - GIN index on `params` (JSONB)
- `idx_templates_updated_at` - On `updated_at` (DESC)

#### `pipelines`
Stores pipeline definitions.

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
    created_by VARCHAR(255)
);
```

**Indexes:**
- `idx_pipelines_path` - On `path` column
- `idx_pipelines_updated_at` - On `updated_at` (DESC)

#### `artifact_audit_log`
Tracks all changes to artifacts for compliance and audit purposes.

```sql
CREATE TABLE artifact_audit_log (
    id SERIAL PRIMARY KEY,
    artifact_type VARCHAR(50) NOT NULL,  -- 'rule', 'ruleset', 'template', 'pipeline'
    artifact_id VARCHAR(255) NOT NULL,
    operation VARCHAR(50) NOT NULL,  -- 'create', 'update', 'delete'
    old_content TEXT,
    new_content TEXT,
    old_version INTEGER,
    new_version INTEGER,
    changed_by VARCHAR(255),
    changed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    change_reason TEXT,
    metadata JSONB
);
```

**Indexes:**
- `idx_audit_artifact` - On `(artifact_type, artifact_id)`
- `idx_audit_changed_at` - On `changed_at` (DESC)
- `idx_audit_artifact_time` - On `(artifact_id, changed_at DESC)`

## Audit Logging

### Automatic Triggers (Optional)

The `005_create_audit_log.sql` migration includes commented-out triggers for automatic audit logging. To enable:

1. Uncomment the trigger statements at the end of `005_create_audit_log.sql`
2. Re-run the migration

```sql
-- Enable automatic audit logging for rules
CREATE TRIGGER audit_rules_changes
AFTER INSERT OR UPDATE OR DELETE ON rules
FOR EACH ROW EXECUTE FUNCTION log_artifact_change();

-- Similar triggers for rulesets, templates, pipelines
```

### Manual Audit Logging

If you prefer manual control, log changes explicitly in your application code using the `artifact_audit_log` table.

## Version Management

All artifact tables include a `version` column that:
- Starts at 1 for new artifacts
- Increments automatically on each UPDATE
- Can be used for optimistic locking

Example query:
```sql
-- Update only if version matches (optimistic locking)
UPDATE rules
SET content = $1, version = version + 1
WHERE id = $2 AND version = $3;
```

## Example Queries

### Find all rules created in the last week
```sql
SELECT id, description, version, created_at
FROM rules
WHERE created_at >= NOW() - INTERVAL '7 days'
ORDER BY created_at DESC;
```

### Get ruleset inheritance chain
```sql
WITH RECURSIVE inheritance AS (
    SELECT id, extends, 1 as level
    FROM rulesets
    WHERE id = 'payment_high_value'

    UNION ALL

    SELECT r.id, r.extends, i.level + 1
    FROM rulesets r
    INNER JOIN inheritance i ON r.id = i.extends
)
SELECT * FROM inheritance ORDER BY level;
```

### Find templates by parameter
```sql
SELECT id, params
FROM templates
WHERE params @> '{"critical_threshold": 150}'::jsonb;
```

### Get audit history for a rule
```sql
SELECT operation, old_version, new_version, changed_by, changed_at
FROM artifact_audit_log
WHERE artifact_type = 'rule' AND artifact_id = 'fraud_farm_pattern'
ORDER BY changed_at DESC;
```

## Backup and Restore

### Backup
```bash
pg_dump $DATABASE_URL > backup_$(date +%Y%m%d).sql
```

### Restore
```bash
psql $DATABASE_URL < backup_20250101.sql
```

## Connection Strings

### Local Development
```
postgresql://postgres:password@localhost:5432/corint
```

### Production (with SSL)
```
postgresql://user:password@prod-host:5432/corint?sslmode=require
```

### Supabase
```
postgresql://postgres.project:[PASSWORD]@aws-0-region.pooler.supabase.com:6543/postgres
```

## Testing

Run the test suite with database support:

```bash
# Set test database URL
export DATABASE_URL="postgresql://localhost/corint_test"

# Create test database
createdb corint_test

# Run schema setup
for file in docs/schema/*.sql; do psql $DATABASE_URL < "$file"; done

# Run tests
cargo test --package corint-repository --features postgres
```

## Cleanup

### Drop all tables
```sql
DROP TABLE IF EXISTS artifact_audit_log CASCADE;
DROP TABLE IF EXISTS pipelines CASCADE;
DROP TABLE IF EXISTS templates CASCADE;
DROP TABLE IF EXISTS rulesets CASCADE;
DROP TABLE IF EXISTS rules CASCADE;
DROP FUNCTION IF EXISTS log_artifact_change() CASCADE;
```

### Reset database (careful in production!)
```bash
psql $DATABASE_URL -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
```

## Performance Tips

1. **Use connection pooling** - The `PostgresRepository` uses sqlx's built-in connection pooling
2. **Enable caching** - The repository includes a caching layer to reduce database queries
3. **Index maintenance** - Run `ANALYZE` periodically to update query planner statistics
4. **VACUUM** - Run `VACUUM ANALYZE` regularly to reclaim space and update statistics

```sql
-- Update statistics
ANALYZE rules;
ANALYZE rulesets;
ANALYZE templates;
ANALYZE pipelines;

-- Vacuum and analyze all tables
VACUUM ANALYZE;
```

## Security Considerations

1. **Use SSL connections** in production (`sslmode=require`)
2. **Restrict database user permissions** - Use read-only users where possible
3. **Enable audit logging** for compliance requirements
4. **Regular backups** - Schedule automated database backups
5. **Encrypt sensitive data** - Use PostgreSQL's encryption features for sensitive fields

## Troubleshooting

### Migration fails
```bash
# Check current database state
psql $DATABASE_URL -c "\dt"

# View table structure
psql $DATABASE_URL -c "\d rules"

# Check for conflicts
psql $DATABASE_URL -c "SELECT * FROM rules WHERE id = 'conflicting_id';"
```

### Connection errors
```bash
# Test connection
psql $DATABASE_URL -c "SELECT 1;"

# Check PostgreSQL is running
pg_isready -h localhost -p 5432
```

## Resources

- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [sqlx Documentation](https://docs.rs/sqlx/)
- [Supabase Documentation](https://supabase.com/docs)
