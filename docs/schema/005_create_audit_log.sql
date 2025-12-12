-- Create audit log table
-- Migration: 005_create_audit_log
-- Description: Track all changes to rules, rulesets, templates, and pipelines for compliance

CREATE TABLE IF NOT EXISTS artifact_audit_log (
    id SERIAL PRIMARY KEY,
    artifact_type VARCHAR(50) NOT NULL,  -- 'rule', 'ruleset', 'template', 'pipeline'
    artifact_id VARCHAR(255) NOT NULL,
    operation VARCHAR(50) NOT NULL,  -- 'create', 'update', 'delete'
    old_content TEXT,  -- Previous content (NULL for create operations)
    new_content TEXT,  -- New content (NULL for delete operations)
    old_version INTEGER,
    new_version INTEGER,
    changed_by VARCHAR(255),
    changed_at TIMESTAMP NOT NULL DEFAULT NOW(),
    change_reason TEXT,  -- Optional reason for the change
    metadata JSONB  -- Additional context about the change
);

-- Create indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_audit_artifact ON artifact_audit_log(artifact_type, artifact_id);
CREATE INDEX IF NOT EXISTS idx_audit_changed_at ON artifact_audit_log(changed_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_changed_by ON artifact_audit_log(changed_by);
CREATE INDEX IF NOT EXISTS idx_audit_operation ON artifact_audit_log(operation);

-- Create composite index for common queries
CREATE INDEX IF NOT EXISTS idx_audit_artifact_time
ON artifact_audit_log(artifact_id, changed_at DESC);

-- Add check constraint for artifact_type
ALTER TABLE artifact_audit_log
ADD CONSTRAINT chk_artifact_type
CHECK (artifact_type IN ('rule', 'ruleset', 'template', 'pipeline'));

-- Add check constraint for operation
ALTER TABLE artifact_audit_log
ADD CONSTRAINT chk_operation
CHECK (operation IN ('create', 'update', 'delete'));

-- Add comments for documentation
COMMENT ON TABLE artifact_audit_log IS 'Audit log for all changes to decision artifacts';
COMMENT ON COLUMN artifact_audit_log.id IS 'Auto-incrementing unique identifier for each audit entry';
COMMENT ON COLUMN artifact_audit_log.artifact_type IS 'Type of artifact: rule, ruleset, template, or pipeline';
COMMENT ON COLUMN artifact_audit_log.artifact_id IS 'ID of the artifact that was changed';
COMMENT ON COLUMN artifact_audit_log.operation IS 'Type of change: create, update, or delete';
COMMENT ON COLUMN artifact_audit_log.old_content IS 'Previous YAML content (NULL for create)';
COMMENT ON COLUMN artifact_audit_log.new_content IS 'New YAML content (NULL for delete)';
COMMENT ON COLUMN artifact_audit_log.old_version IS 'Previous version number';
COMMENT ON COLUMN artifact_audit_log.new_version IS 'New version number';
COMMENT ON COLUMN artifact_audit_log.changed_by IS 'User or system that made the change';
COMMENT ON COLUMN artifact_audit_log.changed_at IS 'Timestamp when the change occurred';
COMMENT ON COLUMN artifact_audit_log.change_reason IS 'Optional reason or description of why the change was made';
COMMENT ON COLUMN artifact_audit_log.metadata IS 'Additional context in JSON format (e.g., request ID, IP address)';

-- Create a function to automatically log changes (optional, can be enabled with triggers)
CREATE OR REPLACE FUNCTION log_artifact_change()
RETURNS TRIGGER AS $$
BEGIN
    IF (TG_OP = 'DELETE') THEN
        INSERT INTO artifact_audit_log (artifact_type, artifact_id, operation, old_content, old_version)
        VALUES (TG_TABLE_NAME, OLD.id, 'delete', OLD.content, OLD.version);
        RETURN OLD;
    ELSIF (TG_OP = 'UPDATE') THEN
        INSERT INTO artifact_audit_log (artifact_type, artifact_id, operation, old_content, new_content, old_version, new_version)
        VALUES (TG_TABLE_NAME, OLD.id, 'update', OLD.content, NEW.content, OLD.version, NEW.version);
        RETURN NEW;
    ELSIF (TG_OP = 'INSERT') THEN
        INSERT INTO artifact_audit_log (artifact_type, artifact_id, operation, new_content, new_version)
        VALUES (TG_TABLE_NAME, NEW.id, 'create', NEW.content, NEW.version);
        RETURN NEW;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Create triggers for automatic audit logging (optional - comment out if not needed)
-- Uncomment these to enable automatic audit logging:

-- CREATE TRIGGER audit_rules_changes
-- AFTER INSERT OR UPDATE OR DELETE ON rules
-- FOR EACH ROW EXECUTE FUNCTION log_artifact_change();

-- CREATE TRIGGER audit_rulesets_changes
-- AFTER INSERT OR UPDATE OR DELETE ON rulesets
-- FOR EACH ROW EXECUTE FUNCTION log_artifact_change();

-- CREATE TRIGGER audit_templates_changes
-- AFTER INSERT OR UPDATE OR DELETE ON templates
-- FOR EACH ROW EXECUTE FUNCTION log_artifact_change();

-- CREATE TRIGGER audit_pipelines_changes
-- AFTER INSERT OR UPDATE OR DELETE ON pipelines
-- FOR EACH ROW EXECUTE FUNCTION log_artifact_change();
