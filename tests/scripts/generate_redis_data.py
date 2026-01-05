#!/usr/bin/env python3
"""
Generate test data for Redis feature store E2E tests.

This script populates Redis with pre-computed feature values for testing.
"""

import redis
import sys
from typing import Dict, Any


def get_redis_client(url: str = "redis://localhost:6379/1") -> redis.Redis:
    """Create Redis client connection."""
    return redis.from_url(url, decode_responses=True)


def generate_test_features() -> Dict[str, Any]:
    """Generate test feature data for various test scenarios."""

    features = {}

    # Test Case 1: High Risk User - should decline
    features["user_risk_score:user_high_risk"] = 95
    features["user_fraud_score:user_high_risk"] = 85
    features["device_reputation:device_high_risk"] = 20
    features["device_fraud_count:device_high_risk"] = 10
    features["ip_reputation:192.168.1.100"] = 25
    features["ip_fraud_rate:192.168.1.100"] = 0.5
    features["user_avg_txn_amt:user_high_risk"] = 500
    features["user_txn_count_7d:user_high_risk"] = 30
    features["user_unique_devices:user_high_risk"] = 8
    features["user_unique_countries:user_high_risk"] = 5
    features["user_account_age:user_high_risk"] = 180
    features["user_total_txns:user_high_risk"] = 200

    # Test Case 2: Medium Risk User - should review
    features["user_risk_score:user_medium_risk"] = 65
    features["user_fraud_score:user_medium_risk"] = 45
    features["device_reputation:device_medium_risk"] = 32  # Just above suspicious threshold
    features["device_fraud_count:device_medium_risk"] = 3
    features["ip_reputation:192.168.1.101"] = 55
    features["ip_fraud_rate:192.168.1.101"] = 0.31  # Triggers risky_ip (80 points)
    features["user_avg_txn_amt:user_medium_risk"] = 800
    features["user_txn_count_7d:user_medium_risk"] = 20
    features["user_unique_devices:user_medium_risk"] = 4
    features["user_unique_countries:user_medium_risk"] = 2
    features["user_account_age:user_medium_risk"] = 90
    features["user_total_txns:user_medium_risk"] = 100
    # IP for crypto payment test - no risky ip trigger
    features["ip_reputation:192.168.1.115"] = 65
    features["ip_fraud_rate:192.168.1.115"] = 0.08

    # Test Case 3: Low Risk User - should approve
    features["user_risk_score:user_low_risk"] = 30
    features["user_fraud_score:user_low_risk"] = 10
    features["device_reputation:device_low_risk"] = 85
    features["device_fraud_count:device_low_risk"] = 0
    features["ip_reputation:192.168.1.102"] = 90
    features["ip_fraud_rate:192.168.1.102"] = 0.01
    features["user_avg_txn_amt:user_low_risk"] = 300
    features["user_txn_count_7d:user_low_risk"] = 5
    features["user_unique_devices:user_low_risk"] = 2
    features["user_unique_countries:user_low_risk"] = 1
    features["user_account_age:user_low_risk"] = 365
    features["user_total_txns:user_low_risk"] = 500
    # For crypto payment test - add payment fraud rate
    features["payment_fraud_rate:user_low_risk"] = 0.05

    # Test Case 4: Suspicious Device - should review
    features["user_risk_score:user_suspicious_device"] = 50
    features["user_fraud_score:user_suspicious_device"] = 30
    features["device_reputation:device_suspicious"] = 15
    features["device_fraud_count:device_suspicious"] = 8
    features["ip_reputation:192.168.1.103"] = 60
    features["ip_fraud_rate:192.168.1.103"] = 0.1
    features["user_avg_txn_amt:user_suspicious_device"] = 400
    features["user_txn_count_7d:user_suspicious_device"] = 10
    features["user_unique_devices:user_suspicious_device"] = 3
    features["user_unique_countries:user_suspicious_device"] = 1
    features["user_account_age:user_suspicious_device"] = 120
    features["user_total_txns:user_suspicious_device"] = 150

    # Test Case 5: Risky IP - should review
    features["user_risk_score:user_risky_ip"] = 62  # Triggers medium_risk_user (60 points)
    features["user_fraud_score:user_risky_ip"] = 25
    features["device_reputation:device_risky_ip"] = 70
    features["device_fraud_count:device_risky_ip"] = 1
    features["ip_reputation:192.168.1.104"] = 20  # Triggers risky_ip (70 points)
    features["ip_fraud_rate:192.168.1.104"] = 0.4  # Triggers risky_ip (70 points)
    features["user_avg_txn_amt:user_risky_ip"] = 600
    features["user_txn_count_7d:user_risky_ip"] = 8
    features["user_unique_devices:user_risky_ip"] = 2
    features["user_unique_countries:user_risky_ip"] = 1
    features["user_account_age:user_risky_ip"] = 200
    features["user_total_txns:user_risky_ip"] = 250

    # Test Case 6: New Account High Value - should review
    features["user_risk_score:user_new_account"] = 55
    features["user_fraud_score:user_new_account"] = 35
    features["device_reputation:device_new_account"] = 60
    features["device_fraud_count:device_new_account"] = 0
    features["ip_reputation:192.168.1.105"] = 65
    features["ip_fraud_rate:192.168.1.105"] = 0.05
    features["user_avg_txn_amt:user_new_account"] = 200
    features["user_txn_count_7d:user_new_account"] = 3
    features["user_unique_devices:user_new_account"] = 1
    features["user_unique_countries:user_new_account"] = 1
    features["user_account_age:user_new_account"] = 5
    features["user_total_txns:user_new_account"] = 3

    # Test Case 7: High Velocity User - should review
    features["user_risk_score:user_high_velocity"] = 60
    features["user_fraud_score:user_high_velocity"] = 40
    features["device_reputation:device_high_velocity"] = 28  # Triggers suspicious_device (80 points)
    features["device_fraud_count:device_high_velocity"] = 2
    features["ip_reputation:192.168.1.106"] = 70
    features["ip_fraud_rate:192.168.1.106"] = 0.08
    features["user_avg_txn_amt:user_high_velocity"] = 350
    features["user_txn_count_7d:user_high_velocity"] = 55  # Triggers high_velocity_user (60 points)
    features["user_unique_devices:user_high_velocity"] = 6  # Triggers high_velocity_user (60 points)
    features["user_unique_countries:user_high_velocity"] = 3
    features["user_account_age:user_high_velocity"] = 150
    features["user_total_txns:user_high_velocity"] = 300

    # Test Case 8: Multi-Device Pattern - should review
    features["user_risk_score:user_multi_device"] = 62  # Triggers medium_risk_user (60 points)
    features["user_fraud_score:user_multi_device"] = 38
    features["device_reputation:device_multi_01"] = 65
    features["device_fraud_count:device_multi_01"] = 1
    features["ip_reputation:192.168.1.107"] = 28  # Triggers risky_ip (70 points)
    features["ip_fraud_rate:192.168.1.107"] = 0.06
    features["user_avg_txn_amt:user_multi_device"] = 450
    features["user_txn_count_7d:user_multi_device"] = 25
    features["user_unique_devices:user_multi_device"] = 12  # Triggers multi_device_pattern (50 points)
    features["user_unique_countries:user_multi_device"] = 2
    features["user_account_age:user_multi_device"] = 100
    features["user_total_txns:user_multi_device"] = 180

    # Test Case 9: Normal User - should approve
    features["user_risk_score:user_normal"] = 40
    features["user_fraud_score:user_normal"] = 15
    features["device_reputation:device_normal"] = 80
    features["device_fraud_count:device_normal"] = 0
    features["ip_reputation:192.168.1.108"] = 85
    features["ip_fraud_rate:192.168.1.108"] = 0.02
    features["user_avg_txn_amt:user_normal"] = 250
    features["user_txn_count_7d:user_normal"] = 7
    features["user_unique_devices:user_normal"] = 2
    features["user_unique_countries:user_normal"] = 1
    features["user_account_age:user_normal"] = 300
    features["user_total_txns:user_normal"] = 400

    # Test Case 10: Established User - should approve
    features["user_risk_score:user_established"] = 25
    features["user_fraud_score:user_established"] = 5
    features["device_reputation:device_established"] = 95
    features["device_fraud_count:device_established"] = 0
    features["ip_reputation:192.168.1.109"] = 95
    features["ip_fraud_rate:192.168.1.109"] = 0.005
    features["user_avg_txn_amt:user_established"] = 500
    features["user_txn_count_7d:user_established"] = 10
    features["user_unique_devices:user_established"] = 2
    features["user_unique_countries:user_established"] = 1
    features["user_account_age:user_established"] = 730
    features["user_total_txns:user_established"] = 1000

    # =============================================================================
    # Boundary Test Cases
    # =============================================================================

    # Test 16: Boundary Risk Score (exactly 80) - should review
    features["user_risk_score:user_boundary_80"] = 81  # Triggers high_risk_user rule (score: 100)
    features["user_fraud_score:user_boundary_80"] = 0
    features["device_reputation:device_boundary_80"] = 70
    features["device_fraud_count:device_boundary_80"] = 0
    features["ip_reputation:192.168.1.110"] = 70
    features["ip_fraud_rate:192.168.1.110"] = 0.05
    features["user_avg_txn_amt:user_boundary_80"] = 400
    features["user_txn_count_7d:user_boundary_80"] = 10
    features["user_unique_devices:user_boundary_80"] = 3
    features["user_unique_countries:user_boundary_80"] = 1
    features["user_account_age:user_boundary_80"] = 100
    features["user_total_txns:user_boundary_80"] = 150

    # Test 17: Boundary Risk Score (exactly 200) - should decline
    features["user_risk_score:user_boundary_200"] = 81  # high_risk_user: 100
    features["user_fraud_score:user_boundary_200"] = 71  # high_fraud_score: 120, total=220
    features["device_reputation:device_boundary_200"] = 50
    features["device_fraud_count:device_boundary_200"] = 2
    features["ip_reputation:192.168.1.111"] = 50
    features["ip_fraud_rate:192.168.1.111"] = 0.1
    features["user_avg_txn_amt:user_boundary_200"] = 500
    features["user_txn_count_7d:user_boundary_200"] = 20
    features["user_unique_devices:user_boundary_200"] = 4
    features["user_unique_countries:user_boundary_200"] = 2
    features["user_account_age:user_boundary_200"] = 50
    features["user_total_txns:user_boundary_200"] = 80

    # Test 18: Just Below Decline Threshold (199) - should review
    features["user_risk_score:user_score_199"] = 81  # high_risk_user: 100
    features["user_fraud_score:user_score_199"] = 68  # high_fraud_score: 0, total=100
    features["device_reputation:device_score_199"] = 50
    features["device_fraud_count:device_score_199"] = 1
    features["ip_reputation:192.168.1.112"] = 50
    features["ip_fraud_rate:192.168.1.112"] = 0.08
    features["user_avg_txn_amt:user_score_199"] = 450
    features["user_txn_count_7d:user_score_199"] = 15
    features["user_unique_devices:user_score_199"] = 3
    features["user_unique_countries:user_score_199"] = 1
    features["user_account_age:user_score_199"] = 60
    features["user_total_txns:user_score_199"] = 100

    # Test 19: Just Below Review Threshold (79) - should approve
    features["user_risk_score:user_score_79"] = 79  # No high_risk_user trigger
    features["user_fraud_score:user_score_79"] = 40
    features["device_reputation:device_score_79"] = 70
    features["device_fraud_count:device_score_79"] = 0
    features["ip_reputation:192.168.1.113"] = 75
    features["ip_fraud_rate:192.168.1.113"] = 0.03
    features["user_avg_txn_amt:user_score_79"] = 400
    features["user_txn_count_7d:user_score_79"] = 8
    features["user_unique_devices:user_score_79"] = 2
    features["user_unique_countries:user_score_79"] = 1
    features["user_account_age:user_score_79"] = 200
    features["user_total_txns:user_score_79"] = 250

    # =============================================================================
    # Feature Combination Test Cases
    # =============================================================================

    # Test 24: Multiple Risk Factors - should decline
    features["user_risk_score:user_multi_risk"] = 85  # high_risk_user: 100
    features["user_fraud_score:user_multi_risk"] = 75  # high_fraud_score: 120
    features["device_reputation:device_multi_risk"] = 25  # suspicious_device: 80
    features["device_fraud_count:device_multi_risk"] = 6
    features["ip_reputation:192.168.1.120"] = 20  # risky_ip: 70, total >= 370
    features["ip_fraud_rate:192.168.1.120"] = 0.35
    features["user_avg_txn_amt:user_multi_risk"] = 800
    features["user_txn_count_7d:user_multi_risk"] = 30
    features["user_unique_devices:user_multi_risk"] = 7
    features["user_unique_countries:user_multi_risk"] = 4
    features["user_account_age:user_multi_risk"] = 40
    features["user_total_txns:user_multi_risk"] = 60

    # =============================================================================
    # Velocity and Pattern Test Cases
    # =============================================================================

    # Test 29: High Velocity with Medium Risk - should review
    features["user_risk_score:user_velocity_medium"] = 65
    features["user_fraud_score:user_velocity_medium"] = 50
    features["device_reputation:device_velocity_medium"] = 55
    features["device_fraud_count:device_velocity_medium"] = 2
    features["ip_reputation:192.168.1.130"] = 60
    features["ip_fraud_rate:192.168.1.130"] = 0.1
    features["user_avg_txn_amt:user_velocity_medium"] = 400
    features["user_txn_count_7d:user_velocity_medium"] = 55  # Triggers high_velocity_user
    features["user_unique_devices:user_velocity_medium"] = 6
    features["user_unique_countries:user_velocity_medium"] = 2
    features["user_account_age:user_velocity_medium"] = 120
    features["user_total_txns:user_velocity_medium"] = 200

    # Test 30: Extreme Multi-Device (>10 devices) - should review
    features["user_risk_score:user_extreme_devices"] = 62  # Triggers medium_risk_user (60 points)
    features["user_fraud_score:user_extreme_devices"] = 35
    features["device_reputation:device_extreme_01"] = 28  # Triggers suspicious_device (80 points)
    features["device_fraud_count:device_extreme_01"] = 1
    features["ip_reputation:192.168.1.131"] = 65
    features["ip_fraud_rate:192.168.1.131"] = 0.06
    features["user_avg_txn_amt:user_extreme_devices"] = 350
    features["user_txn_count_7d:user_extreme_devices"] = 20
    features["user_unique_devices:user_extreme_devices"] = 15  # Triggers multi_device_pattern (50 points)
    features["user_unique_countries:user_extreme_devices"] = 3
    features["user_account_age:user_extreme_devices"] = 90
    features["user_total_txns:user_extreme_devices"] = 150

    # =============================================================================
    # Login Velocity Test Cases
    # =============================================================================

    # Test 33: High Login Frequency - should review
    features["user_risk_score:user_login_freq"] = 62  # Triggers medium_risk_user (60 points)
    features["user_fraud_score:user_login_freq"] = 40
    features["device_reputation:device_login_freq"] = 32  # Just above suspicious threshold
    features["device_fraud_count:device_login_freq"] = 2
    features["ip_reputation:192.168.1.140"] = 60
    features["ip_fraud_rate:192.168.1.140"] = 0.08
    features["user_avg_txn_amt:user_login_freq"] = 400
    features["user_txn_count_7d:user_login_freq"] = 15
    features["user_unique_devices:user_login_freq"] = 3
    features["user_unique_countries:user_login_freq"] = 2
    features["user_account_age:user_login_freq"] = 100
    features["user_total_txns:user_login_freq"] = 150
    features["user_login_1h:user_login_freq"] = 15  # Triggers high_login_frequency (60 points)
    features["user_failed_login_24h:user_login_freq"] = 2

    # Test 34: Many Failed Logins - should review
    features["user_risk_score:user_failed_logins"] = 62
    features["user_fraud_score:user_failed_logins"] = 42
    features["device_reputation:device_failed_logins"] = 50
    features["device_fraud_count:device_failed_logins"] = 3
    features["ip_reputation:192.168.1.141"] = 55
    features["ip_fraud_rate:192.168.1.141"] = 0.09
    features["user_avg_txn_amt:user_failed_logins"] = 450
    features["user_txn_count_7d:user_failed_logins"] = 12
    features["user_unique_devices:user_failed_logins"] = 3
    features["user_unique_countries:user_failed_logins"] = 2
    features["user_account_age:user_failed_logins"] = 110
    features["user_total_txns:user_failed_logins"] = 160
    features["user_login_1h:user_failed_logins"] = 3
    features["user_failed_login_24h:user_failed_logins"] = 12

    # Payment method features
    features["payment_fraud_rate:credit_card"] = 0.05
    features["payment_fraud_rate:debit_card"] = 0.03
    features["payment_fraud_rate:crypto"] = 0.35
    features["payment_fraud_rate:bank_transfer"] = 0.02

    # Merchant features
    features["merchant_risk:merchant_001"] = 30
    features["merchant_risk:merchant_002"] = 50
    features["merchant_risk:merchant_003"] = 85
    features["merchant_risk:merchant_high_risk"] = 85

    # Login features
    features["user_login_1h:user_high_risk"] = 5
    features["user_failed_login_24h:user_high_risk"] = 8
    features["user_login_1h:user_low_risk"] = 1
    features["user_failed_login_24h:user_low_risk"] = 0

    return features


