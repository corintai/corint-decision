-- Create rules table
-- Migration: 001_create_rules_table
-- Description: Store rule definitions with versioning support

CREATE TABLE IF NOT EXISTS rules (
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

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_rules_path ON rules(path);
CREATE INDEX IF NOT EXISTS idx_rules_updated_at ON rules(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_rules_created_at ON rules(created_at DESC);

-- Add comments for documentation
COMMENT ON TABLE rules IS 'Stores rule definitions in YAML format';
COMMENT ON COLUMN rules.id IS 'Unique rule identifier (from YAML id field)';
COMMENT ON COLUMN rules.path IS 'Optional file path (e.g., library/rules/fraud/fraud_farm.yaml)';
COMMENT ON COLUMN rules.content IS 'Full YAML content of the rule';
COMMENT ON COLUMN rules.version IS 'Version number, incremented on each update';
COMMENT ON COLUMN rules.description IS 'Optional description extracted from rule';
COMMENT ON COLUMN rules.metadata IS 'Optional metadata in JSON format';
COMMENT ON COLUMN rules.created_at IS 'Timestamp when rule was first created';
COMMENT ON COLUMN rules.updated_at IS 'Timestamp when rule was last updated';
COMMENT ON COLUMN rules.created_by IS 'User or system that created the rule';
