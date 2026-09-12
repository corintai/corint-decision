#!/usr/bin/env bash
# Sourced by run_e2e_tests.sh; SQL mutations touch only this run's schema.
log_info 'Running RisingWave materialized view -> Lookup -> HTTP decision tests...'

rw_payload() { jq -nc --arg id "$1" '{event:{type:"transaction",user:{id:$id}}}'; }
rw_sql() { PGCONNECT_TIMEOUT=5 psql -X -q -v ON_ERROR_STOP=1 -v rw_schema="$RISINGWAVE_SCHEMA" "$RISINGWAVE_URL"; }

run_test_case 'RW low velocity' "$(rw_payload low)" approve
run_test_case 'RW high velocity and entity isolation' "$(rw_payload high)" decline rw_high_velocity
run_test_case 'RW window excludes old and future events' "$(rw_payload window)" approve
run_test_case 'RW missing row uses fallback' "$(rw_payload missing)" review rw_feature_unavailable
run_test_case 'RW entity key is bound, not SQL' "$(rw_payload "low' OR TRUE --")" review rw_feature_unavailable
run_error_test_case 'RW missing key cannot use fallback' '{"event":{"type":"transaction","user":{}}}' feature_error

rw_sql <<'SQL_UPDATE'
INSERT INTO :"rw_schema".transactions VALUES
    (8, 'low', NOW() - INTERVAL '1 minute'),
    (9, 'low', NOW() - INTERVAL '1 minute');
FLUSH;
SQL_UPDATE
run_test_case 'RW updated view is visible without cache' "$(rw_payload low)" decline rw_high_velocity

rw_sql <<'SQL_DELETE'
DELETE FROM :"rw_schema".transactions WHERE user_id = 'low';
FLUSH;
SQL_DELETE
run_test_case 'RW deleted group becomes missing' "$(rw_payload low)" review rw_feature_unavailable

rw_sql <<'SQL_DROP'
DROP MATERIALIZED VIEW :"rw_schema".user_features_mv;
SQL_DROP
run_test_case 'RW query failure uses explicit fallback' "$(rw_payload high)" review rw_feature_unavailable

rw_sql <<'SQL_RECOVER'
CREATE MATERIALIZED VIEW :"rw_schema".user_features_mv AS
SELECT user_id, COUNT(*) AS txn_count_1h
FROM :"rw_schema".transactions
WHERE event_timestamp >= NOW() - INTERVAL '1 hour'
  AND event_timestamp < NOW()
GROUP BY user_id;
FLUSH;
SQL_RECOVER
run_test_case 'RW recovery does not retain fallback' "$(rw_payload high)" decline rw_high_velocity
