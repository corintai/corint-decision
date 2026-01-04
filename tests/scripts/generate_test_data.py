#!/usr/bin/env python3
"""
CORINT Decision Engine - E2E Test Data Generator

Generates SQL INSERT statements with diverse event data using relative timestamps
to avoid time window expiration issues.
"""

import random
import json
from datetime import datetime, timedelta
from pathlib import Path

# SQL output file path
SQL_OUTPUT = Path(__file__).parent.parent / "data" / "test_data.sql"

# User pools
NORMAL_USERS = [f"user_{i:04d}" for i in range(1, 51)]  # 50 normal users
SUSPICIOUS_USERS = [f"sus_{i:04d}" for i in range(1, 11)]  # 10 suspicious users
VIP_USERS = [f"vip_{i:04d}" for i in range(1, 6)]  # 5 VIP users

# Test-specific users that should NOT be included in random generation
# These users are reserved for specific test cases
TEST_SPECIFIC_USERS = {
    "user_0001", "user_0002", "user_0005", "user_0100", "user_0102", "user_0103",
    "user_0104", "user_0105", "user_0106", "user_9997", "user_9998", "user_9999",
    "user_acceleration", "user_active_block", "user_avg_baseline", "user_boundary_1",
    "user_boundary_2", "user_boundary_3", "user_concentration", "user_device_high",
    "user_device_velocity", "user_expired_block", "user_failure_rate", "user_file_test_1",
    "user_file_test_2", "user_high_total_spending", "user_low_login", "user_low_weekly_activity",
    "user_max_history", "user_micro_pattern", "user_multi_1", "user_multi_device_24h",
    "user_multi_ip", "user_new_highvalue", "user_payment_freq", "user_payment_max",
    "user_payment_ratio", "user_payment_weekly", "user_recent_spike", "user_velocity_24h",
    "user_velocity_test", "user_vip_001", "user_wide_range",
    # Blocked users (used in Test 2 and database list tests)
    "sus_0001", "sus_0002", "sus_0003", "sus_0004", "sus_0005"
}

# Other data pools
MERCHANTS = [f"merchant_{i:03d}" for i in range(1, 21)]
# Use isolated random device pool to avoid collisions with test fixtures (device_00001, device_shared, etc.)
DEVICES = [f"rnd_device_{i:05d}" for i in range(1, 101)]
RANDOM_DEVICES = [f"rnd_device_{i:05d}" for i in range(1, 151)]
COUNTRIES = ["US", "GB", "CA", "AU", "DE", "FR", "JP", "SG", "BR", "IN"]
SUSPICIOUS_COUNTRIES = ["NG", "RU", "CN", "KP"]
# Keep random IPs in 10.200.x.x to avoid overlapping test fixture IPs (192.168.x.x)
IPS = [f"10.200.{random.randint(1, 254)}.{random.randint(1, 254)}" for _ in range(100)]
SUSPICIOUS_IPS = ["45.142.212.61", "185.220.101.52", "91.109.190.28"]

# Event types
EVENT_TYPES = ["transaction", "login", "payment", "withdrawal", "transfer"]


def generate_schema_sql():
    """Generate SQL statements to create database schema

    Schema matches runtime expectations:
    - Keep key fields as direct columns so aggregations do not depend on JSON extraction
    - Still store attributes JSON for reference/debugging
    """
    return """-- Create events table
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    status TEXT,  -- Direct column for status (used in WHERE clauses)
    amount REAL,
    currency TEXT,
    merchant_id TEXT,
    device_id TEXT,
    ip_address TEXT,
    country TEXT,
    email TEXT,
    phone TEXT,
    metadata TEXT,
    attributes TEXT  -- JSON column containing event fields like amount, currency, merchant_id, etc.
);

-- Create indexes for events
CREATE INDEX IF NOT EXISTS idx_user_id ON events(user_id);
CREATE INDEX IF NOT EXISTS idx_timestamp ON events(timestamp);
CREATE INDEX IF NOT EXISTS idx_event_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_user_timestamp ON events(user_id, timestamp);

-- Create list_entries table for database-backed lists
CREATE TABLE IF NOT EXISTS list_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    list_id TEXT NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TEXT,
    metadata TEXT
);

-- Create indexes for list_entries
CREATE INDEX IF NOT EXISTS idx_list_id ON list_entries(list_id);
CREATE INDEX IF NOT EXISTS idx_list_value ON list_entries(list_id, value);
CREATE INDEX IF NOT EXISTS idx_expires_at ON list_entries(expires_at);

-- Clear existing data
DELETE FROM events;
DELETE FROM list_entries;
"""


def generate_timestamp(hours_ago=0, days_ago=0, minutes_ago=0):
    """Generate timestamp relative to current UTC time

    IMPORTANT: Use UTC to match the test runner which uses `date -u`
    """
    from datetime import timezone
    now = datetime.now(timezone.utc)
    offset = timedelta(hours=hours_ago, days=days_ago, minutes=minutes_ago)
    # Remove timezone info for isoformat to match expected format
    return (now - offset).replace(tzinfo=None).isoformat()


def escape_sql_string(s):
    """Escape single quotes in SQL string"""
    if s is None:
        return "NULL"
    return f"'{str(s).replace(chr(39), chr(39)+chr(39))}'"

def generate_insert_statements(events):
    """Generate SQL INSERT statements from events

    Writes both direct columns (for feature queries) and attributes JSON.
    Status is stored as a direct column and excluded from attributes.
    """
    import json
    sql_statements = []

    for event in events:
        event_type, user_id, timestamp, amount, currency, merchant_id, \
        device_id, ip_address, country, email, phone, status, metadata = event

        # Build attributes JSON containing event fields (excluding status which is a direct column)
        attributes = {}
        if amount is not None:
            attributes["amount"] = amount
        if currency:
            attributes["currency"] = currency
        if merchant_id:
            attributes["merchant_id"] = merchant_id
        if device_id:
            attributes["device_id"] = device_id
        if ip_address:
            attributes["ip_address"] = ip_address
        if country:
            attributes["country"] = country
        if email:
            attributes["email"] = email
        if phone:
            attributes["phone"] = phone
        # Note: status is NOT included in attributes - it's a direct column
        if metadata:
            attributes["metadata"] = metadata

        # Convert attributes dict to JSON string
        attributes_json = json.dumps(attributes) if attributes else "{}"

        # Format values
        values = [
            escape_sql_string(event_type),
            escape_sql_string(user_id),
            escape_sql_string(timestamp),
            escape_sql_string(status),  # Direct column
            escape_sql_string(amount),
            escape_sql_string(currency),
            escape_sql_string(merchant_id),
            escape_sql_string(device_id),
            escape_sql_string(ip_address),
            escape_sql_string(country),
            escape_sql_string(email),
            escape_sql_string(phone),
            escape_sql_string(metadata),
            escape_sql_string(attributes_json)
        ]

        sql = (
            "INSERT INTO events (event_type, user_id, timestamp, status, amount, currency, "
            "merchant_id, device_id, ip_address, country, email, phone, metadata, attributes) "
            f"VALUES ({', '.join(values)});"
        )
        sql_statements.append(sql)

    return sql_statements


def generate_normal_transactions(count=100):
    """Generate normal transaction events

    Excludes test-specific users to prevent cross-contamination.
    """
    events = []
    now = datetime.now()

    # Filter out test-specific users from random generation
    available_users = [u for u in (NORMAL_USERS + VIP_USERS) if u not in TEST_SPECIFIC_USERS]

    for i in range(count):
        user = random.choice(available_users)
        # Distribute events across last 30 days
        hours_ago = random.uniform(0, 720)  # 0-30 days

        metadata = {
            "payment_method": random.choice(["credit_card", "debit_card", "bank_transfer"]),
            "merchant_category": random.choice(["retail", "food", "travel", "online"]),
            "is_vip": user in VIP_USERS
        }

        events.append((
            "transaction",
            user,
            generate_timestamp(hours_ago=hours_ago),
            round(random.uniform(10, 500), 2),
            "USD",
            random.choice(MERCHANTS),
            random.choice(RANDOM_DEVICES),
            random.choice(IPS),
            random.choice(COUNTRIES),
            f"{user}@example.com",
            f"+1555{random.randint(1000000, 9999999)}",
            "completed",
            json.dumps(metadata)
        ))

    return events


