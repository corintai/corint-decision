# CORINT Risk Definition Language (RDL)
## Event Specification (v0.1)

This document defines the standard event types, schemas, and data structures used in CORINT's Risk Definition Language.

A well-defined event catalog ensures:
- Consistent data structures across all risk scenarios
- Type safety and validation at compile time
- Clear documentation for integration teams
- Reusable schemas across rules and pipelines

---

## 1. Event Structure Overview

### 1.1 Base Event Structure

Every event in CORINT follows a base structure:

```yaml
event:
  # Metadata (required)
  id: string                     # Unique event identifier
  type: string                   # Event type (login, transaction, etc.)
  timestamp: datetime            # Event occurrence time
  version: string                # Schema version

  # Common entities (optional, depends on event type)
  user: <user-schema>            # User information
  device: <device-schema>        # Device information
  geo: <geo-schema>              # Geographic information
  session: <session-schema>      # Session information

  # Event-specific data
  data: <event-specific-data>    # Varies by event type
```

### 1.2 Event Metadata

```yaml
event_metadata:
  id:
    type: string
    format: uuid
    required: true
    description: "Unique identifier for this event"
    example: "550e8400-e29b-41d4-a716-446655440000"

  type:
    type: string
    required: true
    description: "Event type identifier"
    enum: [login, logout, transaction, transfer, registration, ...]

  timestamp:
    type: datetime
    format: iso8601
    required: true
    description: "When the event occurred"
    example: "2024-01-15T10:30:00Z"

  version:
    type: string
    required: true
    description: "Event schema version"
    pattern: "^\\d+\\.\\d+$"
    example: "1.0"

  source:
    type: string
    required: false
    description: "System that generated the event"
    example: "mobile-app-ios"

  correlation_id:
    type: string
    format: uuid
    required: false
    description: "ID to correlate related events"
```

---

## 2. Common Entity Schemas

### 2.1 User Schema

```yaml
user_schema:
  id:
    type: string
    required: true
    description: "User unique identifier"

  email:
    type: string
    format: email
    required: false

  phone:
    type: string
    format: phone
    required: false

  name:
    type: object
    properties:
      first:
        type: string
      last:
        type: string
      full:
        type: string

  profile:
    type: object
    properties:
      tier:
        type: string
        enum: [basic, standard, premium, vip]
        default: basic

      status:
        type: string
        enum: [active, suspended, blocked, pending]
        default: active

      kyc_level:
        type: integer
        min: 0
        max: 3
        description: "KYC verification level"

      created_at:
        type: datetime
        description: "Account creation date"

      country:
        type: string
        format: iso3166-alpha2
        description: "User's registered country"

  risk_profile:
    type: object
    properties:
      score:
        type: number
        min: 0
        max: 100

      level:
        type: string
        enum: [low, medium, high, critical]

      last_updated:
        type: datetime

  # Historical data (for feature engineering)
  history:
    type: object
    properties:
      login_count_7d:
        type: integer
      failed_login_count_24h:
        type: integer
      transaction_count_30d:
        type: integer
      last_login_time:
        type: datetime
      last_transaction_time:
        type: datetime
      known_devices:
        type: array
        items: string
      known_ips:
        type: array
        items: string
```

### 2.2 Device Schema

```yaml
device_schema:
  id:
    type: string
    required: true
    description: "Device unique identifier or fingerprint"

  type:
    type: string
    enum: [desktop, mobile, tablet, unknown]
    required: true

  platform:
    type: string
    enum: [ios, android, windows, macos, linux, web, unknown]

  browser:
    type: object
    properties:
      name:
        type: string
        enum: [chrome, firefox, safari, edge, opera, unknown]
      version:
        type: string

  os:
    type: object
    properties:
      name:
        type: string
      version:
        type: string

  hardware:
    type: object
    properties:
      model:
        type: string
      manufacturer:
        type: string
      screen_resolution:
        type: string

  fingerprint:
    type: object
    properties:
      hash:
        type: string
      confidence:
        type: number
        min: 0
        max: 1
      components:
        type: object

  # Device trust indicators
  trust:
    type: object
    properties:
      is_known:
        type: boolean
        description: "Device previously used by this user"
      is_new:
        type: boolean
        description: "First time seeing this device"
      is_trusted:
        type: boolean
        description: "Device explicitly trusted by user"
      first_seen:
        type: datetime
      last_seen:
        type: datetime
      usage_count:
        type: integer

  # Risk indicators
  risk:
    type: object
    properties:
      is_emulator:
        type: boolean
      is_rooted:
        type: boolean
      is_jailbroken:
        type: boolean
      is_bot:
        type: boolean
      tampering_detected:
        type: boolean
```

