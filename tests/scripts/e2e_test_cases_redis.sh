#!/bin/bash
# Redis E2E Test Cases
# Tests lookup features from Redis feature store

# =============================================================================
# Test Cases for Redis Feature Store
# =============================================================================

log_info "Running Redis Feature Store Tests..."
echo ""

# Test 1: High Risk User - should decline (score >= 200)
run_test_case "High Risk User - Decline" '{
    "event": {
        "type": "transaction",
        "user_id": "user_high_risk",
        "amount": 1000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_high_risk",
        "ip_address": "192.168.1.100",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 2: Medium Risk User - should review (score >= 80)
run_test_case "Medium Risk User - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_medium_risk",
        "amount": 800.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_medium_risk",
        "ip_address": "192.168.1.101",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 3: Low Risk User - should approve (score < 80)
run_test_case "Low Risk User - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_low_risk",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_low_risk",
        "ip_address": "192.168.1.102",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 4: Suspicious Device - should review
run_test_case "Suspicious Device - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_suspicious_device",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_suspicious",
        "ip_address": "192.168.1.103",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 5: Risky IP - should review
run_test_case "Risky IP Address - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_risky_ip",
        "amount": 600.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_risky_ip",
        "ip_address": "192.168.1.104",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 6: New Account High Value - should review
run_test_case "New Account High Value - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_new_account",
        "amount": 1500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_new_account",
        "ip_address": "192.168.1.105",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 7: High Velocity User - should review
run_test_case "High Velocity User - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_high_velocity",
        "amount": 400.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_high_velocity",
        "ip_address": "192.168.1.106",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 8: Multi-Device Pattern - should review
run_test_case "Multi-Device Pattern - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_multi_device",
        "amount": 450.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_multi_01",
        "ip_address": "192.168.1.107",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 9: Normal User - should approve
run_test_case "Normal User - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_normal",
        "amount": 250.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_normal",
        "ip_address": "192.168.1.108",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 10: Established User - should approve
run_test_case "Established User - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_established",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_established",
        "ip_address": "192.168.1.109",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 11: Login with High Risk User - should decline
run_test_case "High Risk Login - Decline" '{
    "event": {
        "type": "login",
        "user_id": "user_high_risk",
        "device_id": "device_high_risk",
        "ip_address": "192.168.1.100",
        "country": "US",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 12: Login with Low Risk User - should approve
run_test_case "Low Risk Login - Approve" '{
    "event": {
        "type": "login",
        "user_id": "user_low_risk",
        "device_id": "device_low_risk",
        "ip_address": "192.168.1.102",
        "country": "US",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 13: Payment with Crypto (high fraud rate) - should review
run_test_case "Crypto Payment - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_medium_risk",
        "amount": 800.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_medium_risk",
        "ip_address": "192.168.1.115",
        "payment_method": "crypto",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 14: Payment with Bank Transfer (low fraud rate) - should approve
run_test_case "Bank Transfer Payment - Approve" '{
    "event": {
        "type": "payment",
        "user_id": "user_low_risk",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_low_risk",
        "ip_address": "192.168.1.102",
        "payment_method": "bank_transfer",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 15: New Account Small Transaction - should approve
run_test_case "New Account Small Transaction - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_new_account",
        "amount": 50.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_new_account",
        "ip_address": "192.168.1.105",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# =============================================================================
# Boundary Tests
# =============================================================================

echo ""
log_info "Running Boundary Tests..."
echo ""

# Test 16: Boundary Risk Score (exactly 80) - should review
run_test_case "Boundary Risk Score 80 - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_boundary_80",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_boundary_80",
        "ip_address": "192.168.1.110",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 17: Boundary Risk Score (exactly 200) - should decline
run_test_case "Boundary Risk Score 200 - Decline" '{
    "event": {
        "type": "transaction",
        "user_id": "user_boundary_200",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_boundary_200",
        "ip_address": "192.168.1.111",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 18: Just Below Decline Threshold (199) - should review
run_test_case "Below Decline Threshold - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_score_199",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_score_199",
        "ip_address": "192.168.1.112",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 19: Just Below Review Threshold (79) - should approve
run_test_case "Below Review Threshold - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_score_79",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_score_79",
        "ip_address": "192.168.1.113",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# =============================================================================
# Error Handling Tests
# =============================================================================

echo ""
log_info "Running Error Handling Tests..."
echo ""

# Test 20: Unknown User (fallback values) - should approve
run_test_case "Unknown User Fallback - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_unknown_12345",
        "amount": 100.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_unknown_12345",
        "ip_address": "192.168.1.200",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 21: Missing Optional Fields - should approve (graceful handling)
run_test_case "Missing Optional Fields - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_low_risk"
    }
}' "approve"

# Test 22: Unknown Event Type - should use default fallback
run_error_test_case "Unknown Event Type - Default Fallback" '{
    "event": {
        "type": "unknown_type",
        "user_id": "user_low_risk",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "default_fallback"

# Test 23: Empty User ID (fallback) - should approve
run_test_case "Empty User ID Fallback - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "",
        "amount": 100.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_00001",
        "ip_address": "192.168.1.201",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# =============================================================================
# Feature Combination Tests
# =============================================================================

echo ""
log_info "Running Feature Combination Tests..."
echo ""

# Test 24: Multiple Risk Factors - should decline
run_test_case "Multiple Risk Factors - Decline" '{
    "event": {
        "type": "transaction",
        "user_id": "user_multi_risk",
        "amount": 1000.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_multi_risk",
        "ip_address": "192.168.1.120",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 25: High Risk User with Low Risk Device - should review
run_test_case "High User Low Device - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_high_risk",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_low_risk",
        "ip_address": "192.168.1.121",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "decline"

# Test 26: Low Risk User with High Risk Device - should review
run_test_case "Low User High Device - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_low_risk",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_high_risk",
        "ip_address": "192.168.1.122",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 27: Good User, Good Device, Bad IP - should review
run_test_case "Good User/Device Bad IP - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_low_risk",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_low_risk",
        "ip_address": "192.168.1.104",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 28: New Account with Low Amount - should approve
run_test_case "New Account Low Amount - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_new_account",
        "amount": 100.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_new_account",
        "ip_address": "192.168.1.105",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# =============================================================================
# Velocity and Pattern Tests
# =============================================================================

echo ""
log_info "Running Velocity and Pattern Tests..."
echo ""

# Test 29: High Velocity with Medium Risk - should review
run_test_case "High Velocity Medium Risk - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_velocity_medium",
        "amount": 400.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_velocity_medium",
        "ip_address": "192.168.1.130",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 30: Extreme Multi-Device (>10 devices) - should review
run_test_case "Extreme Multi-Device - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_extreme_devices",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_extreme_01",
        "ip_address": "192.168.1.131",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# =============================================================================
# Payment Method Tests
# =============================================================================

echo ""
log_info "Running Payment Method Tests..."
echo ""

# Test 31: Low Risk User with High Risk Payment Method - should review
run_test_case "Low Risk User Crypto - Review" '{
    "event": {
        "type": "payment",
        "user_id": "user_low_risk",
        "amount": 500.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_low_risk",
        "ip_address": "192.168.1.102",
        "payment_method": "crypto",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 32: Debit Card Payment - should approve
run_test_case "Debit Card Payment - Approve" '{
    "event": {
        "type": "payment",
        "user_id": "user_normal",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_normal",
        "ip_address": "192.168.1.108",
        "payment_method": "debit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# =============================================================================
# Login Velocity Tests
# =============================================================================

echo ""
log_info "Running Login Velocity Tests..."
echo ""

# Test 33: High Login Frequency - should review
run_test_case "High Login Frequency - Review" '{
    "event": {
        "type": "login",
        "user_id": "user_login_freq",
        "device_id": "device_login_freq",
        "ip_address": "192.168.1.140",
        "country": "US",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 34: Many Failed Logins - should review
run_test_case "Many Failed Logins - Review" '{
    "event": {
        "type": "login",
        "user_id": "user_failed_logins",
        "device_id": "device_failed_logins",
        "ip_address": "192.168.1.141",
        "country": "US",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# =============================================================================
# Merchant Risk Tests
# =============================================================================

echo ""
log_info "Running Merchant Risk Tests..."
echo ""

# Test 35: High Risk Merchant - should review
run_test_case "High Risk Merchant - Review" '{
    "event": {
        "type": "transaction",
        "user_id": "user_normal",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_normal",
        "ip_address": "192.168.1.108",
        "payment_method": "credit_card",
        "merchant_id": "merchant_003",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "review"

# Test 36: Low Risk Merchant - should approve
run_test_case "Low Risk Merchant - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_normal",
        "amount": 300.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_normal",
        "ip_address": "192.168.1.108",
        "payment_method": "credit_card",
        "merchant_id": "merchant_001",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# =============================================================================
# Zero and Null Value Tests
# =============================================================================

echo ""
log_info "Running Zero and Null Value Tests..."
echo ""

# Test 37: Zero Amount Transaction - should approve (edge case)
run_test_case "Zero Amount Transaction - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_normal",
        "amount": 0.00,
        "currency": "USD",
        "country": "US",
        "device_id": "device_normal",
        "ip_address": "192.168.1.108",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

# Test 38: Very Small Amount (1 cent) - should approve
run_test_case "Micro Transaction - Approve" '{
    "event": {
        "type": "transaction",
        "user_id": "user_normal",
        "amount": 0.01,
        "currency": "USD",
        "country": "US",
        "device_id": "device_normal",
        "ip_address": "192.168.1.108",
        "payment_method": "credit_card",
        "timestamp": "'"$CURRENT_TIME"'"
    }
}' "approve"

log_info "Redis test cases completed!"
echo ""
