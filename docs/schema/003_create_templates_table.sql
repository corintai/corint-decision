-- Create templates table
-- Migration: 003_create_templates_table
-- Description: Store decision logic templates with parameterization support

CREATE TABLE IF NOT EXISTS templates (
    id VARCHAR(255) PRIMARY KEY,
    path VARCHAR(512),
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    description TEXT,
    params JSONB,  -- Default parameter values in JSON format
    metadata JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    created_by VARCHAR(255)
);

-- Create indexes for performance
CREATE INDEX IF NOT EXISTS idx_templates_path ON templates(path);
CREATE INDEX IF NOT EXISTS idx_templates_updated_at ON templates(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_templates_created_at ON templates(created_at DESC);

-- Create GIN index for params JSONB column for efficient querying
CREATE INDEX IF NOT EXISTS idx_templates_params ON templates USING GIN (params);

-- Add comments for documentation
COMMENT ON TABLE templates IS 'Stores decision logic templates in YAML format (Phase 3 feature)';
COMMENT ON COLUMN templates.id IS 'Unique template identifier (from YAML id field)';
COMMENT ON COLUMN templates.path IS 'Optional file path (e.g., library/templates/score_based_decision.yaml)';
COMMENT ON COLUMN templates.content IS 'Full YAML content of the template';
COMMENT ON COLUMN templates.version IS 'Version number, incremented on each update';
COMMENT ON COLUMN templates.description IS 'Optional description extracted from template';
COMMENT ON COLUMN templates.params IS 'Default parameter values in JSON format';
COMMENT ON COLUMN templates.metadata IS 'Optional metadata in JSON format';
COMMENT ON COLUMN templates.created_at IS 'Timestamp when template was first created';
COMMENT ON COLUMN templates.updated_at IS 'Timestamp when template was last updated';
COMMENT ON COLUMN templates.created_by IS 'User or system that created the template';