### 2.3 Geographic Schema

```yaml
geo_schema:
  ip:
    type: string
    format: ip
    required: true
    description: "IP address (IPv4 or IPv6)"

  country:
    type: string
    format: iso3166-alpha2
    description: "Country code"

  region:
    type: string
    description: "State/Province/Region"

  city:
    type: string

  postal_code:
    type: string

  location:
    type: object
    properties:
      latitude:
        type: number
        min: -90
        max: 90
      longitude:
        type: number
        min: -180
        max: 180
      accuracy:
        type: number
        description: "Location accuracy in meters"

  timezone:
    type: string
    format: iana_timezone
    example: "America/New_York"

  # IP intelligence
  ip_info:
    type: object
    properties:
      is_proxy:
        type: boolean
      is_vpn:
        type: boolean
      is_tor:
        type: boolean
      is_datacenter:
        type: boolean
      is_mobile:
        type: boolean
      isp:
        type: string
      organization:
        type: string
      asn:
        type: integer
      reputation_score:
        type: number
        min: 0
        max: 100
```

### 2.4 Session Schema

```yaml
session_schema:
  id:
    type: string
    required: true

  created_at:
    type: datetime
    required: true

  expires_at:
    type: datetime

  duration:
    type: integer
    description: "Session duration in seconds"

  is_active:
    type: boolean

  auth_method:
    type: string
    enum: [password, mfa, sso, biometric, token]

  mfa:
    type: object
    properties:
      enabled:
        type: boolean
      method:
        type: string
        enum: [sms, totp, email, push, hardware_key]
      verified:
        type: boolean
```

---

## 3. Event Type Definitions

### 3.1 Login Event

```yaml
event_type: login
version: "1.0"
description: "User authentication attempt"

schema:
  # Base metadata
  <<: *event_metadata

  # Event type
  type:
    const: login

  # Common entities
  user:
    $ref: "#/user_schema"
    required: true

  device:
    $ref: "#/device_schema"
    required: true

  geo:
    $ref: "#/geo_schema"
    required: true

  # Login-specific data
  login:
    type: object
    required: true
    properties:
      method:
        type: string
        enum: [password, sso, social, biometric, magic_link, api_key]
        required: true

      status:
        type: string
        enum: [success, failed, blocked, pending_mfa]
        required: true

      failure_reason:
        type: string
        enum: [invalid_credentials, account_locked, expired_password, mfa_failed, rate_limited]
        required_if: status == "failed"

      mfa:
        type: object
        properties:
          required:
            type: boolean
          method:
            type: string
            enum: [sms, totp, email, push, hardware_key]
          status:
            type: string
            enum: [pending, verified, failed, skipped]

      provider:
        type: string
        description: "SSO or social provider"
        enum: [google, facebook, apple, microsoft, okta, custom]
        required_if: method in ["sso", "social"]

      remember_me:
        type: boolean
        default: false

example:
  id: "evt_login_123456"
  type: "login"
  timestamp: "2024-01-15T10:30:00Z"
  version: "1.0"

  user:
    id: "usr_abc123"
    email: "user@example.com"
    profile:
      tier: "premium"
      kyc_level: 2

  device:
    id: "dev_xyz789"
    type: "mobile"
    platform: "ios"
    trust:
      is_known: true
      is_new: false

  geo:
    ip: "203.0.113.42"
    country: "US"
    city: "San Francisco"

  login:
    method: "password"
    status: "success"
    mfa:
      required: true
      method: "totp"
      status: "verified"
```

### 3.2 Transaction Event

