-- ============================================================================
-- PostgreSQL Sample Data and Queries
-- Demonstrates how to use the Corint Risk Control System database tables
-- ============================================================================

-- ============================================================================
-- 1. Insert Sample Data
-- ============================================================================
-- Note: Data is inserted with timestamps spanning multiple time windows
-- to support testing of various feature calculations (1h, 24h, 7d, 30d)

-- 1.0 Insert Register Event (for account_age_days feature)
INSERT INTO events (
    request_id, event_id, event_timestamp, event_type,
    user_id, device_id, ip_address,
    attributes
) VALUES
(
    'req_000',
    'evt_register_001',
    '2025-11-01 08:00:00+00',  -- 35 days ago (for user_001)
    'register',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "registration_method": "email",
        "referral_code": "REF001"
    }'::jsonb
),
(
    'req_000b',
    'evt_register_002',
    '2025-11-28 10:00:00+00',  -- 7 days ago (for user_003)
    'register',
    'user_003',
    'device_003',
    '203.0.113.3',
    '{
        "country": "CN",
        "city": "Shanghai",
        "registration_method": "phone",
        "referral_code": null
    }'::jsonb
);

-- 1.1 Insert Event Data (Login Events - Multiple time points for testing)
INSERT INTO events (
    request_id, event_id, event_timestamp, event_type,
    user_id, device_id, ip_address,
    attributes
) VALUES
-- Login events for user_001 (spread across time windows)
(
    'req_001',
    'evt_login_001',
    '2025-12-05 10:00:00+00',  -- Current time
    'login',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "device_type": "mobile",
        "os": "iOS",
        "os_version": "17.2",
        "browser": "Safari",
        "browser_version": "17.1",
        "login_method": "password",
        "status": "success",
        "is_new_device": false,
        "account_age_days": 35,
        "user_agent": "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X)",
        "referrer": "https://app.example.com/home",
        "language": "en-US",
        "screen_resolution": "1170x2532",
        "timezone": "America/New_York"
    }'::jsonb
),
-- Failed login attempt (for failed_login_count_1h feature)
(
    'req_001b',
    'evt_login_002',
    '2025-12-05 09:45:00+00',  -- 15 minutes ago
    'login',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "device_type": "mobile",
        "os": "iOS",
        "login_method": "password",
        "status": "failed",
        "failure_reason": "invalid_password"
    }'::jsonb
),
-- Login from different device (for unique_devices feature)
(
    'req_001c',
    'evt_login_003',
    '2025-12-04 14:00:00+00',  -- 20 hours ago
    'login',
    'user_001',
    'device_002',  -- Different device
    '203.0.113.2',  -- Different IP
    '{
        "country": "US",
        "city": "Boston",
        "device_type": "desktop",
        "os": "Windows",
        "login_method": "password",
        "status": "success"
    }'::jsonb
),
-- Login from different city (for unique_cities feature)
(
    'req_001d',
    'evt_login_004',
    '2025-11-28 12:00:00+00',  -- 7 days ago
    'login',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "Los Angeles",
        "device_type": "mobile",
        "login_method": "password",
        "status": "success"
    }'::jsonb
);

