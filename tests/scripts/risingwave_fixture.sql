-- The runner supplies a unique rw_schema and removes only that schema on exit.
CREATE TABLE :"rw_schema".transactions (
    id BIGINT PRIMARY KEY,
    user_id VARCHAR,
    event_timestamp TIMESTAMPTZ
);
CREATE MATERIALIZED VIEW :"rw_schema".user_features_mv AS
SELECT user_id, COUNT(*) AS txn_count_1h
FROM :"rw_schema".transactions
WHERE event_timestamp >= NOW() - INTERVAL '1 hour'
  AND event_timestamp < NOW()
GROUP BY user_id;
INSERT INTO :"rw_schema".transactions VALUES
    (1, 'low', NOW() - INTERVAL '1 minute'),
    (2, 'high', NOW() - INTERVAL '1 minute'),
    (3, 'high', NOW() - INTERVAL '2 minutes'),
    (4, 'high', NOW() - INTERVAL '3 minutes'),
    (5, 'window', NOW() - INTERVAL '1 minute'),
    (6, 'window', NOW() - INTERVAL '2 hours'),
    (7, 'window', NOW() + INTERVAL '1 hour');
FLUSH;