```yaml
event_type: transaction
version: "1.0"
description: "Financial transaction event"

schema:
  <<: *event_metadata

  type:
    const: transaction

  user:
    $ref: "#/user_schema"
    required: true

  device:
    $ref: "#/device_schema"
    required: false

  geo:
    $ref: "#/geo_schema"
    required: true

  # Transaction-specific data
  transaction:
    type: object
    required: true
    properties:
      id:
        type: string
        required: true

      type:
        type: string
        enum: [purchase, transfer, withdrawal, deposit, refund, payment]
        required: true

      status:
        type: string
        enum: [pending, completed, failed, cancelled, reversed]
        default: pending

      amount:
        type: number
        required: true
        min: 0

      currency:
        type: string
        format: iso4217
        required: true
        example: "USD"

      # Payment method
      payment_method:
        type: object
        properties:
          type:
            type: string
            enum: [card, bank_transfer, wallet, crypto, cash]
          id:
            type: string
            description: "Payment method ID"
          is_new:
            type: boolean
          last_four:
            type: string
            pattern: "^\\d{4}$"
          brand:
            type: string
            enum: [visa, mastercard, amex, discover, jcb, unionpay]

      # For transfers
      sender:
        type: object
        properties:
          account_id:
            type: string
          name:
            type: string
          bank:
            type: string

      recipient:
        type: object
        properties:
          account_id:
            type: string
          name:
            type: string
          bank:
            type: string
          country:
            type: string
            format: iso3166-alpha2
          is_new:
            type: boolean
            description: "First transaction to this recipient"

      # Merchant info (for purchases)
      merchant:
        type: object
        properties:
          id:
            type: string
          name:
            type: string
          category:
            type: string
            description: "MCC category"
          category_code:
            type: string
            pattern: "^\\d{4}$"
          country:
            type: string
            format: iso3166-alpha2
          risk_level:
            type: string
            enum: [low, medium, high]

      # Order details
      items:
        type: array
        items:
          type: object
          properties:
            name:
              type: string
            quantity:
              type: integer
            unit_price:
              type: number
            category:
              type: string

      description:
        type: string
        max_length: 500

      reference:
        type: string
        description: "External reference number"

example:
  id: "evt_txn_789012"
  type: "transaction"
  timestamp: "2024-01-15T14:22:00Z"
  version: "1.0"

  user:
    id: "usr_abc123"
    profile:
      tier: "standard"

  geo:
    ip: "203.0.113.42"
    country: "US"

  transaction:
    id: "txn_xyz456"
    type: "purchase"
    status: "pending"
    amount: 299.99
    currency: "USD"
    payment_method:
      type: "card"
      brand: "visa"
      last_four: "4242"
    merchant:
      name: "Electronics Store"
      category: "electronics"
      category_code: "5732"
```

### 3.3 Crypto Transfer Event