def generate_suspicious_transactions(count=20):
    """Generate suspicious transaction events

    Excludes test-specific users to prevent cross-contamination.
    """
    events = []

    # Filter out test-specific users from random generation
    available_users = [u for u in SUSPICIOUS_USERS if u not in TEST_SPECIFIC_USERS]

    for i in range(count):
        user = random.choice(available_users)
        # Recent suspicious activity
        hours_ago = random.uniform(0, 48)  # Last 2 days

        metadata = {
            "payment_method": random.choice(["crypto", "wire_transfer"]),
            "merchant_category": "suspicious",
            "high_risk": True,
            "new_recipient": True
        }

        events.append((
            "transaction",
            user,
            generate_timestamp(hours_ago=hours_ago),
            round(random.uniform(5000, 15000), 2),
            "USD",
            random.choice(MERCHANTS),
            random.choice(RANDOM_DEVICES),
            random.choice(SUSPICIOUS_IPS),
            random.choice(SUSPICIOUS_COUNTRIES),
            f"{user}@tempmail.com",
            None,
            "pending",
            json.dumps(metadata)
        ))

    return events


def generate_velocity_abuse(count=30):
    """Generate velocity abuse pattern - same user, multiple transactions in short time

    Excludes test-specific users to prevent cross-contamination.
    """
    events = []

    # Filter out test-specific users from random generation
    available_users = [u for u in SUSPICIOUS_USERS if u not in TEST_SPECIFIC_USERS]

    # Pick a few users for velocity abuse
    for user in random.sample(available_users, min(3, len(available_users))):
        base_time = datetime.now()

        # Generate 10 transactions within 1 hour
        for i in range(10):
            minutes_ago = random.uniform(0, 60)

            metadata = {
                "payment_method": "credit_card",
                "velocity_pattern": True
            }

            events.append((
                "transaction",
                user,
                generate_timestamp(minutes_ago=minutes_ago),
                round(random.uniform(100, 500), 2),
                "USD",
                random.choice(MERCHANTS),
                random.choice(RANDOM_DEVICES),
                random.choice(IPS),
                random.choice(COUNTRIES),
                f"{user}@example.com",
                None,
                "completed",
                json.dumps(metadata)
            ))

    return events


def generate_login_events(count=200):
    """Generate login events

    Excludes test-specific users to prevent cross-contamination.
    """
    events = []

    # Filter out test-specific users from random generation
    available_users = [u for u in (NORMAL_USERS + SUSPICIOUS_USERS + VIP_USERS) if u not in TEST_SPECIFIC_USERS]

    for i in range(count):
        user = random.choice(available_users)
        hours_ago = random.uniform(0, 168)  # Last week

        # Suspicious logins have more failed attempts
        is_suspicious = user in SUSPICIOUS_USERS
        success_rate = 0.5 if is_suspicious else 0.95

        metadata = {
            "login_method": random.choice(["password", "oauth", "sso"]),
            "failed_attempts": random.randint(0, 5) if is_suspicious else random.randint(0, 1),
            "user_agent": "Mozilla/5.0"
        }

        events.append((
            "login",
            user,
            generate_timestamp(hours_ago=hours_ago),
            None,
            None,
            None,
            random.choice(RANDOM_DEVICES),
            random.choice(SUSPICIOUS_IPS if is_suspicious else IPS),
            random.choice(SUSPICIOUS_COUNTRIES if is_suspicious else COUNTRIES),
            f"{user}@example.com",
            None,
            "success" if random.random() < success_rate else "failed",
            json.dumps(metadata)
        ))

    return events


def generate_payment_events(count=80):
    """Generate payment events

    Excludes test-specific users to prevent cross-contamination.
    """
    events = []

    # Filter out test-specific users from random generation
    available_users = [u for u in (NORMAL_USERS + VIP_USERS) if u not in TEST_SPECIFIC_USERS]

    for i in range(count):
        user = random.choice(available_users)
        hours_ago = random.uniform(0, 240)  # Last 10 days

        amount = random.uniform(50, 2000) if user in VIP_USERS else random.uniform(10, 500)

        metadata = {
            "payment_type": random.choice(["subscription", "one_time", "recurring"]),
            "card_type": random.choice(["visa", "mastercard", "amex"]),
            "card_last4": f"{random.randint(1000, 9999)}"
        }

        events.append((
            "payment",
            user,
            generate_timestamp(hours_ago=hours_ago),
            round(amount, 2),
            random.choice(["USD", "EUR", "GBP"]),
            random.choice(MERCHANTS),
            random.choice(RANDOM_DEVICES),
            random.choice(IPS),
            random.choice(COUNTRIES),
            f"{user}@example.com",
            None,
            "completed",
            json.dumps(metadata)
        ))

    return events


def generate_account_takeover_pattern(count=15):
    """Generate account takeover pattern - location change, device change

    Excludes test-specific users to prevent cross-contamination.
    """
    events = []

    # Filter out test-specific users from random generation
    available_users = [u for u in NORMAL_USERS if u not in TEST_SPECIFIC_USERS]

    # Use dedicated device names for each user to avoid collisions
    for idx, user in enumerate(random.sample(available_users, 5)):
        normal_device = f"device_takeover_normal_{idx:03d}"
        suspicious_device = f"device_takeover_sus_{idx:03d}"

        # Normal login
        events.append((
            "login",
            user,
            generate_timestamp(hours_ago=12),
            None,
            None,
            None,
            normal_device,
            IPS[0],
            "US",
            f"{user}@example.com",
            None,
            "success",
            json.dumps({"normal_activity": True})
        ))

        # Suspicious login from different location shortly after
        events.append((
            "login",
            user,
            generate_timestamp(hours_ago=11.5),
            None,
            None,
            None,
            suspicious_device,
            SUSPICIOUS_IPS[0],
            random.choice(SUSPICIOUS_COUNTRIES),
            f"{user}@example.com",
            None,
            "success",
            json.dumps({"possible_takeover": True, "location_change": True})
        ))

        # High-value transaction from suspicious location
        events.append((
            "transaction",
            user,
            generate_timestamp(hours_ago=11),
            round(random.uniform(8000, 15000), 2),
            "USD",
            random.choice(MERCHANTS),
            suspicious_device,
            SUSPICIOUS_IPS[0],
            random.choice(SUSPICIOUS_COUNTRIES),
            f"{user}@example.com",
            None,
            "pending",
            json.dumps({"high_risk": True, "account_takeover_pattern": True})
        ))

    return events


def generate_failed_login_history():
    """Generate failed login history for brute force detection testing (Test 17)"""
    events = []

    # Generate 10 failed logins for user_0105 within last 24 hours
    # This will trigger excessive_failures rule (>= 5 failed logins)
    user_id = "user_0105"
    for i in range(10):
        hours_ago = random.uniform(0, 23)  # Within last 24 hours

        events.append((
            "login",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            None,  # amount
            None,  # currency
            None,  # merchant_id
            random.choice(RANDOM_DEVICES),
            random.choice(IPS),
            random.choice(COUNTRIES),
            f"{user_id}@example.com",
            None,  # phone
            "failed",  # Critical: status is failed
            json.dumps({
                "login_method": "password",
                "failed_attempts": i + 1,
                "user_agent": "Mozilla/5.0",
                "brute_force_test": True
            })
        ))

    # Also generate some failed logins for other suspicious users
    # Filter out test-specific users
    available_suspicious = [u for u in SUSPICIOUS_USERS if u not in TEST_SPECIFIC_USERS]
    for user in random.sample(available_suspicious, min(3, len(available_suspicious))):
        for i in range(6):  # 6 failed logins each
            hours_ago = random.uniform(0, 23)
            events.append((
                "login",
                user,
                generate_timestamp(hours_ago=hours_ago),
                None, None, None,
                random.choice(RANDOM_DEVICES),
                random.choice(SUSPICIOUS_IPS),
                random.choice(SUSPICIOUS_COUNTRIES),
                f"{user}@example.com",
                None,
                "failed",
                json.dumps({
                    "login_method": "password",
                    "failed_attempts": random.randint(1, 5),
                    "user_agent": "Mozilla/5.0"
                })
            ))

    return events