-- 1.2 Insert Event Data (Transaction Events - Multiple amounts and time points)
INSERT INTO events (
    request_id, event_id, event_timestamp, event_type,
    user_id, device_id, ip_address,
    attributes
) VALUES
-- Transaction 1: Normal amount, current time
(
    'req_002',
    'evt_txn_001',
    '2025-12-05 10:30:00+00',
    'transaction',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "device_type": "mobile",
        "os": "iOS",
        "browser": "Safari",
        "payment_method": "credit_card",
        "card_type": "visa",
        "card_last4": "4242",
        "merchant_id": "merchant_123",
        "merchant_category": "electronics",
        "amount": 299.99,
        "currency": "USD",
        "is_international": false,
        "shipping_country": "US",
        "user_agent": "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X)",
        "checkout_duration_sec": 45,
        "language": "en-US"
    }'::jsonb
),
-- Transaction 2: Different amount, 2 hours ago (for velocity testing)
(
    'req_002b',
    'evt_txn_002',
    '2025-12-05 08:30:00+00',  -- 2 hours ago
    'transaction',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "amount": 150.50,
        "currency": "USD",
        "payment_method": "credit_card",
        "merchant_category": "food"
    }'::jsonb
),
-- Transaction 3: Different amount, 1 day ago
(
    'req_002c',
    'evt_txn_003',
    '2025-12-04 10:00:00+00',  -- 1 day ago
    'transaction',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "amount": 450.00,
        "currency": "USD",
        "payment_method": "credit_card",
        "merchant_category": "clothing"
    }'::jsonb
),
-- Transaction 4: Different amount, 5 days ago (for 7d window)
(
    'req_002d',
    'evt_txn_004',
    '2025-11-30 10:00:00+00',  -- 5 days ago
    'transaction',
    'user_001',
    'device_002',  -- Different device
    '203.0.113.2',  -- Different IP
    '{
        "country": "US",
        "city": "Boston",
        "amount": 200.00,
        "currency": "USD",
        "payment_method": "credit_card",
        "merchant_category": "books"
    }'::jsonb
),
-- Transaction 5: Different amount, 15 days ago (for 30d window)
(
    'req_002e',
    'evt_txn_005',
    '2025-11-20 10:00:00+00',  -- 15 days ago
    'transaction',
    'user_001',
    'device_001',
    '203.0.113.1',
    '{
        "country": "US",
        "city": "New York",
        "amount": 100.00,
        "currency": "USD",
        "payment_method": "credit_card",
        "merchant_category": "electronics"
    }'::jsonb
);

-- 1.3 Insert High-Risk Event (Raw Input)
INSERT INTO events (
    request_id, event_id, event_timestamp, event_type,
    user_id, device_id, ip_address,
    attributes
) VALUES
(
    'req_003',
    'evt_txn_006',
    '2025-12-05 11:00:00+00',
    'transaction',
    'user_003',
    'device_003',
    '203.0.113.3',
    '{
        "country": "CN",
        "city": "Shanghai",
        "device_type": "mobile",
        "os": "Android",
        "os_version": "14",
        "payment_method": "credit_card",
        "card_type": "mastercard",
        "merchant_id": "merchant_456",
        "merchant_category": "cryptocurrency",
        "amount": 5000.00,
        "currency": "USD",
        "is_international": true,
        "is_new_device": true,
        "user_agent": "Mozilla/5.0 (Linux; Android 14)",
        "vpn_detected": true,
        "language": "zh-CN"
    }'::jsonb
),
-- Additional high-velocity transactions for user_003 (for velocity testing)
(
    'req_003b',
    'evt_txn_007',
    '2025-12-05 10:50:00+00',  -- 10 minutes ago
    'transaction',
    'user_003',
    'device_003',
    '203.0.113.3',
    '{
        "country": "CN",
        "city": "Shanghai",
        "amount": 1000.00,
        "currency": "USD",
        "payment_method": "credit_card",
        "merchant_category": "cryptocurrency"
    }'::jsonb
),
(
    'req_003c',
    'evt_txn_008',
    '2025-12-05 10:40:00+00',  -- 20 minutes ago
    'transaction',
    'user_003',
    'device_003',
    '203.0.113.3',
    '{
        "country": "CN",
        "city": "Shanghai",
        "amount": 2000.00,
        "currency": "USD",
        "payment_method": "credit_card",
        "merchant_category": "cryptocurrency"
    }'::jsonb
);

-- ============================================================================
-- 2. Basic Query Examples (Single Table - Events Only)
-- ============================================================================

-- 2.1 Query all events from the last 24 hours
SELECT
    request_id,
    event_id,
    event_timestamp,
    event_type,
    user_id,
    device_id,
    ip_address,
    attributes->>'amount' as amount
FROM events
WHERE event_timestamp > NOW() - INTERVAL '24 hours'
ORDER BY event_timestamp DESC
LIMIT 100;

