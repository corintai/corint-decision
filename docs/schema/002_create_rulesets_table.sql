-- Create rulesets table
-- Migration: 002_create_rulesets_table
-- Description: Store ruleset definitions with inheritance support

CREATE TABLE IF NOT EXISTS rulesets (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    extends VARCHAR(255),  -- Parent ruleset ID for inheritance
    description TEXT,
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_rulesets_path ON rulesets(path);
CREATE INDEX IF NOT EXISTS idx_rulesets_extends ON rulesets(extends);
CREATE INDEX IF NOT EXISTS idx_rulesets_updated_at ON rulesets(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_rulesets_created_at ON rulesets(created_at DESC);

-- Add foreign key constraint for extends (self-referential)
-- Note: We use ON DELETE SET NULL so deleting a parent doesn't cascade
ALTER TABLE rulesets
ADD CONSTRAINT fk_rulesets_extends
FOREIGN KEY (extends)
REFERENCES rulesets(id)
ON DELETE SET NULL;

-- Add comments for documentation
COMMENT ON TABLE rulesets IS 'Stores ruleset definitions in YAML format';
COMMENT ON COLUMN rulesets.id IS 'Unique ruleset identifier (from YAML id field)';
COMMENT ON COLUMN rulesets.path IS 'Optional file path (e.g., library/rulesets/fraud_detection_core.yaml)';
COMMENT ON COLUMN rulesets.content IS 'Full YAML content of the ruleset';
COMMENT ON COLUMN rulesets.version IS 'Version number, incremented on each update';
COMMENT ON COLUMN rulesets.extends IS 'Parent ruleset ID for inheritance (Phase 3 feature)';
COMMENT ON COLUMN rulesets.description IS 'Optional description extracted from ruleset';
COMMENT ON COLUMN rulesets.metadata IS 'Optional metadata in JSON format';
COMMENT ON COLUMN rulesets.created_at IS 'Timestamp when ruleset was first created';
COMMENT ON COLUMN rulesets.updated_at IS 'Timestamp when ruleset was last updated';
COMMENT ON COLUMN rulesets.created_by IS 'User or system that created the ruleset';
