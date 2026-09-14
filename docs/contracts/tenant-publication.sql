-- SQLite and PostgreSQL. Only the trusted repository publisher may write here.
-- Publish a complete core-repository document in one transaction.
CREATE TABLE corint_tenant_publication (
    tenant_id TEXT NOT NULL,
    environment TEXT NOT NULL,
    deployment TEXT NOT NULL,
    document TEXT NOT NULL,
    PRIMARY KEY (tenant_id, environment, deployment)
);