-- 2.2 Query all transactions for a specific user
SELECT
    event_id,
    event_timestamp,
    attributes->>'amount' as amount,
    attributes->>'currency' as currency,
    attributes->>'payment_method' as payment_method
FROM events
WHERE user_id = 'user_001'
  AND event_type = 'transaction'
  AND event_timestamp > NOW() - INTERVAL '30 days'
ORDER BY event_timestamp DESC;

-- 2.3 Query events from a specific country
SELECT
    request_id,
    event_id,
    event_timestamp,
    event_type,
    user_id,
    attributes->>'country' as country,
    attributes->>'city' as city
FROM events
WHERE attributes->>'country' = 'CN'
ORDER BY event_timestamp DESC;

-- 2.4 Query events by event type
SELECT
    event_id,
    event_timestamp,
    event_type,
    user_id,
    (attributes->>'amount')::numeric as amount
FROM events
WHERE event_type = 'transaction'
  AND event_timestamp > NOW() - INTERVAL '7 days'
ORDER BY event_timestamp DESC;

-- ============================================================================
-- 3. JSONB Query Examples (Single Table - Events Only)
-- ============================================================================

-- 3.1 Query events by payment method
SELECT
    event_id,
    event_timestamp,
    (attributes->>'amount')::numeric as amount,
    attributes->>'payment_method' as payment_method,
    attributes->>'card_type' as card_type
FROM events
WHERE attributes->>'payment_method' = 'credit_card'
  AND event_type = 'transaction'
ORDER BY event_timestamp DESC
LIMIT 50;

-- 3.2 Query events from mobile devices
SELECT
    event_id,
    event_timestamp,
    user_id,
    attributes->>'device_type' as device_type,
    attributes->>'os' as os
FROM events
WHERE attributes->>'device_type' = 'mobile'
ORDER BY event_timestamp DESC
LIMIT 50;

-- 3.3 Query events with VPN detected
SELECT
    event_id,
    event_timestamp,
    user_id,
    ip_address,
    attributes->>'country' as country,
    (attributes->>'vpn_detected')::boolean as vpn_detected
FROM events
WHERE (attributes->>'vpn_detected')::boolean = true
ORDER BY event_timestamp DESC;

-- 3.4 Query events with specific merchant category
SELECT
    event_id,
    event_timestamp,
    (attributes->>'amount')::numeric as amount,
    attributes->>'merchant_category' as merchant_category
FROM events
WHERE attributes->>'merchant_category' IN ('cryptocurrency', 'gambling')
  AND event_type = 'transaction'
ORDER BY event_timestamp DESC;

-- ============================================================================
-- 4. Aggregate Query Examples (Real-time Feature Calculation)
-- ============================================================================

-- 4.1 Calculate transaction velocity for a user (last 1 hour)
SELECT
    user_id,
    COUNT(*) as txn_count_1h,
    SUM((attributes->>'amount')::numeric) as total_amount_1h,
    AVG((attributes->>'amount')::numeric) as avg_amount_1h
FROM events
WHERE user_id = 'user_001'
  AND event_type = 'transaction'
  AND event_timestamp > NOW() - INTERVAL '1 hour'
GROUP BY user_id;

-- 4.2 Calculate device velocity (transactions per device in last 24h)
SELECT
    e.device_id,
    COUNT(*) as txn_count_24h,
    COUNT(DISTINCT e.user_id) as unique_users,
    SUM((e.attributes->>'amount')::numeric) as total_amount
FROM events e
WHERE e.event_type = 'transaction'
  AND e.event_timestamp > NOW() - INTERVAL '24 hours'
GROUP BY e.device_id
HAVING COUNT(*) > 5
ORDER BY txn_count_24h DESC;

-- 4.3 Calculate IP velocity (events per IP in last 1 hour)
SELECT
    ip_address,
    COUNT(*) as event_count_1h,
    COUNT(DISTINCT user_id) as unique_users,
    COUNT(DISTINCT device_id) as unique_devices
FROM events
WHERE event_timestamp > NOW() - INTERVAL '1 hour'
GROUP BY ip_address
HAVING COUNT(DISTINCT user_id) > 3
ORDER BY event_count_1h DESC;