```yaml
event_type: crypto_transfer
version: "1.0"
description: "Cryptocurrency transfer event"

schema:
  <<: *event_metadata

  type:
    const: crypto_transfer

  user:
    $ref: "#/user_schema"
    required: true

  device:
    $ref: "#/device_schema"

  geo:
    $ref: "#/geo_schema"

  # Crypto-specific data
  crypto:
    type: object
    required: true
    properties:
      chain:
        type: string
        enum: [ethereum, bitcoin, polygon, solana, bsc, arbitrum, optimism, avalanche]
        required: true

      transaction_hash:
        type: string
        required: false
        description: "On-chain transaction hash (after broadcast)"

      type:
        type: string
        enum: [send, receive, swap, stake, unstake, bridge, contract_interaction]
        required: true

      status:
        type: string
        enum: [pending, broadcasted, confirmed, failed]

      # Sender wallet
      from_wallet:
        type: object
        required: true
        properties:
          address:
            type: string
            required: true
          is_internal:
            type: boolean
            description: "Wallet owned by platform user"
          label:
            type: string
          risk_score:
            type: number

      # Recipient wallet
      to_wallet:
        type: object
        required: true
        properties:
          address:
            type: string
            required: true
          is_internal:
            type: boolean
          is_new:
            type: boolean
            description: "First time sending to this address"
          label:
            type: string
          risk_score:
            type: number
          is_contract:
            type: boolean
          contract_name:
            type: string

      # Asset info
      asset:
        type: object
        required: true
        properties:
          symbol:
            type: string
            required: true
            example: "ETH"
          name:
            type: string
            example: "Ethereum"
          contract_address:
            type: string
            description: "Token contract address (for ERC20, etc.)"
          decimals:
            type: integer
          type:
            type: string
            enum: [native, erc20, erc721, erc1155, spl]

      # Amount
      amount:
        type: string
        required: true
        description: "Amount in smallest unit (wei, satoshi, etc.)"

      amount_decimal:
        type: number
        description: "Human-readable amount"

      amount_usd:
        type: number
        description: "USD equivalent at time of transaction"

      # Gas/Fee info
      gas:
        type: object
        properties:
          limit:
            type: integer
          price:
            type: string
          max_fee:
            type: string
          priority_fee:
            type: string
          estimated_cost_usd:
            type: number

example:
  id: "evt_crypto_345678"
  type: "crypto_transfer"
  timestamp: "2024-01-15T16:45:00Z"
  version: "1.0"

  user:
    id: "usr_abc123"

  crypto:
    chain: "ethereum"
    type: "send"
    status: "pending"

    from_wallet:
      address: "0x1234...abcd"
      is_internal: true

    to_wallet:
      address: "0x5678...efgh"
      is_new: true
      is_contract: false

    asset:
      symbol: "ETH"
      name: "Ethereum"
      type: "native"

    amount: "1000000000000000000"
    amount_decimal: 1.0
    amount_usd: 2500.00
```

### 3.4 Registration Event

```yaml
event_type: registration
version: "1.0"
description: "New user registration"

schema:
  <<: *event_metadata

  type:
    const: registration

  device:
    $ref: "#/device_schema"
    required: true

  geo:
    $ref: "#/geo_schema"
    required: true

  # Registration-specific data
  registration:
    type: object
    required: true
    properties:
      method:
        type: string
        enum: [email, phone, social, sso]
        required: true

      email:
        type: string
        format: email
        required_if: method == "email"

      phone:
        type: string
        format: phone
        required_if: method == "phone"

      social_provider:
        type: string
        enum: [google, facebook, apple, twitter]
        required_if: method == "social"

      referral_code:
        type: string

      promo_code:
        type: string

      terms_accepted:
        type: boolean
        required: true

      marketing_consent:
        type: boolean
        default: false

      # Risk signals during registration
      email_validation:
        type: object
        properties:
          is_disposable:
            type: boolean
          is_free_provider:
            type: boolean
          domain_age_days:
            type: integer
          mx_valid:
            type: boolean

      phone_validation:
        type: object
        properties:
          is_valid:
            type: boolean
          carrier:
            type: string
          type:
            type: string
            enum: [mobile, landline, voip, unknown]
          is_voip:
            type: boolean
```

### 3.5 Password Change Event

```yaml
event_type: password_change
version: "1.0"
description: "User password change or reset"

schema:
  <<: *event_metadata

  type:
    const: password_change

  user:
    $ref: "#/user_schema"
    required: true

  device:
    $ref: "#/device_schema"
    required: true

  geo:
    $ref: "#/geo_schema"
    required: true

  session:
    $ref: "#/session_schema"

  # Password change specific
  password_change:
    type: object
    required: true
    properties:
      type:
        type: string
        enum: [change, reset, forced_reset]
        required: true

      trigger:
        type: string
        enum: [user_initiated, forgot_password, admin_forced, security_policy, breach_detected]
        required: true

      verification_method:
        type: string
        enum: [current_password, email_link, sms_code, security_questions, support_verification]

      is_successful:
        type: boolean
        required: true

      failure_reason:
        type: string
        required_if: is_successful == false
```

### 3.6 KYC Verification Event