def generate_high_frequency_transactions():
    """Generate high frequency transactions for velocity detection (Test 13)

    Design for decline (score >= 150):
    - high_frequency_1h (>= 5): score 70
    - high_frequency_24h (>= 15): score 50
    - velocity_spike (txn_velocity_1h_to_24h > 10): score 90
    Total: 70 + 50 + 90 = 210 → decline

    Strategy: 14 txns in 1h, 24 total in 24h
    - txn_count_1h = 14
    - txn_count_24h = 24
    - velocity = (14/24) * 24 = 14 > 10 ✓
    """
    events = []

    user_id = "user_velocity_24h"

    # 14 transactions within last hour (deterministic)
    for i in range(14):
        minutes_ago = (i * 4) + 2  # 2, 6, 10, ... 54 minutes ago

        events.append((
            "transaction",
            user_id,
            generate_timestamp(minutes_ago=minutes_ago),
            100.0 + (i * 20.0),  # Deterministic amounts: 100, 120, 140, ..., 360
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.1.{i+1}",
            "US",  # Fixed country
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({
                "payment_method": "credit_card",
                "velocity_test": True,
                "high_frequency": True
            })
        ))

    # 10 more transactions outside 1h window but within 24h
    for i in range(10):
        hours_ago = 2 + (i * 2)  # 2, 4, 6, ... 20 hours ago

        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            100.0 + (i * 20.0),  # Deterministic amounts
            "USD",
            f"merchant_{14+i:03d}",
            f"rnd_device_{14+i:05d}",
            f"192.168.1.{15+i}",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({
                "payment_method": "credit_card",
                "velocity_test": True
            })
        ))

    # Also generate high frequency transactions for another test user
    # Generate 8 transactions within 1 hour for 1h frequency test
    user_id_1h = "user_velocity_1h"
    for i in range(8):
        minutes_ago = random.uniform(0, 55)  # Within last hour

        events.append((
            "transaction",
            user_id_1h,
            generate_timestamp(minutes_ago=minutes_ago),
            round(random.uniform(50, 200), 2),
            "USD",
            random.choice(MERCHANTS),
            random.choice(RANDOM_DEVICES),
            random.choice(IPS),
            "US",  # Fixed country to avoid multiple_countries rule
            f"{user_id_1h}@example.com",
            None,
            "completed",
            json.dumps({
                "payment_method": "credit_card",
                "velocity_1h_test": True
            })
        ))

    return events


def generate_vip_user_history():
    """Generate VIP user historical transaction data (Test 12)

    Design:
    - Keep total spending under 50k to avoid triggering unusual_total_spending
    - Use consistent high amounts (6000-8000) to establish pattern
    - Test expects 12000 transaction to be approved
    - Only exceeds_max_history should trigger (12000 > 8000 * 1.5 = 12000), score 70 < 80 threshold
    """
    events = []

    vip_user = "user_vip_001"

    # Generate 6 transactions over 30 days with consistent high amounts
    # Total: 6 * 7250 avg = ~43.5k (under 50k threshold)
    # Max: 8000, so test 12000 = 8000*1.5 → does NOT trigger exceeds_max_history
    # Need 3+ in last 7 days to avoid low_weekly_activity (60) which would push to review
    for i in range(6):
        # First 3 transactions in last 6 days, next 3 spread over remaining 24 days
        days_ago = i + 1 if i < 3 else ((i - 3) * 8) + 7  # 1,2,3,7,15,23 days ago

        events.append((
            "transaction",
            vip_user,
            generate_timestamp(days_ago=days_ago),
            6000.0 + (i * 400.0),  # 6000, 6400, 6800, 7200, 7600, 8000 (deterministic, max=8000)
            "USD",
            f"merchant_{i:03d}",
            "device_vip_001",  # Consistent device
            f"192.168.1.{100+i}",
            "US",  # Consistent country
            f"{vip_user}@example.com",
            "+15551234567",
            "completed",
            json.dumps({
                "payment_method": "credit_card",
                "vip_status": True,
                "verified": True,
                "account_age_days": 1000
            })
        ))

    return events


def generate_crypto_payment_history():
    """Generate crypto payment history for crypto payment tests (Test 16)

    Rule: payment_method == "crypto" AND amount > 1000 → score 100 (review)

    Design:
    - 5 historical crypto payments with amounts 1500-1800
    - Max = 1800, so test 3000 > 1800*2 (3600) is false (avoids max_payment_exceeded)
    - Test amount: 3000 (> 1000) triggers crypto_payment_risk
    - Also add 2 transaction events in 24h to avoid payment_dominance rule
      (payment_to_txn_ratio = payment_count_24h / txn_count_24h)
      With 1 payment (test) and 2 transactions, ratio = 1/2 = 0.5 < 0.8
    Expected: score = 100 → review
    """
    events = []

    user_id = "user_0104"

    # Generate 5 crypto payments with deterministic amounts
    base_amounts = [1500.0, 1550.0, 1600.0, 1700.0, 1800.0]  # Max = 1800
    for i in range(5):
        hours_ago = 24 + (i * 48)  # 24, 72, 120, 168, 216 hours ago (deterministic)

        events.append((
            "payment",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            base_amounts[i],  # Deterministic amounts
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.1.{100+i}",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({
                "payment_method": "crypto",
                "payment_type": "one_time",
                "crypto_type": ["BTC", "ETH", "USDT", "BTC", "ETH"][i]  # Deterministic
            })
        ))

    # Add 2 transaction events in 24h to avoid payment_dominance rule
    # This ensures payment_to_txn_ratio = 1 / (2 + 0.0001) ≈ 0.5 < 0.8
    for i in range(2):
        hours_ago = 6 + (i * 6)  # 6 and 12 hours ago (within 24h)
        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            500.0 + (i * 100),  # 500, 600 - moderate amounts
            "USD",
            f"merchant_txn_{i:03d}",
            f"device_0104",
            "192.168.1.50",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            None
        ))

    return events


# =============================================================================
# New Feature Coverage Test Data Generators (Test 34-54)
# =============================================================================

