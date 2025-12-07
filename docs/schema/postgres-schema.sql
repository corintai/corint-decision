-- ============================================================================
-- PostgreSQL Schema for Corint Risk Control System
-- Database Schema Design (Core Fields + JSONB Extension Pattern)
-- ============================================================================

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";      -- UUID generation
CREATE EXTENSION IF NOT EXISTS "btree_gin";      -- GIN index support for B-Tree operations
CREATE EXTENSION IF NOT EXISTS "pg_trgm";        -- Text similarity search

-- ============================================================================
-- 1. Core Events Table (events)
-- Main table for all risk control events, with time-based partitioning
-- ============================================================================

CREATE TABLE events (
    -- ========== Primary Key and Identifiers ==========
    id BIGSERIAL NOT NULL,                       -- Internal auto-increment ID (partition table primary key)
    request_id VARCHAR(64) NOT NULL,             -- Risk decision request ID (unique per decision call, returned to caller)
    event_id VARCHAR(64),                        -- Business event ID (transaction_id, order_id, etc., optional)
    event_timestamp TIMESTAMPTZ NOT NULL,        -- Event occurrence time (partition key)

    -- ========== Event Classification ==========
    event_type VARCHAR(50) NOT NULL,             -- login, transaction, register, withdraw, etc.

    -- ========== Core Dimension Fields (High-frequency queries, indexed separately) ==========
    user_id VARCHAR(64),                         -- User ID
    device_id VARCHAR(64),                       -- Device ID
    ip_address INET,                             -- IP address (INET type supports network operations)

    -- ========== JSONB Extension Fields (Flexible attributes) ==========
    attributes JSONB,                            -- All event attributes (business dimensions + technical context)
    /*
      attributes example structure (combines business dimensions and technical context):
      {
        -- Business dimensions (used for feature calculation and rule evaluation)
        "country": "US",
        "city": "New York",
        "payment_method": "credit_card",
        "card_type": "visa",
        "merchant_id": "merchant_123",
        "merchant_category": "electronics",
        "amount": 299.99,
        "currency": "USD",

        -- Device and browser information
        "device_type": "mobile",
        "os": "iOS",
        "os_version": "17.2",
        "browser": "Safari",
        "browser_version": "17.1",

        -- Technical context (for debugging and device fingerprinting)
        "user_agent": "Mozilla/5.0...",
        "screen_resolution": "1920x1080",
        "language": "en-US",
        "timezone": "America/New_York",
        "referrer": "https://app.example.com",

        -- Additional business attributes
        "is_new_device": true,
        "account_age_days": 365,
        "customer_segment": "premium"
      }

      Note: All metrics/features are calculated in real-time from raw data,
            not stored here. This field only contains input attributes.
    */

    -- ========== Audit Fields ==========
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- ========== Primary Key ==========
    PRIMARY KEY (id)
);

-- ============================================================================
-- Note: Table Partitioning (Optional - Enable when data volume grows)
-- ============================================================================
-- When your data volume reaches millions of rows (typically 10M+), you can enable
-- time-based partitioning for better query performance and easier data management.
--
-- To enable partitioning, replace the table definition above with:
--
-- CREATE TABLE events (
--     ... (same fields as above)
-- ) PARTITION BY RANGE (event_timestamp);
--
-- Then create monthly partitions:
-- CREATE TABLE events_2025_01 PARTITION OF events
--     FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');
-- CREATE TABLE events_2025_02 PARTITION OF events
--     FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');
-- ... and so on
--
-- Benefits of partitioning:
-- 1. Faster queries with time range filters (only scans relevant partitions)
-- 2. Easy archival/deletion of old data (DROP TABLE old_partition)
-- 3. Smaller indexes per partition
-- 4. Better maintenance performance
--
-- Trade-offs:
-- - Unique indexes must include partition key (event_timestamp)
-- - Slightly more complex schema management
-- ============================================================================

-- Unique constraint for idempotency (prevent duplicate decision requests)
CREATE UNIQUE INDEX idx_events_request_id ON events (request_id);

-- Create index on event_id for business event lookups (event_id is optional, so not unique)
CREATE INDEX idx_events_event_id ON events (event_id) WHERE event_id IS NOT NULL;

-- ============================================================================
-- 2. Risk Decisions Table (risk_decisions)
-- Stores risk decision results separately from raw event data
-- ============================================================================

