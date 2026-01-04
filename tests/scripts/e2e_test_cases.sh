#!/bin/bash

# ============================================================================
# CORINT Decision Engine - E2E Test Cases
# ============================================================================
# This file contains all test cases for the E2E test suite.
# It is sourced by run_e2e_tests.sh
#
# Test case format:
#   run_test_case "Test Name" 'JSON payload' "expected_decision"
#   run_error_test_case "Test Name" 'JSON payload' "expected_pattern"
#
# Expected decisions: approve, review, decline
# Expected patterns: no_pipeline, error, default_fallback
# ============================================================================

# ============================================================================
# Transaction Pipeline Tests
# ============================================================================

log_info "Running Transaction Pipeline Tests..."
echo ""

# Test 1: Normal low-value transaction - should approve
run_test_case "Normal Transaction - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_0001",
        "amount": 50.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00001",
        "ip_address": "192.168.1.1",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 2: Blocked user transaction - should decline
run_test_case "Blocked User Transaction - Decline" '{
    "event": {
        "type": "transaction",
        "user_id": "sus_0001",
        "amount": 100.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00002",
        "ip_address": "192.168.1.2",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 3: High-risk country transaction - should review
run_test_case "High Risk Country Transaction - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_0002",
        "amount": 200.00,
        "currency": "USD",
        "country": "NG",
        "device_id": "device_00003",
        "ip_address": "192.168.1.3",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 4: High-value new user transaction - should review/decline
run_test_case "High Value New User - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_new_highvalue",
        "amount": 5000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00004",
        "ip_address": "192.168.1.4",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 5: Multiple risk factors - should decline
run_test_case "Multiple Risk Factors - Decline" '{
    "event": {
        "type": "transaction",
        "user_id": "user_0005",
        "amount": 8000.00,
        "currency": "USD",
        "country": "RU",
        "device_id": "device_00005",
        "ip_address": "45.142.212.61",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# ============================================================================
# Login Pipeline Tests
# ============================================================================

echo ""
log_info "Running Login Pipeline Tests..."
echo ""

# Test 6: Normal login - should approve
run_test_case "Normal Login - Approve" '{
    "event": {
        "type": "login",
        "user_id": "user_0100",
        "country": "US",
        "device_id": "device_00010",
        "ip_address": "192.168.1.10",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 7: High risk country login (with sufficient login history) - should approve
run_test_case "High Risk Country Login - Approve" '{
    "event": {
        "type": "login",
        "user_id": "test_clean_user_russia",
        "country": "RU",
        "device_id": "test_device_001",
        "ip_address": "192.168.1.11",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 8: Blocked IP login - should decline
run_test_case "Blocked IP Login - Decline" '{
    "event": {
        "type": "login",
        "user_id": "user_0103",
        "country": "US",
        "device_id": "device_00012",
        "ip_address": "45.142.212.61",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 9: Excessive failed logins - should decline
run_test_case "Excessive Failed Logins - Decline" '{
    "event": {
        "type": "login",
        "user_id": "user_0105",
        "country": "US",
        "device_id": "device_00013",
        "ip_address": "192.168.1.13",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# ============================================================================
# Payment Pipeline Tests
# ============================================================================

echo ""
log_info "Running Payment Pipeline Tests..."
echo ""

# Test 10: Normal payment - should approve
run_test_case "Normal Payment - Approve" '{
    "event": {
        "type": "payment",
        "user_id": "user_0001",
        "amount": 100.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00020",
        "ip_address": "192.168.1.20",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 11: Blocked user payment - should decline
run_test_case "Blocked User Payment - Decline" '{
    "event": {
        "type": "payment",
        "user_id": "sus_0002",
        "amount": 200.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00021",
        "ip_address": "192.168.1.21",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 12: VIP user high-value payment - should approve
run_test_case "VIP High Value Payment - Approve" '{
    "event": {
        "type": "payment",
        "user_id": "user_vip_001",
        "amount": 12000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_vip_001",
        "ip_address": "192.168.1.22",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 13: High frequency transaction user - should decline
run_test_case "High Frequency Transactions - Decline" '{
    "event": {
        "type": "transaction",
        "user_id": "user_velocity_24h",
        "amount": 250.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00023",
        "ip_address": "192.168.1.23",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 14: Geographic mismatch (new device from high-risk country) - should review
# User has 2 devices in history, now logs in from high-risk country
run_test_case "Geographic Mismatch - Review" '{
    "event": {
        "type": "login",
        "user_id": "user_0102",
        "country": "NG",
        "device_id": "device_geo_new",
        "ip_address": "192.168.2.100",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 15: Very high amount payment - should review
run_test_case "Very High Amount Payment - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_0001",
        "amount": 6000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00025",
        "ip_address": "192.168.1.25",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 16: Crypto payment over threshold - should review
run_test_case "Crypto Payment - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_0104",
        "amount": 3000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00026",
        "ip_address": "192.168.1.26",
        "payment_method": "crypto",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# ============================================================================
# Edge Cases and Error Handling Tests
# ============================================================================

echo ""
log_info "Running Edge Cases and Error Handling Tests..."
echo ""

# Test 17: Unknown event type - should use default fallback pipeline
run_error_test_case "Unknown Event Type" '{
    "event": {
        "type": "unknown_type",
        "user_id": "user_9997",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "default_fallback"

# Test 18: Missing optional fields - should approve (graceful handling)
run_test_case "Missing Fields" '{
    "event": {
        "type": "transaction"
    }
}' "approve"

# Test 19: Boundary amount (exactly at threshold)
run_test_case "Boundary Amount - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_boundary_1",
        "amount": 1000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00030",
        "ip_address": "192.168.1.30",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# ============================================================================
# Feature-Specific Tests (Aggregation Features)
# ============================================================================

echo ""
log_info "Running Feature-Specific Tests..."
echo ""

# Test 20: Low weekly activity with high-value transaction - should review
run_test_case "Low Weekly Activity - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_low_weekly_activity",
        "amount": 2500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00040",
        "ip_address": "192.168.1.40",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 21: High total spending - should review (high cumulative spending with large transaction)
run_test_case "High Total Spending - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_high_total_spending",
        "amount": 6000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_high_spending",
        "ip_address": "192.168.1.41",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 22: Large vs average - should review
run_test_case "Large vs Average - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_avg_baseline",
        "amount": 1100.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00042",
        "ip_address": "192.168.1.42",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 23: Exceeds max history - should review
run_test_case "Exceeds Max History - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_max_history",
        "amount": 1600.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00043",
        "ip_address": "192.168.1.43",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 24: Micro transaction pattern - should review (jump from micro to large transactions)
run_test_case "Micro Transaction Pattern - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_micro_pattern",
        "amount": 3200.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00044",
        "ip_address": "192.168.1.44",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 25: Recent spending spike - should review
run_test_case "Recent Spending Spike - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_recent_spike",
        "amount": 600.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00045",
        "ip_address": "192.168.1.45",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 26: Wide amount range - should review (large variance in transaction amounts)
run_test_case "Wide Amount Range - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_wide_range",
        "amount": 1000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00046",
        "ip_address": "192.168.1.46",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 27: Spending acceleration - should review
run_test_case "Spending Acceleration - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_acceleration",
        "amount": 700.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00047",
        "ip_address": "192.168.1.47",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 28: Multi-device 24h - should review
run_test_case "Multi Device 24h - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_multi_device_24h",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_test_005",
        "ip_address": "192.168.1.48",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 29: High device activity - should review
run_test_case "High Device Activity - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_device_high",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_high_activity",
        "ip_address": "192.168.1.49",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# ============================================================================
# Payment Feature Tests
# ============================================================================

echo ""
log_info "Running Payment Feature Tests..."
echo ""

# Test 30: Payment frequency - should review
run_test_case "Payment Frequency - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_payment_freq",
        "amount": 600.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00050",
        "ip_address": "192.168.1.50",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 31: Weekly payment limit - should review
run_test_case "Weekly Payment Limit - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_payment_weekly",
        "amount": 3500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00051",
        "ip_address": "192.168.1.51",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 32: Payment max exceeded - should review
run_test_case "Payment Max Exceeded - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_payment_max",
        "amount": 5000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00052",
        "ip_address": "192.168.1.52",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 33: Payment ratio - should approve
run_test_case "Payment Ratio - Approve" '{
    "event": {
        "type": "payment",
        "user_id": "user_payment_ratio",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00053",
        "ip_address": "192.168.1.53",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# ============================================================================
# Login Feature Tests
# ============================================================================

echo ""
log_info "Running Login Feature Tests..."
echo ""

# Test 34: Low login history from high-risk country - should review
run_test_case "Low Login History - Review" '{
    "event": {
        "type": "login",
        "user_id": "user_low_login",
        "country": "NG",
        "device_id": "device_low_login",
        "ip_address": "192.168.1.60",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 35: Multi-IP 24h - should review
run_test_case "Multi IP 24h - Review" '{
    "event": {
        "type": "login",
        "user_id": "user_multi_ip",
        "country": "US",
        "device_id": "device_multi_ip",
        "ip_address": "10.0.7.1",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 36: High failure rate - should decline (10 failed logins in 24h triggers excessive_failures)
run_test_case "High Failure Rate - Decline" '{
    "event": {
        "type": "login",
        "user_id": "user_failure_rate",
        "country": "US",
        "device_id": "device_00062",
        "ip_address": "192.168.1.62",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 37: Device velocity - should review
run_test_case "Device Velocity - Review" '{
    "event": {
        "type": "login",
        "user_id": "user_device_velocity",
        "country": "US",
        "device_id": "device_24h_001",
        "ip_address": "172.16.24.1",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 38: Shared device - should review
run_test_case "Shared Device - Review" '{
    "event": {
        "type": "login",
        "user_id": "shared_user_007",
        "country": "US",
        "device_id": "device_shared",
        "ip_address": "192.168.1.64",
        "status": "success",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

echo ""
log_info "Test cases completed!"
echo ""
