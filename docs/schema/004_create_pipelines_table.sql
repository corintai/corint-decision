-- Create pipelines table
-- Migration: 004_create_pipelines_table
-- Description: Store pipeline definitions for orchestration

CREATE TABLE IF NOT EXISTS pipelines (
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
CREATE INDEX IF NOT EXISTS idx_pipelines_path ON pipelines(path);
CREATE INDEX IF NOT EXISTS idx_pipelines_updated_at ON pipelines(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_pipelines_created_at ON pipelines(created_at DESC);

-- Add comments for documentation
COMMENT ON TABLE pipelines IS 'Stores pipeline definitions in YAML format';
COMMENT ON COLUMN pipelines.id IS 'Unique pipeline identifier (from YAML id field)';
COMMENT ON COLUMN pipelines.path IS 'Optional file path (e.g., pipelines/fraud_detection.yaml)';
COMMENT ON COLUMN pipelines.content IS 'Full YAML content of the pipeline';
COMMENT ON COLUMN pipelines.version IS 'Version number, incremented on each update';
COMMENT ON COLUMN pipelines.description IS 'Optional description extracted from pipeline';
COMMENT ON COLUMN pipelines.metadata IS 'Optional metadata in JSON format';
COMMENT ON COLUMN pipelines.created_at IS 'Timestamp when pipeline was first created';
COMMENT ON COLUMN pipelines.updated_at IS 'Timestamp when pipeline was last updated';
COMMENT ON COLUMN pipelines.created_by IS 'User or system that created the pipeline';