def generate_low_weekly_activity_history():
    """Generate low weekly activity history for Test 34 (tests txn_count_7d)

    Goal: ONLY trigger low_weekly_activity (score 60) → review

    Rules to trigger:
    - low_weekly_activity: txn_count_7d < 3 AND amount > 2000 → score 60

    Rules to avoid:
    - high_value_new_user: amount > 1000 AND total_txn < 5 → Need >= 5 total txns
    - velocity_spike: txn_velocity_1h_to_24h > 10 → No txns in 1h window
    - amount_spike: ratio > 3.0 → Keep avg high
    - exceeds_max_history: amount > max * 1.5 → Keep max >= 1700
    - amount_concentration: concentration > 0.8 → Keep 24h total high

    Design:
    - 6 total transactions (>= 5 to avoid high_value_new_user)
    - 2 in 7-day window (< 3 to trigger low_weekly_activity)
    - No transactions in 1h window (avoid velocity_spike)
    - 24h amounts sum to > 2125 (avoid concentration)
    - Historical max >= 1700 (avoid exceeds_max with 2500 test amount)
    - High avg to keep ratio < 3.0

    Expected: 60 → review
    """
    events = []
    user_id = "user_low_weekly_activity"

    # Transaction 1: 3 hours ago (within 24h AND 7d, well outside 1h)
    events.append((
        "transaction",
        user_id,
        generate_timestamp(hours_ago=3),
        1300.0,
        "USD",
        "merchant_001",
        "device_lwact_001",
        "192.168.1.10",
        "US",
        f"{user_id}@example.com",
        None,
        None,
        None
    ))

    # Transaction 2: 8 hours ago (within 24h AND 7d)
    events.append((
        "transaction",
        user_id,
        generate_timestamp(hours_ago=8),
        1200.0,
        "USD",
        "merchant_002",
        "device_lwact_001",
        "192.168.1.10",
        "US",
        f"{user_id}@example.com",
        None,
        None,
        None
    ))

    # Transactions 3-6: Outside 7d but within 90d (for total count >= 5)
    for i, days in enumerate([10, 20, 30, 50]):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days),
            2000.0,
            "USD",
            f"merchant_{i+3:03d}",
            f"device_lwact_{i+2:03d}",
            "192.168.1.10",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_high_total_spending_history():
    """Generate high total spending history for Test 35 (tests user_total_amount)

    Rules to avoid:
    - unusual_total_spending: user_total_amount > 50000 AND amount > 5000 → score 80
    - low_weekly_activity: txn_count_7d < 3 AND amount > 2000 → score 60
    - exceeds_max_history: amount > max * 1.5 → score 70

    Feature: user_total_amount = SUM(amount) WHERE days <= 90

    Design:
    - Total spending in 90-day window: 24 * 2000 = 48000 (< 50000, avoids unusual_total_spending)
    - Add 3 older transactions (95-100 days ago) outside 90d window
    - Place 4 transactions in last 7 days (>= 3, avoids low_weekly_activity)
    - One high-value transaction at 4000 to set max high enough
    - Test amount: 6000
    - Max check: 6000 > 4000 * 1.5 (6000) → FALSE (just at boundary, doesn't trigger)

    Expected: No rules trigger → approve
    """
    events = []
    user_id = "user_high_total_spending"

    # First, add 3 old transactions outside 90-day window (won't count toward total)
    for i in range(3):
        days_ago = 95 + i * 2  # 95, 97, 99 days ago
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            2000.0,
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    # Add 4 recent transactions in last 7 days (for txn_count_7d >= 3)
    recent_days = [1, 2, 4, 6]
    for i, days_ago in enumerate(recent_days):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            2000.0,
            "USD",
            f"merchant_{i + 10:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    # Add 1 high-value transaction to set max to 4000
    events.append((
        "transaction",
        user_id,
        generate_timestamp(days_ago=10),
        4000.0,
        "USD",
        f"merchant_020",
        f"device_high_spending",  # Dedicated device
        "192.168.1.100",
        "US",
        f"{user_id}@example.com",
        None,
        None,
        None
    ))

    # Add remaining transactions distributed in 90-day window
    # 24 total - 4 recent - 1 high-value = 19 more transactions
    for i in range(19):
        days_ago = 15 + i * 4  # 15, 19, 23, ..., 87 days ago
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            2000.0,
            "USD",
            f"merchant_{i + 30 % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_avg_baseline_history():
    """Generate average baseline history for Test 36 (tests avg_transaction_amount)

    Rule: amount > avg_transaction_amount * 5 → score 80 (review)
    Feature: AVG(amount) WHERE days <= 30

    Design:
    - 50 transactions, each exactly 50
    - Days: spread across 30 days (some days have multiple)
    - Baseline average: 50
    - Test event: 1100
    - Rule checks: 1100 > 50 * 5 (250) ✓ with huge margin
    - Even if current event included: avg = (50*50 + 1100)/51 = 70.6
    - Then check: 1100 > 70.6 * 5 (353) ✓ still huge margin
    Expected: score = 80 → review
    """
    events = []
    user_id = "user_avg_baseline"

    # 50 transactions @ 50 each, distributed across 30 days
    for i in range(50):
        days_ago = (i % 30) + 1  # Cycle through days 1-30
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            50.0,  # Very small baseline for large margin
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_max_history_baseline():
    """Generate max history baseline for Test 37 (tests max_transaction_amount)

    Rules to trigger:
    - exceeds_max_history: amount > max_transaction_amount * 1.5 → score 70
    - high_value_new_user: amount > 1000 AND total_txn < 5 → score 80

    Feature: MAX(amount) WHERE days <= 30

    Design:
    - 4 transactions with amounts: 400, 600, 800, 1000
    - Days: 5, 10, 15, 20 (all within 30-day window)
    - Max: 1000
    - Test event: 1600
    - exceeds_max_history: 1600 > 1000 * 1.5 (1500) → TRUE → score 70
    - high_value_new_user: 1600 > 1000 AND 4 < 5 → TRUE → score 80
    - Total: 70 + 80 = 150 → decline

    Wait, that's too high. Let me use 3 transactions instead:
    - 3 transactions: 400, 700, 1000
    - Days: 10, 15, 20
    - Max: 1000
    - Test event: 1600
    - exceeds_max_history: 1600 > 1000 * 1.5 (1500) → TRUE → score 70
    - high_value_new_user: 1600 > 1000 AND 3 < 5 → TRUE → score 80
    - Total: 70 + 80 = 150 → decline

    Still too high. Actually, I need to be more careful. Let me just reduce transactions to get score >= 80 but < 150.
    The issue is that with total_txn < 5, high_value_new_user (score 80) will trigger.
    70 + 80 = 150 → decline (not review)

    So I need to trigger only exceeds_max_history + get the score to >=80 somehow.
    Or I can have exactly 5 total transactions, so high_value_new_user doesn't trigger,
    but then I need another rule.

    Actually, simplest approach: just have 4 transactions and set the max lower,
    so that the test amount doesn't trigger high_value_new_user.

    Let me use amount 800 for the test instead of 1600.
    - 3 transactions: 250, 350, 500
    - Max: 500
    - Test amount: 800
    - exceeds_max_history: 800 > 500 * 1.5 (750) → TRUE → score 70
    - high_value_new_user: 800 NOT > 1000 → FALSE
    - Total: 70... still < 80

    Hmm, this is tricky. Let me think differently. Maybe I should just increase the
    exceeds_max_history score to 80 in the rule file... but that changes the rule globally.

    Wait, let me check if the test amount can be adjusted instead. Looking at the test
    script, it uses 1600. What if I make the test trigger TWO small rules?

    Actually, let me look at what other rules might trigger with amount=1600 and
    a small transaction history. Let me go with 2 historical transactions of 600, 800,
    then test with 1600:
    - Max: 800
    - exceeds_max_history: 1600 > 800 * 1.5 (1200) → TRUE → score 70
    - high_value_new_user: 1600 > 1000 AND 2 < 5 → TRUE → score 80
    - Total: 150 → decline (too high!)

    The problem is that 1600 > 1000 always triggers high_value_new_user if total < 5.

    Let me try a different approach: keep 5 transactions so high_value_new_user doesn't trigger,
    but make it trigger amount_spike (ratio > 3.0, score 60).
    - 5 transactions: 300, 350, 400, 450, 500 (avg = 400)
    - Max: 500
    - Test amount: 1600
    - Ratio: 1600 / 400 = 4.0 > 3.0 → amount_spike triggers → score 60
    - exceeds_max_history: 1600 > 500 * 1.5 (750) → TRUE → score 70
    - Total: 60 + 70 = 130 → review! ✓

    This looks good! Let me implement this approach.
    Expected: score = 130 (60 + 70) → review
    """
    events = []
    user_id = "user_max_history"

    # 5 transactions to avoid high_value_new_user (which requires total_txn < 5)
    # Amounts: 300, 350, 400, 450, 500 → avg = 400, max = 500
    amounts = [300.0, 350.0, 400.0, 450.0, 500.0]

    for i, amount in enumerate(amounts):
        days_ago = (i + 1) * 4  # Days: 4, 8, 12, 16, 20 (within 30-day window)
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            amount,
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_micro_transaction_history():
    """Generate micro transaction pattern for Test 38 (tests min_transaction_amount)

    Rule: min_transaction_amount < 10 AND amount > 3000 → score 40 (approve)
    Feature: MIN(amount) WHERE days <= 30

    Design:
    - Maintain a minimum of 5 to satisfy the min_transaction_amount check
    - Raise the historical average to avoid amount_spike / large_vs_average
      by mixing in mid-size transactions
    - 10 micro transactions @ 5 (days 1-10)
    - 15 mid transactions @ 2200 (days 11-25)
    - Avg ~1322, so 3200/1322 < 3 and does not trigger amount_spike
    Expected: only micro_transaction_pattern fires → score 40 → approve
    """
    events = []
    user_id = "user_micro_pattern"

    # 10 micro transactions
    for i in range(10):
        days_ago = i + 1  # Days 1-10
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            5.0,  # Fixed micro amount
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    # 15 mid-size transactions to lift the average
    for i in range(15):
        days_ago = 11 + i  # Days 11-25
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            2200.0,
            "USD",
            f"merchant_{(i + 10) % 10:03d}",
            f"rnd_device_{(i + 10) % 5:05d}",
            f"192.168.{(i + 10) // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_recent_spike_history():
    """Generate recent spending spike for Test 39 (tests avg_transaction_amount_7d)

    Rule: avg_transaction_amount_7d > avg_transaction_amount * 2 → score 65 (review)
    Features:
    - AVG(amount) WHERE days <= 7
    - AVG(amount) WHERE days <= 30

    Design:
    - 30-day baseline: 20 transactions @ 50 each, days 8-27 → avg_30d ≈ 200
    - 7-day spike: 7 transactions @ 500 each, days 1-7 → avg_7d = 500 (> 400)
    - Test event: 600
    Expected: score = 65 → review
    """
    events = []
    user_id = "user_recent_spike"

    # 30-day baseline: 20 transactions @ 50 each
    for i in range(20):
        days_ago = 8 + i  # Days 8-27
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            50.0,  # Fixed baseline amount
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.1.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    # 7-day spike: 7 transactions @ 500 each
    for i in range(7):
        days_ago = i + 1  # Days 1-7 (within 7-day window)
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            500.0,  # Fixed spike amount
            "USD",
            f"merchant_{(20 + i) % 10:03d}",
            f"rnd_device_{(20 + i) % 5:05d}",
            f"192.168.2.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_velocity_spike_history():
    """Generate velocity spike pattern for Test 40 (tests txn_velocity_1h_to_24h)

    Rule: txn_velocity_1h_to_24h > 10 → score 90 (decline)
    Expression: txn_count_1h / max(txn_count_24h / 24, 1)
    Features:
    - COUNT(*) WHERE hours <= 1
    - COUNT(*) WHERE hours <= 24

    Design:
    - 24h baseline: 24 transactions (one per hour), hours 2-25 → txn_count_24h = 24
    - 1h spike: Will be added by test event (included in 1h count)
    - Note: Test will add multiple events in 1h to trigger velocity > 10
    - Just need baseline 24h transactions evenly distributed
    Expected: When test adds 15 events in 1h, velocity = 15 / (24/24) = 15 > 10
    """
    events = []
    user_id = "user_velocity_test"

    # 24 transactions evenly distributed in 24h (one per hour)
    for i in range(24):
        hours_ago = 2 + i  # Hours 2, 3, 4, ..., 25 (avoid 1h window)
        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            200.0,  # Fixed amount
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_concentration_history():
    """Generate concentration history for Test 41 (tests amount_concentration_24h)

    Rule: amount_concentration_24h > 0.8 → score 55 (review)
    Expression: max_transaction_amount / max(txn_amount_24h, 1)
    Features:
    - MAX(amount) WHERE hours <= 24
    - SUM(amount) WHERE hours <= 24

    Design:
    - 24h history: 4 small transactions @ 100 each, total = 400
    - Hours: 5, 10, 15, 20
    - Test event: 5000
    - Concentration: 5000 / (5000 + 400) = 5000 / 5400 = 0.926 > 0.8
    Expected: score = 55 → review
    """
    events = []
    user_id = "user_concentration"

    # 4 small transactions @ 100 each
    hours_offsets = [5, 10, 15, 20]
    for i, hours_ago in enumerate(hours_offsets):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            100.0,  # Fixed small amount
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.1.{i + 10}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_wide_range_history():
    """Generate wide amount range for Test 42 (tests txn_amount_range_30d)

    Rule: txn_amount_range_30d > 5000 → score 45 (approve)
    Expression: max_transaction_amount - min_transaction_amount
    Features:
    - MAX(amount) WHERE days <= 30
    - MIN(amount) WHERE days <= 30

    Design:
    - 10 transactions with specific amounts: 50, 100, 500, 1000, 2000, 3000, 4000, 5000, 6000, 6500
    - Days: 1, 3, 5, 7, 10, 13, 16, 20, 25, 29
    - Range: 6500 - 50 = 6450 > 5000
    - Test event: 1000
    Expected: score = 45 → approve (< 80)
    """
    events = []
    user_id = "user_wide_range"

    # Fixed amounts and days for deterministic range
    amounts = [50, 100, 500, 1000, 2000, 3000, 4000, 5000, 6000, 6500]
    days_offsets = [1, 3, 5, 7, 10, 13, 16, 20, 25, 29]

    for i, (amount, days_ago) in enumerate(zip(amounts, days_offsets)):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            float(amount),
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 5}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_spending_acceleration_history():
    """Generate spending acceleration for Test 43 (tests avg_amount_acceleration)

    Rule: avg_amount_acceleration > 2.5 → score 70 (review)
    Expression: avg_transaction_amount_7d / max(avg_transaction_amount, 1)
    Features:
    - AVG(amount) WHERE days <= 7
    - AVG(amount) WHERE days <= 30

    Design:
    - 30-day baseline: 23 transactions @ 50 each, days 8-30 → avg_30d ≈ 170
    - 7-day spike: 7 transactions @ 600 each, days 1-7 → avg_7d = 600
    - Acceleration: 600 / ~170 > 3.5 > 2.5
    - Test event: 700
    Expected: score = 70 → review
    """
    events = []
    user_id = "user_acceleration"

    # 30-day baseline: 23 transactions @ 50 each (stay within 30d window)
    for i in range(23):
        days_ago = 8 + i  # Days 8-30
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            50.0,  # Fixed baseline amount
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.1.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    # 7-day spike: 7 transactions @ 600 each
    for i in range(7):
        days_ago = i + 1  # Days 1-7
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            600.0,  # Fixed spike amount
            "USD",
            f"merchant_{(25 + i) % 10:03d}",
            f"rnd_device_{(25 + i) % 5:05d}",
            f"192.168.2.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_multi_device_24h_history():
    """Generate multi-device 24h pattern for Test 44 (tests unique_devices_24h)

    Rule: unique_devices_24h >= 3 → score 60 (review)
    Feature: DISTINCT(device_id) WHERE hours <= 24

    Design:
    - 24h: 4 transactions from 4 different devices
    - Devices: device_test_001, device_test_002, device_test_003, device_test_004
    - Hours: 2, 6, 12, 20
    - 7-day history: 10 transactions from 2 old devices (alternating)
    - Test event: device_test_005
    - Count: 4 >= 3
    Expected: score = 60 → review
    """
    events = []
    user_id = "user_multi_device_24h"

    # 4 transactions from 4 different devices in 24h
    devices_24h = ["device_test_001", "device_test_002", "device_test_003", "device_test_004"]
    hours_offsets = [2, 6, 12, 20]

    for i, (device, hours_ago) in enumerate(zip(devices_24h, hours_offsets)):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            350.0,  # Increased to avoid exceeds_max_history (500 <= 350*1.5=525)
            "USD",
            f"merchant_{i:03d}",
            device,
            f"192.168.1.{i + 10}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    # 10 older transactions on 2 old devices (alternating)
    for i in range(10):
        days_ago = 2 + i  # Days 2-11
        device = f"device_old_{i % 2:03d}"  # Alternating between 2 devices
        events.append((
            "transaction",
            user_id,
            generate_timestamp(days_ago=days_ago),
            300.0,  # Increased for consistency
            "USD",
            f"merchant_{i % 10:03d}",
            device,
            f"192.168.2.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_high_device_activity():
    """Generate high device activity for Test 45 (tests txn_count_by_device_24h)

    Rule: txn_count_by_device_24h >= 10 → score 50 (review)
    Feature: COUNT(*) WHERE device_id = event.device_id AND hours <= 24

    Design:
    - Device: device_high_activity
    - 12 transactions in 24h from this device
    - Hours: 1, 2, 3, 4, 6, 8, 10, 12, 14, 16, 18, 22
    - Amounts: 150 each (fixed)
    - Test event: same device, amount 300
    - Count: 12 >= 10
    Expected: score = 50 → review
    """
    events = []
    user_id = "user_device_high"
    device_id = "device_high_activity"

    # 12 transactions from same device in 24h
    # Start from 2h to avoid triggering high_frequency_1h (needs 5+ in 1h)
    hours_offsets = [2, 3, 4, 5, 7, 9, 11, 13, 15, 17, 19, 23]

    for i, hours_ago in enumerate(hours_offsets):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            220.0,  # Increased to avoid exceeds_max_history (300 <= 220*1.5=330)
            "USD",
            f"merchant_{i % 10:03d}",
            device_id,
            f"192.168.1.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_payment_frequency_history():
    """Generate payment frequency for Test 46 (tests payment_count_24h)

    Rule: payment_count_24h >= 5 → score 55 (review)
    Feature: COUNT(*) WHERE event_type = 'payment' AND hours <= 24

    Design:
    - 6 payments in 24h
    - Hours: 2, 5, 8, 12, 16, 20
    - Amounts: 500 each (fixed)
    - Test event: payment, amount 600
    - Count: 6 >= 5
    Expected: score = 55 → review
    """
    events = []
    user_id = "user_payment_freq"

    # 6 payments in 24h
    hours_offsets = [2, 5, 8, 12, 16, 20]

    for i, hours_ago in enumerate(hours_offsets):
        events.append((
            "payment",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            500.0,  # Fixed amount
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.1.{i + 10}",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({"payment_method": "card", "payment_type": "one_time"})
        ))

    return events


def generate_weekly_payment_history():
    """Generate weekly payment history for Test 47 (tests payment_sum_7d)

    Rule: payment_sum_7d > 20000 → score 60 (review)
    Feature: SUM(amount) WHERE event_type = 'payment' AND days <= 7

    Design:
    - 8 payments @ 3000 each = 24000 (> 20000)
    - Days: 1, 2, 3, 4, 5, 6, 7, 7 (within 7-day window)
    - Test event: payment, amount 3500
    Expected: score = 60 → review
    """
    events = []
    user_id = "user_payment_weekly"

    # 8 payments @ 3000 each
    # Use 6 instead of 7 for last two to ensure they fall within 7-day window
    # (7 days ago might be just outside the 604800 second window depending on time of day)
    days_offsets = [1, 2, 3, 4, 5, 6, 6, 6]  # 8 payments within 7 days

    for i, days_ago in enumerate(days_offsets):
        events.append((
            "payment",
            user_id,
            generate_timestamp(days_ago=days_ago),
            3000.0,  # Fixed amount, 8 * 3000 = 24000
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.1.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({"payment_method": "card", "payment_type": "one_time"})
        ))

    return events


def generate_payment_max_baseline():
    """Generate payment max baseline for Test 48 (tests max_payment_amount_30d)

    Rule: amount > max_payment_amount_30d * 2 → score 65 (review)
    Feature: MAX(amount) WHERE event_type = 'payment' AND days <= 30

    Design:
    - 20 payments with increasing amounts: 500, 600, 700, ..., 2400
    - Days: 1, 2, 3, ..., 20
    - Max: 2400
    - Test event: payment, amount 5000 (> 2400 * 2)
    Expected: score = 65 → review
    """
    events = []
    user_id = "user_payment_max"

    # 20 payments with increasing amounts
    for i in range(20):
        days_ago = i + 1  # Days 1-20
        amount = 500 + i * 100  # 500, 600, 700, ..., 2400
        events.append((
            "payment",
            user_id,
            generate_timestamp(days_ago=days_ago),
            float(amount),
            "USD",
            f"merchant_{i % 10:03d}",
            f"rnd_device_{i % 100:05d}",
            f"192.168.{i // 10}.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({"payment_method": "card", "payment_type": "one_time"})
        ))

    return events


def generate_payment_ratio_history():
    """Generate payment/transaction ratio for Test 49 (tests payment_to_txn_ratio)

    Rule: payment_to_txn_ratio > 0.8 → score 50 (review)
    Expression: payment_count_24h / max(txn_count_24h + payment_count_24h, 1)
    Features:
    - COUNT(*) WHERE event_type = 'payment' AND hours <= 24
    - COUNT(*) WHERE event_type = 'transaction' AND hours <= 24

    Design:
    - 4 payments in 24h
    - 1 transaction in 24h
    - Ratio: 4 / (4 + 1) = 0.8
    - Test event: payment -> 5 / (5 + 1) = 0.833 > 0.8
    - Keeps payment_count_24h below 5 to avoid high_payment_frequency firing
    Expected: score = 50 → approve (only payment_dominance would score 50 < 100)
    """
    events = []
    user_id = "user_payment_ratio"

    # 4 payments in 24h
    hours_offsets_payments = [2, 8, 14, 20]
    for i, hours_ago in enumerate(hours_offsets_payments):
        events.append((
            "payment",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            400.0,  # Fixed amount
            "USD",
            f"merchant_{i:03d}",
            f"rnd_device_{i:05d}",
            f"192.168.1.{i + 10}",
            "US",
            f"{user_id}@example.com",
            None,
            "completed",
            json.dumps({"payment_method": "card", "payment_type": "one_time"})
        ))

    # 1 transaction in 24h
    hours_offsets_txns = [5]
    for i, hours_ago in enumerate(hours_offsets_txns):
        events.append((
            "transaction",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            200.0,  # Fixed amount
            "USD",
            f"merchant_{i + 10:03d}",
            f"rnd_device_{i + 10:05d}",
            f"192.168.2.{i + 10}",
            "US",
            f"{user_id}@example.com",
            None,
            None,
            None
        ))

    return events


def generate_low_login_history():
    """Generate low login history for Test 50 (tests successful_login_count_7d)

    Rule: successful_login_count_7d < 2 AND country in high_risk_countries → score 70 (review)
    Feature: COUNT(*) WHERE event_type = 'login' AND status = 'success' AND days <= 7

    Design:
    - 1 successful login in 7 days (< 2)
    - Days: 3
    - Test event: login from high-risk country (NG)
    - Count: 1 < 2
    Expected: score = 70 → review
    """
    events = []
    user_id = "user_low_login"

    # Only 1 successful login in 7 days
    events.append((
        "login",
        user_id,
        generate_timestamp(days_ago=3),
        None,
        None,
        None,
        "device_low_login",  # Dedicated device to avoid shared_device rule
        "192.168.1.100",
        "US",
        f"{user_id}@example.com",
        None,
        "success",
        None
    ))

    return events


def generate_multi_ip_history():
    """Generate multi-IP history for Test 51 (tests unique_ips_24h)

    Rule: unique_ips_24h >= 5 → score 75 (review)
    Feature: DISTINCT(ip_address) WHERE event_type = 'login' AND hours <= 24

    Design:
    - 6 logins from 6 different IPs in 24h
    - IPs: 10.0.1.1, 10.0.2.1, 10.0.3.1, 10.0.4.1, 10.0.5.1, 10.0.6.1
    - Hours: 2, 5, 8, 12, 16, 20
    - Test event: login from new IP (10.0.7.1)
    - Count: 6 >= 5
    Expected: score = 75 → review
    """
    events = []
    user_id = "user_multi_ip"

    # 6 logins from 6 different IPs
    ips = [f"10.0.{i}.1" for i in range(1, 7)]
    hours_offsets = [2, 5, 8, 12, 16, 20]

    for i, (ip, hours_ago) in enumerate(zip(ips, hours_offsets)):
        events.append((
            "login",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            None,
            None,
            None,
            f"device_multi_ip_{i:03d}",  # Dedicated device prefix
            ip,
            "US",
            f"{user_id}@example.com",
            None,
            "success",
            None
        ))

    return events


def generate_failure_rate_history():
    """Generate high failure rate for Test 52 (tests rate_failed_login)

    Rule: rate_failed_login > 0.7 → score 80 (review)
    Expression: failed_login_count_24h / max(successful_login_count_7d, 1)
    Features:
    - COUNT(*) WHERE event_type = 'login' AND status = 'failed' AND hours <= 24
    - COUNT(*) WHERE event_type = 'login' AND status = 'success' AND days <= 7

    Design:
    - 10 failed logins in 24h
    - 3 successful logins in 7 days (outside 24h window)
    - Rate: 10 / 3 = 3.33 > 0.7
    - Test event: login (success)
    Expected: score = 80 → review
    """
    events = []
    user_id = "user_failure_rate"

    # 10 failed logins in 24h
    hours_offsets_failed = [2, 4, 6, 8, 10, 12, 14, 16, 18, 20]
    for i, hours_ago in enumerate(hours_offsets_failed):
        events.append((
            "login",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            None,
            None,
            None,
            f"device_failure_{i:03d}",  # Dedicated device prefix
            f"192.168.1.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            "failed",
            None
        ))

    # 3 successful logins in 7 days (outside 24h)
    days_offsets_success = [2, 4, 6]
    for i, days_ago in enumerate(days_offsets_success):
        events.append((
            "login",
            user_id,
            generate_timestamp(days_ago=days_ago),
            None,
            None,
            None,
            f"device_failure_s_{i:03d}",  # Dedicated device prefix for success logins
            f"192.168.2.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            "success",
            None
        ))

    return events


def generate_device_velocity_history():
    """Generate device velocity pattern for Test 53 (tests device_velocity_ratio)

    Rule: device_velocity_ratio > 0.7 → score 65 (review)
    Expression: unique_devices_24h / max(unique_devices_7d, 1)
    Features:
    - DISTINCT(device_id) WHERE event_type = 'login' AND hours <= 24
    - DISTINCT(device_id) WHERE event_type = 'login' AND days <= 7

    Design:
    - 24h: 5 different devices (device_24h_001 - device_24h_005)
    - 7d total: 6 different devices (5 from 24h + 1 older device)
    - Ratio: 5 / 6 = 0.833 > 0.7
    - Test event: login from device_24h_001 (reuse existing device to keep ratio same)
    Expected: score = 80 → review
    """
    events = []
    user_id = "user_device_velocity"

    # 5 logins from 5 different devices in 24h
    devices_24h = [f"device_24h_{i:03d}" for i in range(1, 6)]
    hours_offsets = [2, 6, 10, 14, 20]
    stable_ip = "172.16.24.1"

    for i, (device, hours_ago) in enumerate(zip(devices_24h, hours_offsets)):
        events.append((
            "login",
            user_id,
            generate_timestamp(hours_ago=hours_ago),
            None,
            None,
            None,
            device,
            stable_ip,  # keep IP constant to avoid multiple_ips_24h triggering
            "US",
            f"{user_id}@example.com",
            None,
            "success",
            None
        ))

    # 5 logins from 1 older device in 7d (outside 24h)
    older_device = "device_7d_001"
    days_offsets = [2, 3, 4, 5, 6]
    for i, days_ago in enumerate(days_offsets):
        events.append((
            "login",
            user_id,
            generate_timestamp(days_ago=days_ago),
            None,
            None,
            None,
            older_device,
            f"192.168.2.{i % 256}",
            "US",
            f"{user_id}@example.com",
            None,
            "success",
            None
        ))

    return events


def generate_shared_device_history():
    """Generate shared device pattern for Test 54 (tests unique_users_by_device_7d)

    Rule: unique_users_by_device_7d >= 5 → score 70 (review)
    Feature: DISTINCT(user_id) WHERE device_id = event.device_id AND days <= 7

    Design:
    - Device: device_shared
    - 6 logins from 6 different users on same device
    - Users: shared_user_001 - shared_user_006
    - Days: 1, 2, 3, 4, 5, 6
    - Test event: login from user shared_user_007 on same device
    - Count: 6 >= 5
    Expected: score = 70 → review
    """
    events = []
    device_id = "device_shared"

    # 6 logins from 6 different users on same device
    users = [f"shared_user_{i:03d}" for i in range(1, 7)]
    days_offsets = [1, 2, 3, 4, 5, 6]

    for i, (user, days_ago) in enumerate(zip(users, days_offsets)):
        events.append((
            "login",
            user,
            generate_timestamp(days_ago=days_ago),
            None,
            None,
            None,
            device_id,
            f"192.168.1.{i % 256}",
            "US",
            f"{user}@example.com",
            None,
            "success",
            None
        ))

    return events


def generate_user_0102_login_history():
    """Generate login history for user_0102 (Test 14 - Geographic Mismatch)

    Design:
    - Has >= 2 successful logins in past 7 days (avoids low_login_history)
    - Has 2 different devices in 7d history (unique_devices_7d = 2 > 1)
    - Test event uses NEW device from high-risk country
    - This triggers new_device_high_risk_country: unique_devices_7d > 1 AND country in high_risk
    Expected: score = 90 → review
    """
    events = []
    user_id = "user_0102"

    # 4 successful logins from 2 different devices in past 7 days
    # This ensures unique_devices_7d = 2 > 1
    login_data = [
        (2, "device_0102_a"),  # 2 days ago, device A
        (3, "device_0102_b"),  # 3 days ago, device B
        (5, "device_0102_a"),  # 5 days ago, device A
        (6, "device_0102_b"),  # 6 days ago, device B
    ]

    for days_ago, device in login_data:
        events.append((
            "login",
            user_id,
            generate_timestamp(days_ago=days_ago),
            None,
            None,
            None,
            device,
            "192.168.2.100",
            "US",  # Normal login from US
            f"{user_id}@example.com",
            None,
            "success",
            None
        ))

    return events


def generate_test_clean_user_russia_login_history():
    """Generate login history for test_clean_user_russia (Test 7 - High Risk Country Login)

    To prevent low_login_history rule from triggering, ensure this user
    has >= 2 successful logins in past 7 days from Russia.
    """
    events = []
    user_id = "test_clean_user_russia"

    # 3 successful logins in past 7 days from Russia
    days_offsets = [1, 3, 5]
    for i, days_ago in enumerate(days_offsets):
        events.append((
            "login",
            user_id,
            generate_timestamp(days_ago=days_ago),
            None,
            None,
            None,
            "test_device_001",  # Same device as Test 7
            f"192.168.1.{200 + i}",  # Consistent Russian IPs
            "RU",  # Russia
            f"{user_id}@example.com",
            None,
            "success",
            None
        ))

    return events


def generate_list_data():
    """Generate list entries data for database-backed lists"""
    from datetime import timezone
    list_entries = []
    now = datetime.now(timezone.utc).replace(tzinfo=None)

    # Blocked users list
    blocked_users = SUSPICIOUS_USERS[:5]  # Use first 5 suspicious users
    for user_id in blocked_users:
        list_entries.append({
            'list_id': 'blocked_users',
            'value': user_id,
            'expires_at': None,  # Never expires
            'metadata': json.dumps({'reason': 'suspicious_activity', 'blocked_date': now.isoformat()})
        })

    # Blocked IPs list
    for ip in SUSPICIOUS_IPS:
        list_entries.append({
            'list_id': 'blocked_ips',
            'value': ip,
            'expires_at': None,  # Never expires
            'metadata': json.dumps({'reason': 'malicious_traffic', 'threat_level': 'high'})
        })

    # High risk countries list
    for country in SUSPICIOUS_COUNTRIES:
        list_entries.append({
            'list_id': 'high_risk_countries',
            'value': country,
            'expires_at': None,  # Never expires
            'metadata': json.dumps({'risk_level': 'high', 'category': 'fraud_hotspot'})
        })

    # === Expiration test entries ===
    # These entries use 'blocked_users_db' list_id to match the SQLite backend list config

    # Expired blocked user (should NOT block - Test 23)
    expired_time = (now - timedelta(days=1)).isoformat()
    list_entries.append({
        'list_id': 'blocked_users_db',
        'value': 'user_expired_block',
        'expires_at': expired_time,
        'metadata': json.dumps({'reason': 'test_expiration', 'expired': True})
    })

    # Active blocked user with future expiration (should block - Test 24)
    future_time = (now + timedelta(days=30)).isoformat()
    list_entries.append({
        'list_id': 'blocked_users_db',
        'value': 'user_active_block',
        'expires_at': future_time,
        'metadata': json.dumps({'reason': 'test_expiration', 'expired': False})
    })

    return list_entries


def generate_list_insert_statements(list_entries):
    """Generate SQL INSERT statements for list entries"""
    sql_statements = []

    for entry in list_entries:
        list_id = escape_sql_string(entry['list_id'])
        value = escape_sql_string(entry['value'])
        expires_at = escape_sql_string(entry.get('expires_at'))
        metadata = escape_sql_string(entry.get('metadata'))

        sql = f"INSERT INTO list_entries (list_id, value, expires_at, metadata) VALUES ({list_id}, {value}, {expires_at}, {metadata});"
        sql_statements.append(sql)

    return sql_statements


def main():
    print("=" * 60)
    print("CORINT E2E Test Data Generator")
    print("=" * 60)
    print()

    # Create output directory
    SQL_OUTPUT.parent.mkdir(parents=True, exist_ok=True)

    # Generate all event types
    all_events = []

    print("Generating test data...")
    generators = [
        ("Normal transactions", generate_normal_transactions, 100),
        ("Suspicious transactions", generate_suspicious_transactions, 20),
        ("Velocity abuse patterns", generate_velocity_abuse, 30),
        ("Login events", generate_login_events, 200),
        ("Payment events", generate_payment_events, 80),
        ("Account takeover patterns", generate_account_takeover_pattern, 15),
    ]

    total_events = 0
    for name, generator, count in generators:
        events = generator(count)
        all_events.extend(events)
        total_events += len(events)
        print(f"  ✓ {name}: {len(events)} events")

    # Additional test-specific data generators (no count parameter)
    print()
    print("Generating test-specific data...")
    additional_generators = [
        ("Failed login history (Test 17)", generate_failed_login_history),
        ("High frequency transactions (Test 13)", generate_high_frequency_transactions),
        ("VIP user history (Test 12)", generate_vip_user_history),
        ("Crypto payment history (Test 16)", generate_crypto_payment_history),
        ("User 0102 login history (Test 14)", generate_user_0102_login_history),
        ("Test clean user Russia login history (Test 7)", generate_test_clean_user_russia_login_history),
        # New feature coverage tests (Test 34-54)
        ("Low weekly activity (Test 34)", generate_low_weekly_activity_history),
        ("High total spending (Test 35)", generate_high_total_spending_history),
        ("Average baseline (Test 36)", generate_avg_baseline_history),
        ("Max history baseline (Test 37)", generate_max_history_baseline),
        ("Micro transaction (Test 38)", generate_micro_transaction_history),
        ("Recent spike (Test 39)", generate_recent_spike_history),
        ("Velocity spike (Test 40)", generate_velocity_spike_history),
        ("Concentration (Test 41)", generate_concentration_history),
        ("Wide range (Test 42)", generate_wide_range_history),
        ("Spending acceleration (Test 43)", generate_spending_acceleration_history),
        ("Multi-device 24h (Test 44)", generate_multi_device_24h_history),
        ("High device activity (Test 45)", generate_high_device_activity),
        ("Payment frequency (Test 46)", generate_payment_frequency_history),
        ("Weekly payment (Test 47)", generate_weekly_payment_history),
        ("Payment max baseline (Test 48)", generate_payment_max_baseline),
        ("Payment ratio (Test 49)", generate_payment_ratio_history),
        ("Low login history (Test 50)", generate_low_login_history),
        ("Multi-IP (Test 51)", generate_multi_ip_history),
        ("Failure rate (Test 52)", generate_failure_rate_history),
        ("Device velocity (Test 53)", generate_device_velocity_history),
        ("Shared device (Test 54)", generate_shared_device_history),
    ]

    for name, generator in additional_generators:
        events = generator()
        all_events.extend(events)
        total_events += len(events)
        print(f"  ✓ {name}: {len(events)} events")

    # Generate list data
    print()
    print("Generating list data...")
    list_entries = generate_list_data()
    print(f"  ✓ Blocked users: {len([e for e in list_entries if e['list_id'] == 'blocked_users'])} entries")
    print(f"  ✓ Blocked IPs: {len([e for e in list_entries if e['list_id'] == 'blocked_ips'])} entries")
    print(f"  ✓ High risk countries: {len([e for e in list_entries if e['list_id'] == 'high_risk_countries'])} entries")

    print()
    print(f"Generating SQL statements for {total_events} events and {len(list_entries)} list entries...")

    # Write SQL file
    with open(SQL_OUTPUT, 'w') as f:
        # Write schema
        f.write("-- CORINT E2E Test Data\n")
        f.write(f"-- Generated at: {datetime.now().isoformat()}\n")
        f.write(f"-- Total events: {total_events}\n")
        f.write(f"-- Total list entries: {len(list_entries)}\n\n")

        f.write(generate_schema_sql())
        f.write("\n")

        # Write event INSERT statements
        insert_statements = generate_insert_statements(all_events)
        for sql in insert_statements:
            f.write(sql + "\n")

        f.write("\n-- List entries data\n")

        # Write list INSERT statements
        list_insert_statements = generate_list_insert_statements(list_entries)
        for sql in list_insert_statements:
            f.write(sql + "\n")

    print(f"✓ SQL file generated: {SQL_OUTPUT}")
    print()

    # Print statistics
    print("Data Statistics:")
    print("-" * 60)
    print(f"Total events: {total_events}")

    # Count by type
    event_types = {}
    for event in all_events:
        event_type = event[0]
        event_types[event_type] = event_types.get(event_type, 0) + 1

    print("\nEvents by type:")
    for event_type, count in sorted(event_types.items()):
        print(f"  {event_type}: {count}")

    # Count unique users
    unique_users = set(event[1] for event in all_events)
    print(f"\nUnique users: {len(unique_users)}")

    # Time range
    timestamps = [event[2] for event in all_events]
    print(f"\nTime range:")
    print(f"  Earliest: {min(timestamps)}")
    print(f"  Latest: {max(timestamps)}")

    print()
    print("=" * 60)
    print("✓ SQL generation completed!")
    print("=" * 60)
    print()
    print(f"To create database, run:")
    print(f"  sqlite3 tests/data/e2e_test.db < {SQL_OUTPUT}")


if __name__ == "__main__":
    main()