```yaml
event_type: kyc_verification
version: "1.0"
description: "Know Your Customer verification event"

schema:
  <<: *event_metadata

  type:
    const: kyc_verification

  user:
    $ref: "#/user_schema"
    required: true

  # KYC-specific data
  kyc:
    type: object
    required: true
    properties:
      level:
        type: integer
        min: 1
        max: 3
        required: true
        description: "Target KYC level"

      status:
        type: string
        enum: [initiated, pending_documents, pending_review, approved, rejected, expired]
        required: true

      provider:
        type: string
        description: "KYC verification provider"

      # Document verification
      documents:
        type: array
        items:
          type: object
          properties:
            type:
              type: string
              enum: [passport, drivers_license, national_id, residence_permit, utility_bill, bank_statement]
            country:
              type: string
              format: iso3166-alpha2
            status:
              type: string
              enum: [submitted, verified, rejected, expired]
            rejection_reason:
              type: string

      # Identity verification
      identity:
        type: object
        properties:
          name_match:
            type: boolean
          dob_match:
            type: boolean
          address_match:
            type: boolean

      # Liveness check
      liveness:
        type: object
        properties:
          passed:
            type: boolean
          method:
            type: string
            enum: [video, photo, 3d_face]
          confidence:
            type: number

      # Sanctions/PEP check
      screening:
        type: object
        properties:
          sanctions_hit:
            type: boolean
          pep_hit:
            type: boolean
          adverse_media:
            type: boolean
          watchlist_hits:
            type: array
            items: string
```

### 3.7 Withdrawal Request Event

```yaml
event_type: withdrawal_request
version: "1.0"
description: "Withdrawal or payout request"

schema:
  <<: *event_metadata

  type:
    const: withdrawal_request

  user:
    $ref: "#/user_schema"
    required: true

  device:
    $ref: "#/device_schema"

  geo:
    $ref: "#/geo_schema"

  # Withdrawal-specific data
  withdrawal:
    type: object
    required: true
    properties:
      id:
        type: string
        required: true

      amount:
        type: number
        required: true
        min: 0

      currency:
        type: string
        format: iso4217
        required: true

      method:
        type: string
        enum: [bank_transfer, card, crypto, check, cash]
        required: true

      destination:
        type: object
        properties:
          type:
            type: string
            enum: [bank_account, card, crypto_wallet, cash_pickup]
          account_id:
            type: string
          is_new:
            type: boolean
          is_verified:
            type: boolean
          country:
            type: string
            format: iso3166-alpha2

      # For crypto withdrawals
      crypto:
        type: object
        properties:
          chain:
            type: string
          address:
            type: string
          is_whitelisted:
            type: boolean

      reason:
        type: string

      urgency:
        type: string
        enum: [standard, express, instant]
```

---

## 4. Event Versioning

### 4.1 Version Format

```yaml
versioning:
  format: major.minor           # e.g., "1.0", "1.1", "2.0"

  compatibility:
    # Minor version changes are backward compatible
    minor: backward_compatible

    # Major version changes may break compatibility
    major: breaking_changes

  migration:
    # How to handle old versions
    strategy: transform         # transform | reject | coexist
```

### 4.2 Version Migration

```yaml
event_migrations:
  - from_version: "1.0"
    to_version: "1.1"
    event_type: transaction

    transformations:
      # Rename field
      - rename:
          from: transaction.receiver
          to: transaction.recipient

      # Add default value for new field
      - add:
          field: transaction.risk_flags
          default: []

  - from_version: "1.1"
    to_version: "2.0"
    event_type: transaction

    transformations:
      # Restructure
      - move:
          from: transaction.card_info
          to: transaction.payment_method

      # Remove deprecated field
      - remove:
          field: transaction.legacy_id
```

---

## 5. Event Validation

### 5.1 Validation Rules

```yaml
validation:
  # Strict mode: reject events with unknown fields
  strict_mode: true

  # Required fields validation
  required_fields:
    - id
    - type
    - timestamp

  # Type validation
  type_validation: true

  # Format validation (email, phone, etc.)
  format_validation: true

  # Custom validators
  custom_validators:
    - name: amount_positive
      condition: event.transaction.amount > 0
      error: "Transaction amount must be positive"

    - name: currency_matches_amount
      condition: |
        event.transaction.currency is_not_null implies
        event.transaction.amount is_not_null
      error: "Currency requires amount"
```

### 5.2 Validation Error Handling