-- 4.4 Calculate amount statistics for user transactions
    SELECT
        user_id,
        AVG((attributes->>'amount')::numeric) as avg_amount,
    STDDEV((attributes->>'amount')::numeric) as stddev_amount,
    MIN((attributes->>'amount')::numeric) as min_amount,
    MAX((attributes->>'amount')::numeric) as max_amount
    FROM events
    WHERE event_type = 'transaction'
      AND event_timestamp > NOW() - INTERVAL '30 days'
    GROUP BY user_id
ORDER BY avg_amount DESC;

-- 4.5 Calculate cross-device usage pattern (multiple users per device)
SELECT
    e.device_id,
    COUNT(DISTINCT e.user_id) as unique_users,
    COUNT(*) as total_events,
    ARRAY_AGG(DISTINCT e.user_id) as user_list
FROM events e
WHERE e.event_timestamp > NOW() - INTERVAL '7 days'
GROUP BY e.device_id
HAVING COUNT(DISTINCT e.user_id) > 1
ORDER BY unique_users DESC;

-- ============================================================================
-- 5. Window Function Examples (Time-series Analysis)
-- ============================================================================

-- 5.1 Calculate running transaction count for each user
SELECT
    event_id,
    event_timestamp,
    user_id,
    (attributes->>'amount')::numeric as amount,
    COUNT(*) OVER (
        PARTITION BY user_id
        ORDER BY event_timestamp
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) as running_txn_count
FROM events
WHERE event_type = 'transaction'
  AND user_id = 'user_001'
ORDER BY event_timestamp DESC;

-- 5.2 Calculate time between consecutive transactions for a user
SELECT
    event_id,
    event_timestamp,
    user_id,
    (attributes->>'amount')::numeric as amount,
    event_timestamp - LAG(event_timestamp) OVER (
        PARTITION BY user_id
        ORDER BY event_timestamp
    ) as time_since_last_txn
FROM events
WHERE event_type = 'transaction'
  AND user_id = 'user_001'
ORDER BY event_timestamp DESC;

-- 5.3 Calculate moving average of transaction amounts (30-day window)
SELECT
    event_id,
    event_timestamp,
    user_id,
    (attributes->>'amount')::numeric as amount,
    AVG((attributes->>'amount')::numeric) OVER (
        PARTITION BY user_id
        ORDER BY event_timestamp
        RANGE BETWEEN INTERVAL '30 days' PRECEDING AND CURRENT ROW
    ) as moving_avg_30d
FROM events
WHERE event_type = 'transaction'
  AND user_id = 'user_001'
ORDER BY event_timestamp DESC;

-- ============================================================================
-- 6. Event Statistics Analysis (Single Table - Events Only)
-- ============================================================================

-- 6.1 Analyze event distribution by event type
SELECT
    event_type,
    COUNT(*) as count,
    COUNT(DISTINCT user_id) as unique_users,
    COUNT(DISTINCT device_id) as unique_devices
FROM events
WHERE event_timestamp > NOW() - INTERVAL '7 days'
GROUP BY event_type
ORDER BY count DESC;

-- 6.2 Analyze event volume by hour
SELECT
    DATE_TRUNC('hour', event_timestamp) as hour,
    COUNT(*) as event_count,
    COUNT(DISTINCT user_id) as unique_users
FROM events
WHERE event_timestamp > NOW() - INTERVAL '24 hours'
GROUP BY hour
ORDER BY hour DESC;

-- 6.3 Analyze transaction amounts by merchant category
SELECT
    attributes->>'merchant_category' as merchant_category,
    COUNT(*) as transaction_count,
    AVG((attributes->>'amount')::numeric) as avg_amount,
    SUM((attributes->>'amount')::numeric) as total_amount
FROM events
WHERE event_type = 'transaction'
  AND event_timestamp > NOW() - INTERVAL '7 days'
GROUP BY attributes->>'merchant_category'
ORDER BY transaction_count DESC;

-- ============================================================================
-- End of Examples
-- ============================================================================