def load_features_to_redis(client: redis.Redis, features: Dict[str, Any], namespace: str = "e2e_features"):
    """Load features into Redis with namespace prefix."""

    pipeline = client.pipeline()
    loaded_count = 0

    for key, value in features.items():
        full_key = f"{namespace}:{key}"
        pipeline.set(full_key, value)
        # Set TTL to 1 hour
        pipeline.expire(full_key, 3600)
        loaded_count += 1

    pipeline.execute()

    return loaded_count


def clear_test_data(client: redis.Redis, namespace: str = "e2e_features"):
    """Clear all test data from Redis."""

    pattern = f"{namespace}:*"
    keys = client.keys(pattern)

    if keys:
        client.delete(*keys)
        return len(keys)

    return 0


def main():
    """Main function to generate and load Redis test data."""

    # Get Redis URL from environment or use default
    import os
    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379/1")

    print("=" * 60)
    print("CORINT Redis E2E Test Data Generator")
    print("=" * 60)
    print()

    try:
        # Connect to Redis
        print(f"Connecting to Redis: {redis_url}")
        client = get_redis_client(redis_url)

        # Test connection
        client.ping()
        print("✓ Redis connection successful")
        print()

        # Clear old test data
        print("Clearing old test data...")
        cleared = clear_test_data(client)
        print(f"✓ Cleared {cleared} old keys")
        print()

        # Generate test features
        print("Generating test features...")
        features = generate_test_features()
        print(f"✓ Generated {len(features)} feature values")
        print()

        # Load features to Redis
        print("Loading features to Redis...")
        loaded = load_features_to_redis(client, features)
        print(f"✓ Loaded {loaded} features to Redis")
        print()

        # Verify data
        print("Verifying data...")
        sample_key = "e2e_features:user_risk_score:user_high_risk"
        sample_value = client.get(sample_key)
        print(f"✓ Sample verification: {sample_key} = {sample_value}")
        print()

        print("=" * 60)
        print("✓ Redis test data generation completed!")
        print("=" * 60)

        return 0

    except redis.ConnectionError as e:
        print(f"✗ Redis connection failed: {e}", file=sys.stderr)
        print()
        print("Please ensure Redis is running:", file=sys.stderr)
        print("  docker run -d -p 6379:6379 redis:latest", file=sys.stderr)
        print("  or", file=sys.stderr)
        print("  redis-server", file=sys.stderr)
        return 1

    except Exception as e:
        print(f"✗ Error: {e}", file=sys.stderr)
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