```yaml
on_validation_error:
  strategy: reject              # reject | warn | fix

  # Auto-fix options (when strategy is "fix")
  auto_fix:
    missing_timestamp: use_current_time
    missing_id: generate_uuid
    invalid_email: set_null

  # Logging
  log_level: warn
  log_details: true
```

---

## 6. Event Enrichment

### 6.1 Automatic Enrichment

```yaml
enrichment:
  # Geo enrichment from IP
  geo_from_ip:
    enabled: true
    source_field: geo.ip
    enrich_fields:
      - country
      - city
      - timezone
      - ip_info

  # Device enrichment from user-agent
  device_from_ua:
    enabled: true
    source_field: headers.user_agent
    enrich_fields:
      - device.type
      - device.platform
      - device.browser

  # User enrichment from database
  user_profile:
    enabled: true
    source_field: user.id
    enrich_fields:
      - user.profile
      - user.history
      - user.risk_profile
```

---

## 7. Using Events in Rules

### 7.1 Event Type Filtering

```yaml
rule:
  id: suspicious_login
  name: Suspicious Login Detection

  when:
    # Filter by event type
    event.type: login

    conditions:
      - event.login.status == "success"
      - event.device.trust.is_new == true
      - event.geo.ip_info.is_vpn == true

  score: 60
```

### 7.2 Multi-Event Rules

```yaml
rule:
  id: registration_fraud_pattern

  when:
    # Match multiple event types
    event.type in ["registration", "login"]

    conditions:
      - any:
          - all:
              - event.type == "registration"
              - event.registration.email_validation.is_disposable == true

          - all:
              - event.type == "login"
              - event.device.risk.is_emulator == true

  score: 80
```

---

## 8. Event Catalog Registry

### 8.1 Event Catalog Definition

```yaml
# events.yml
event_catalog:
  version: "1.0"
  name: "CORINT Event Catalog"
  description: "Standard event definitions for risk decisioning"

  # Common schemas (reusable)
  common_schemas:
    user: schemas/user.yml
    device: schemas/device.yml
    geo: schemas/geo.yml
    session: schemas/session.yml

  # Event type definitions
  event_types:
    # Authentication events
    - category: authentication
      events:
        - type: login
          file: events/login.yml
        - type: logout
          file: events/logout.yml
        - type: password_change
          file: events/password_change.yml
        - type: mfa_challenge
          file: events/mfa_challenge.yml

    # Transaction events
    - category: transactions
      events:
        - type: transaction
          file: events/transaction.yml
        - type: crypto_transfer
          file: events/crypto_transfer.yml
        - type: withdrawal_request
          file: events/withdrawal_request.yml

    # Account events
    - category: account
      events:
        - type: registration
          file: events/registration.yml
        - type: profile_update
          file: events/profile_update.yml
        - type: kyc_verification
          file: events/kyc_verification.yml
```

---

## 9. Best Practices

### 9.1 Event Design

**Good:**
```yaml
event:
  type: transaction
  version: "1.0"

  # Clear structure
  transaction:
    amount: 100.00
    currency: "USD"

  # Typed fields
  user:
    id: "usr_123"

  # Timestamps in ISO8601
  timestamp: "2024-01-15T10:30:00Z"
```

**Avoid:**
```yaml
event:
  type: "txn"                    # Unclear abbreviation
  data:
    amt: "100"                   # String instead of number
    curr: "usd"                  # Inconsistent case
  ts: 1705315800                 # Unix timestamp (hard to read)
```

### 9.2 Schema Evolution

- Use semantic versioning for schemas
- Add new optional fields (backward compatible)
- Deprecate before removing fields
- Document all changes in changelog
- Test migrations thoroughly

---

## 10. Summary

CORINT's Event Catalog provides:

- **Standardized event structures** for all risk scenarios
- **Common entity schemas** (user, device, geo) for consistency
- **Event type definitions** for login, transaction, crypto, KYC, etc.
- **Version management** for schema evolution
- **Validation rules** for data quality
- **Enrichment capabilities** for enhanced context

This enables reliable event processing and consistent rule evaluation across all risk decisioning scenarios.

---

## 11. Related Documentation

- `schema.md` - Type system and validation
- `context.md` - Context and variable management
- `rule.md` - Using events in rules
- `feature.md` - Feature engineering from events