CREATE TABLE risk_decisions (
    -- ========== Primary Key ==========
    id BIGSERIAL PRIMARY KEY,

    -- ========== Link to Event ==========
    request_id VARCHAR(64) NOT NULL,             -- Links to events.request_id (decision request ID)
    event_id VARCHAR(64),                        -- Optional business event ID (for reference)

    -- ========== Pipeline Identifier ==========
    pipeline_id VARCHAR(100) NOT NULL,           -- Pipeline ID that processed this decision

    -- ========== Risk Scores ==========
    risk_score NUMERIC(5, 2) NOT NULL,           -- Overall risk score (0-100)

    -- ========== Decision Result ==========
    decision VARCHAR(20) NOT NULL,               -- approve, deny, review, monitor
    decision_reason TEXT,                        -- Human-readable decision explanation

    -- ========== Rule Execution Details ==========
    triggered_rules TEXT[],                      -- Array of triggered rule IDs
    rule_scores JSONB,                           -- Individual rule scores: {"rule_1": 25, "rule_2": 15}

    -- ========== Feature Values (Optional) ==========
    feature_values JSONB,                        -- Calculated feature values for this decision
    /*
      Example feature_values structure:
      {
        "txn_velocity_1h": 5,
        "user_age_days": 365,
        "device_trust_score": 85.5,
        "ip_reputation_score": 70.2,
        "amount_zscore": 2.3
      }
    */

    -- ========== Processing Metadata ==========
    processing_time_ms INT,                      -- Decision processing time in milliseconds

    -- ========== Timestamps ==========
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- Risk Decisions Indexes
-- ============================================================================

-- Primary lookup index (link to events)
CREATE UNIQUE INDEX idx_risk_decisions_request_id ON risk_decisions (request_id);

-- Optional business event ID lookup
CREATE INDEX idx_risk_decisions_event_id ON risk_decisions (event_id)
    WHERE event_id IS NOT NULL;

-- Query by decision result
CREATE INDEX idx_risk_decisions_decision ON risk_decisions (decision, created_at DESC);

-- Query by pipeline ID
CREATE INDEX idx_risk_decisions_pipeline_id ON risk_decisions (pipeline_id, created_at DESC);

-- High-risk score queries
CREATE INDEX idx_risk_decisions_high_risk ON risk_decisions (risk_score DESC, created_at DESC)
    WHERE risk_score > 60;

-- Time-based queries
CREATE INDEX idx_risk_decisions_timestamp ON risk_decisions (created_at DESC);

-- JSONB index for feature values
CREATE INDEX idx_risk_decisions_features_gin ON risk_decisions USING GIN (feature_values);

-- JSONB index for rule scores
CREATE INDEX idx_risk_decisions_rule_scores_gin ON risk_decisions USING GIN (rule_scores);

-- ============================================================================
-- Core Field Indexes (Optimized for high-frequency queries)
-- ============================================================================

-- Timestamp index (Most important, used by almost all queries)
CREATE INDEX idx_events_timestamp ON events (event_timestamp DESC);

-- User dimension indexes
CREATE INDEX idx_events_user_id ON events (user_id) WHERE user_id IS NOT NULL;
CREATE INDEX idx_events_user_timestamp ON events (user_id, event_timestamp DESC) WHERE user_id IS NOT NULL;

-- Device dimension indexes
CREATE INDEX idx_events_device_id ON events (device_id) WHERE device_id IS NOT NULL;
CREATE INDEX idx_events_device_timestamp ON events (device_id, event_timestamp DESC) WHERE device_id IS NOT NULL;

-- IP dimension indexes
CREATE INDEX idx_events_ip_address ON events (ip_address) WHERE ip_address IS NOT NULL;
CREATE INDEX idx_events_ip_timestamp ON events (ip_address, event_timestamp DESC) WHERE ip_address IS NOT NULL;

-- Event type indexes
CREATE INDEX idx_events_event_type ON events (event_type);
CREATE INDEX idx_events_type_timestamp ON events (event_type, event_timestamp DESC);

-- Composite indexes (Optimize common query patterns)
CREATE INDEX idx_events_user_type_time ON events (user_id, event_type, event_timestamp DESC)
    WHERE user_id IS NOT NULL;
CREATE INDEX idx_events_device_type_time ON events (device_id, event_type, event_timestamp DESC)
    WHERE device_id IS NOT NULL;

-- ============================================================================
-- JSONB Field Indexes (Flexible dimension query optimization)
-- ============================================================================

-- GIN index (Support all JSONB operations: @>, ?, ?&, ?|)
-- Single GIN index covers all attribute queries
CREATE INDEX idx_events_attributes_gin ON events USING GIN (attributes);

-- ============================================================================
-- B-Tree indexes for specific JSONB paths (Optional - Enable based on query patterns)
-- ============================================================================
-- Note: These B-Tree indexes provide better performance for specific attribute queries
--       compared to GIN indexes, but increase index maintenance overhead.
--       Only create these indexes if you have high-frequency queries on specific attributes.
--
-- Business dimension indexes
-- CREATE INDEX idx_attr_country ON events ((attributes->>'country'))
--     WHERE attributes->>'country' IS NOT NULL;
-- CREATE INDEX idx_attr_city ON events ((attributes->>'city'))
--     WHERE attributes->>'city' IS NOT NULL;
-- CREATE INDEX idx_attr_payment_method ON events ((attributes->>'payment_method'))
--     WHERE attributes->>'payment_method' IS NOT NULL;
-- CREATE INDEX idx_attr_merchant_id ON events ((attributes->>'merchant_id'))
--     WHERE attributes->>'merchant_id' IS NOT NULL;
-- CREATE INDEX idx_attr_merchant_category ON events ((attributes->>'merchant_category'))
--     WHERE attributes->>'merchant_category' IS NOT NULL;
--
-- Device and technical context indexes
-- CREATE INDEX idx_attr_device_type ON events ((attributes->>'device_type'))
--     WHERE attributes->>'device_type' IS NOT NULL;
-- CREATE INDEX idx_attr_os ON events ((attributes->>'os'))
--     WHERE attributes->>'os' IS NOT NULL;
-- CREATE INDEX idx_attr_browser ON events ((attributes->>'browser'))
--     WHERE attributes->>'browser' IS NOT NULL;
--
-- Boolean field indexes
-- CREATE INDEX idx_attr_new_device ON events (((attributes->>'is_new_device')::boolean))
--     WHERE (attributes->>'is_new_device')::boolean = true;

-- ============================================================================
-- 3. Rule Execution Log Table (rule_executions)
-- Record rule engine execution history
-- ============================================================================

CREATE TABLE rule_executions (
    id BIGSERIAL PRIMARY KEY,
    request_id VARCHAR(64) NOT NULL,
    rule_id VARCHAR(100) NOT NULL,
    rule_name VARCHAR(255),

    -- Execution result
    triggered BOOLEAN NOT NULL,
    score NUMERIC(5, 2),
    execution_time_ms INT,

    -- Rule context
    feature_values JSONB,                        -- Feature calculation results
    rule_conditions JSONB,                       -- Rule conditions

    -- Audit fields
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Rule execution log indexes
CREATE INDEX idx_rule_exec_request_id ON rule_executions (request_id);
CREATE INDEX idx_rule_exec_rule_id ON rule_executions (rule_id);
CREATE INDEX idx_rule_exec_triggered ON rule_executions (triggered, created_at DESC)
    WHERE triggered = TRUE;
CREATE INDEX idx_rule_exec_time ON rule_executions (created_at DESC);

-- ============================================================================
-- 4. Comments and Documentation
-- ============================================================================

COMMENT ON TABLE events IS 'Core events table: Stores raw event data (input) with time-based partitioning';
COMMENT ON COLUMN events.request_id IS 'Unique risk decision request ID (links to risk_decisions table)';
COMMENT ON COLUMN events.event_id IS 'Optional business event ID (transaction_id, order_id, etc.)';
COMMENT ON COLUMN events.attributes IS 'JSONB field: Flexible business dimensions and technical context';

COMMENT ON TABLE risk_decisions IS 'Risk decisions table: Stores decision results (output) linked to events by request_id';
COMMENT ON COLUMN risk_decisions.request_id IS 'Links to events.request_id (one-to-one relationship)';
COMMENT ON COLUMN risk_decisions.feature_values IS 'JSONB field: Calculated feature values used for this decision';
COMMENT ON COLUMN risk_decisions.rule_scores IS 'JSONB field: Individual rule scores';

COMMENT ON TABLE rule_executions IS 'Rule execution log: Record rule engine execution history';

-- ============================================================================
-- End of Schema
-- ============================================================================
