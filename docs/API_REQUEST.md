# API Request and Response Design

This document defines the API request and response format for Corint's risk management platform.

## Table of Contents

- [Overview](#overview)
- [Request Format](#request-format)
- [Response Format](#response-format)
- [Decision Structure](#decision-structure)
- [Status Codes](#status-codes)
- [Error Handling](#error-handling)
- [Examples](#examples)
- [Async Request Pattern](#async-request-pattern)
- [Management APIs](#management-apis)
- [Best Practices](#best-practices)

---

## Overview

The Corint API evaluates events against configured rules and returns risk decisions. Each request is assigned a unique `request_id` for tracking and debugging.

---

## Request Format

### Endpoint

```
POST /v1/decide
```

### Request Headers

| Header | Required | Description |
|--------|----------|-------------|
| `Content-Type` | ✅ Yes | Must be `application/json` |
| `Authorization` | ✅ Yes | API key: `Bearer <api_key>` |

### Request Body

```typescript
{
  // Event information (required)
  "event": {
    "type": string,              // Event type (e.g., "login", "transaction", "registration")
    "timestamp": string,         // ISO 8601 timestamp (e.g., "2025-12-23T10:30:00Z")
    "user_id": string,           // User identifier
    "session_id"?: string,       // Optional session identifier
    "ip_address"?: string,       // Client IP address
    "device_id"?: string,        // Device fingerprint or ID
    "amount"?: number,           // Transaction amount (for financial events)
    "currency"?: string,         // Currency code (e.g., "USD", "EUR")
    "location"?: {               // Geolocation data
      "country": string,
      "city": string,
      "latitude": number,
      "longitude": number
    },
    // ... other event-specific fields
  },

  // User profile/context (optional but recommended)
  "user"?: {
    "account_age_days": number,
    "email_verified": boolean,
    "phone_verified": boolean,
    "kyc_status": string,
    "risk_level": string,
    "timezone": string,
    // ... other user profile fields
  },

  // Optional configuration
  "options"?: {
    "return_features"?: boolean, // Whether to return computed feature values (default: false)
    "enable_trace"?: boolean,    // Whether to return detailed execution trace (default: false)
    "async"?: boolean            // Whether to process asynchronously (default: false)
  }
}
```

---

## Response Format

### Successful Response

**HTTP Status:** `200 OK`

```typescript
{
  // Request metadata
  "request_id": string,          // Unique request identifier (e.g., "req_20251223021809_0f8f20")
  "status": 200,                 // HTTP status code
  "process_time_ms": number,     // Processing duration in milliseconds

  // Pipeline information
  "pipeline_id": string,         // Pipeline that processed this request

  // Decision result
  "decision": {
    "result": string,            // Decision result: "APPROVE", "DECLINE", "REVIEW", "HOLD", "PASS"
    "actions": string[],         // Actions to take (e.g., ["KYC_AUTH", "MONITOR", "BLOCK_TRANSACTION"])

    "scores": {
      "canonical": number,       // Normalized risk score (0-1000, where 1000 is highest risk)
      "raw": number,             // Raw aggregated score from rules
      "confidence"?: number      // Confidence level (0-1) - optional
    },

    "evidence": {
      "triggered_rules": string[] // Array of triggered rule IDs
    },

    "cognition": {
      "summary": string,         // Human-readable explanation of the decision
      "reason_codes": string[]   // Machine-readable reason codes (e.g., ["EMAIL_NOT_VERIFIED", "HIGH_RISK_LOCATION"])
    }
  },

  // Optional: Computed features (if options.return_features = true)
  "features"?: {
    [feature_name: string]: any  // Feature name -> computed value
  },

  // Optional: Detailed execution trace (if options.enable_trace = true)
  "trace"?: {
    "pipeline": {
      "pipeline_id": string,
      "steps": [                 // Pipeline execution steps
        {
          "step_id": string,
          "step_name": string,
          "step_type": string,   // "router", "ruleset", "action"
          "executed": boolean,   // Whether this step was executed
          "next_step"?: string,  // Next step ID (if any)
          "default_route"?: boolean,  // Whether default route was taken
          "conditions"?: [       // Router conditions (for router steps)
            {
              "expression": string,
              "result": boolean
            }
          ],
          "ruleset_id"?: string  // Ruleset ID (for ruleset steps)
        }
      ],
      "rulesets": [              // Ruleset evaluation details
        {
          "ruleset_id": string,
          "rules": [
            {
              "rule_id": string,
              "triggered": boolean,
              "score"?: number,  // Rule score (if triggered)
              "conditions": [    // Condition evaluation tree
                {
                  "expression": string,
                  "result": boolean,
                  "left_value"?: any,      // Actual value (if applicable)
                  "nested"?: [...],        // Nested conditions (for all/any groups)
                  "group_type"?: string    // "all" or "any" (for grouped conditions)
                }
              ],
              "execution_time_ms": number
            }
          ],
          "total_score": number,
          "conclusion": [        // Conclusion logic evaluation
            {
              "condition": string,
              "matched": boolean,
              "signal": string,  // Decision signal if this logic matched
              "reason": string
            }
          ],
          "signal": string,      // Ruleset decision signal
          "reason": string       // Ruleset reason
        }
      ]
    }
  }
}
```

---

## Decision Structure

The `decision` object contains comprehensive information about the risk assessment result:

### Decision Result Values

| Value | Description | Typical Use Case |
|-------|-------------|------------------|
| `APPROVE` | Transaction/action approved | Low risk, proceed normally |
| `DECLINE` | Transaction/action rejected | High risk, block immediately |
| `REVIEW` | Requires manual review | Medium-high risk, human intervention needed |
| `HOLD` | Temporarily hold for further processing | Suspicious activity requiring additional checks |
| `PASS` | No decision made, pass to next stage | Rule conditions not met, continue to next ruleset |

### Actions Array

The `actions` field specifies concrete actions to take. Common action values:

| Action | Description |
|--------|-------------|
| `KYC_AUTH` | Require KYC authentication |
| `MONITOR` | Monitor user activity |
| `BLOCK_TRANSACTION` | Block the specific transaction |
| `BLOCK_ACCOUNT` | Suspend/block the user account |
| `MFA_REQUIRED` | Require multi-factor authentication |
| `MANUAL_REVIEW` | Queue for manual review |
| `NOTIFY_USER` | Send notification to user |
| `RATE_LIMIT` | Apply rate limiting |

**Note:** An empty `actions` array indicates no specific action is required beyond the decision result.

### Score Ranges

**Canonical Score (0-1000):**
- `0-200`: Low risk (typically APPROVE)
- `201-500`: Medium risk (typically APPROVE with monitoring)
- `501-700`: Medium-high risk (typically REVIEW or CHALLENGE)
- `701-900`: High risk (typically REVIEW or DECLINE)
- `901-1000`: Critical risk (typically DECLINE)

**Raw Score:** Unprocessed total score from triggered rules. Can exceed 1000.

**Confidence (0-1):** Optional confidence level of the decision (0 = low confidence, 1 = high confidence)

### Reason Codes

Machine-readable codes in `cognition.reason_codes` for automated processing. Common codes:

**Account-related:**
- `EMAIL_NOT_VERIFIED`
- `PHONE_NOT_VERIFIED`
- `NEW_ACCOUNT`
- `ACCOUNT_SUSPENDED`

**Transaction-related:**
- `HIGH_TRANSACTION_AMOUNT`
- `NEW_RECIPIENT`
- `UNUSUAL_PATTERN`
- `VELOCITY_EXCEEDED`

**Location-related:**
- `LOCATION_NOT_MATCH`
- `HIGH_RISK_COUNTRY`
- `VPN_DETECTED`
- `TIMEZONE_MISMATCH`

**Device-related:**
- `UNKNOWN_DEVICE`
- `DEVICE_NOT_VERIFIED`
- `DEVICE_FINGERPRINT_MISMATCH`

**Positive signals:**
- `EMAIL_VERIFIED`
- `PHONE_VERIFIED`
- `KNOWN_DEVICE`
- `LOW_RISK`

---

### Error Response

**HTTP Status:** `4xx` or `5xx`

```typescript
{
  // Request metadata
  "request_id": string,          // Unique request identifier (UUID)
  "status": number,              // HTTP status code (400, 401, 403, 404, 422, 429, 500, 503)

  // Error information
  "error": {
    "code": string,              // Error code (e.g., "INVALID_REQUEST", "RATE_LIMIT_EXCEEDED")
    "message": string,           // Human-readable error message
    "details"?: any,             // Additional error details (e.g., validation errors)
    "retry_after"?: number       // Seconds to wait before retrying (for 429 errors)
  }
}
```

---

## Status Codes

The API uses standard HTTP status codes to indicate request outcomes:

### Success Codes (2xx)

| Status | Description |
|--------|-------------|
| 200 | Request processed successfully |
| 202 | Async request accepted and queued for processing |

### Client Error Codes (4xx)

| Status | Error Code | Description |
|--------|------------|-------------|
| 400 | `INVALID_REQUEST` | Malformed request body or missing required fields |
| 401 | `AUTHENTICATION_FAILED` | Missing or invalid API key |
| 403 | `PERMISSION_DENIED` | API key lacks required permissions |
| 404 | `RESOURCE_NOT_FOUND` | Requested rule or resource not found |
| 422 | `VALIDATION_FAILED` | Request validation failed (detailed errors in `details`) |
| 429 | `RATE_LIMIT_EXCEEDED` | Rate limit exceeded (see `retry_after`) |

### Server Error Codes (5xx)

| Status | Error Code | Description |
|--------|------------|-------------|
| 500 | `INTERNAL_ERROR` | Unexpected server error |
| 503 | `SERVICE_UNAVAILABLE` | Service temporarily unavailable (maintenance or overload) |
| 504 | `TIMEOUT` | Request processing timeout |

---

## Error Handling

### Error Response Structure

All error responses follow this structure:

```typescript
{
  "request_id": "req_20251223103015_v2w3x4",
  "status": 422,
  "error": {
    "code": "VALIDATION_FAILED",
    "message": "Request validation failed",
    "details": {
      "event.type": "Field is required",
      "event.timestamp": "Invalid ISO 8601 timestamp format",
      "event.amount": "Must be a positive number"
    }
  }
}
```

### Client-Side Error Handling Best Practices

1. **Always check `status` field** - Don't rely solely on HTTP status code
2. **Log `request_id`** - Include in support requests for debugging
3. **Handle rate limits** - Respect `retry_after` for 429 errors
4. **Implement retries** - Use exponential backoff for 5xx errors
5. **Parse `error.details`** - Contains field-level validation errors

---

## Examples

All examples below demonstrate the complete response format including the nested `decision` object structure. For detailed information about decision results, actions, scores, and reason codes, see the [Decision Structure](#decision-structure) section.

### Example 1: Successful Login Evaluation

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_live_abc123..." \
  -d '{
    "event": {
      "type": "login",
      "timestamp": "2025-12-23T10:30:00Z",
      "user_id": "user_12345",
      "session_id": "session_67890",
      "ip_address": "203.0.113.42",
      "device_id": "device_xyz789",
      "location": {
        "country": "US",
        "city": "San Francisco",
        "latitude": 37.7749,
        "longitude": -122.4194
      }
    },
    "user": {
      "account_age_days": 365,
      "email_verified": true,
      "phone_verified": true,
      "timezone": "America/Los_Angeles"
    },
    "options": {
      "return_features": true
    }
  }'
```

**Response (200 OK):**

```json
{
  "request_id": "req_20251223103000_a1b2c3",
  "status": 200,
  "process_time_ms": 45,
  "pipeline_id": "login_risk_assessment",
  "decision": {
    "result": "APPROVE",
    "actions": [],
    "scores": {
      "canonical": 150,
      "raw": 15
    },
    "evidence": {
      "triggered_rules": []
    },
    "cognition": {
      "summary": "Low risk: User has verified email and phone, login from known device, location matches user timezone",
      "reason_codes": [
        "EMAIL_VERIFIED",
        "PHONE_VERIFIED",
        "KNOWN_DEVICE",
        "LOCATION_MATCH"
      ]
    }
  },
  "features": {
    "cnt_userid_login_24h": 3,
    "cnt_userid_login_1h_failed": 0,
    "distinct_userid_device_24h": 2,
    "distinct_userid_ip_24h": 1
  }
}
```

---

### Example 2: Blocked Transaction (Fraud Detection)

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_live_abc123..." \
  -d '{
    "event": {
      "type": "transaction",
      "timestamp": "2025-12-23T03:15:00Z",
      "user_id": "user_67890",
      "amount": 5000,
      "currency": "USD",
      "ip_address": "198.51.100.25",
      "device_id": "device_new123",
      "location": {
        "country": "CN",
        "city": "Beijing"
      }
    },
    "user": {
      "account_age_days": 3,
      "email_verified": false,
      "phone_verified": false,
      "timezone": "America/New_York"
    },
    "options": {
      "return_features": true,
      "enable_trace": true
    }
  }'
```

**Response (200 OK):**

```json
{
  "request_id": "req_20251223031500_d4e5f6",
  "status": 200,
  "process_time_ms": 78,
  "pipeline_id": "comprehensive_risk_assessment",
  "decision": {
    "result": "REVIEW",
    "actions": ["KYC_AUTH", "MONITOR"],
    "scores": {
      "canonical": 750,
      "raw": 950,
      "confidence": 0.95
    },
    "evidence": {
      "triggered_rules": [
        "confirmed_fraud_pattern"
      ]
    },
    "cognition": {
      "summary": "Critical fraud risk: Large transaction on new unverified account",
      "reason_codes": [
        "EMAIL_NOT_VERIFIED",
        "PHONE_NOT_VERIFIED",
        "NEW_ACCOUNT",
        "HIGH_TRANSACTION_AMOUNT",
        "NEW_RECIPIENT"
      ]
    }
  },
  "features": {
    "cnt_userid_txn_24h": 1,
    "sum_userid_txn_amt_24h": 15000,
    "avg_userid_order_amt_30d": 15000,
    "distinct_userid_device_24h": 1,
    "account_age_days": 3
  },
  "trace": {
    "pipeline": {
      "pipeline_id": "comprehensive_risk_assessment",
      "steps": [
        {
          "step_id": "blacklist_check",
          "step_name": "Blacklist Check",
          "step_type": "router",
          "executed": true,
          "next_step": "fraud_detection_step",
          "conditions": [
            {
              "expression": "user.ip_country not in list.high_risk_countries",
              "result": true
            }
          ]
        },
        {
          "step_id": "fraud_detection_step",
          "step_name": "Fraud Detection",
          "step_type": "ruleset",
          "executed": true,
          "next_step": "fraud_router",
          "ruleset_id": "fraud_detection_ruleset"
        },
        {
          "step_id": "fraud_router",
          "step_name": "Fraud Result Router",
          "step_type": "router",
          "executed": true,
          "next_step": "end",
          "default_route": true
        },
        {
          "step_id": "user_behavior_step",
          "step_name": "User Behavior Analysis",
          "step_type": "ruleset",
          "executed": false,
          "next_step": "user_behavior_router",
          "ruleset_id": "user_behavior_ruleset"
        },
        {
          "step_id": "user_behavior_router",
          "step_name": "User Behavior Result Router",
          "step_type": "router",
          "executed": false
        },
        {
          "step_id": "amount_router",
          "step_name": "Transaction Amount Router",
          "step_type": "router",
          "executed": false
        },
        {
          "step_id": "transaction_monitoring_step",
          "step_name": "Transaction Monitoring",
          "step_type": "ruleset",
          "executed": false,
          "next_step": "end",
          "ruleset_id": "transaction_monitoring_ruleset"
        },
        {
          "step_id": "basic_monitoring_step",
          "step_name": "Basic Transaction Monitoring",
          "step_type": "ruleset",
          "executed": false,
          "next_step": "end",
          "ruleset_id": "transaction_monitoring_ruleset"
        }
      ],
      "rulesets": [
        {
          "ruleset_id": "fraud_detection_ruleset",
          "rules": [
            {
              "rule_id": "confirmed_fraud_pattern",
              "triggered": true,
              "score": 95,
              "conditions": [
                {
                  "expression": "all:[...]",
                  "result": true,
                  "nested": [
                    {
                      "expression": "event.transaction.amount > 5000.0",
                      "left_value": 15000.0,
                      "result": true
                    },
                    {
                      "expression": "event.user.account_age_days < 7.0",
                      "left_value": 3.0,
                      "result": true
                    },
                    {
                      "expression": "event.transaction.recipient_new == true",
                      "left_value": true,
                      "result": true
                    }
                  ],
                  "group_type": "all"
                }
              ],
              "execution_time_ms": 2
            },
            {
              "rule_id": "complex_fraud_detection",
              "triggered": false,
              "conditions": [
                {
                  "expression": "all:[...]",
                  "result": false,
                  "nested": [
                    {
                      "expression": "event.transaction.amount > 1000.0",
                      "left_value": 15000.0,
                      "result": true
                    },
                    {
                      "expression": "any:[...]",
                      "result": false,
                      "nested": [
                        {
                          "expression": "all:[...]",
                          "result": false,
                          "nested": [
                            {
                              "expression": "event.user.vip_status == false",
                              "left_value": false,
                              "result": true
                            },
                            {
                              "expression": "event.transaction.international == true",
                              "left_value": false,
                              "result": false
                            }
                          ],
                          "group_type": "all"
                        },
                        {
                          "expression": "all:[...]",
                          "result": false,
                          "nested": [
                            {
                              "expression": "event.user.verified == false",
                              "left_value": true,
                              "result": false
                            },
                            {
                              "expression": "event.transaction.payment_method == \"crypto\"",
                              "left_value": "credit_card",
                              "result": false
                            }
                          ],
                          "group_type": "all"
                        }
                      ],
                      "group_type": "any"
                    }
                  ],
                  "group_type": "all"
                }
              ],
              "execution_time_ms": 0
            },
            {
              "rule_id": "risk_threshold_check",
              "triggered": false,
              "conditions": [
                {
                  "expression": "all:[...]",
                  "result": false,
                  "nested": [
                    {
                      "expression": "event.risk_score >= 70.0",
                      "left_value": null,
                      "result": false
                    },
                    {
                      "expression": "event.risk_score <= 90.0",
                      "left_value": null,
                      "result": false
                    },
                    {
                      "expression": "event.confidence_level != \"low\"",
                      "left_value": "medium",
                      "result": true
                    },
                    {
                      "expression": "event.transaction.count < 100.0",
                      "left_value": null,
                      "result": false
                    }
                  ],
                  "group_type": "all"
                }
              ],
              "execution_time_ms": 0
            }
          ],
          "total_score": 950,
          "conclusion": [
            {
              "condition": "total_score >= 900",
              "matched": true,
              "signal": "REVIEW",
              "reason": "Critical fraud risk detected"
            }
          ],
          "signal": "REVIEW",
          "reason": "Risk indicator detected: confirmed_fraud_pattern (high risk)"
        }
      ]
    }
  }
}
```

---

### Example 3: Validation Error (400 Bad Request)

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_live_abc123..." \
  -d '{
    "event": {
      "type": "transaction",
      "amount": -100
    }
  }'
```

**Response (400 Bad Request):**

```json
{
  "request_id": "req_20251223103200_g7h8i9",
  "status": 400,
  "error": {
    "code": "INVALID_REQUEST",
    "message": "Request validation failed",
    "details": {
      "event.timestamp": "Field is required",
      "event.user_id": "Field is required",
      "event.amount": "Must be a positive number"
    }
  }
}
```

---

### Example 4: Rate Limit Exceeded (429 Too Many Requests)

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_live_abc123..." \
  -d '{ ... }'
```

**Response (429 Too Many Requests):**

```json
{
  "request_id": "req_20251223103500_j0k1l2",
  "status": 429,
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded: 1000 requests per minute",
    "details": {
      "limit": 1000,
      "window": "1 minute",
      "current": 1001
    },
    "retry_after": 45
  }
}
```

---

### Example 5: Authentication Failed (401 Unauthorized)

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer invalid_key" \
  -d '{ ... }'
```

**Response (401 Unauthorized):**

```json
{
  "request_id": "req_20251223104000_m3n4o5",
  "status": 401,
  "error": {
    "code": "AUTHENTICATION_FAILED",
    "message": "Invalid API key",
    "details": {
      "hint": "Check your API key in your dashboard settings"
    }
  }
}
```

---

### Example 6: Internal Server Error (500)

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_live_abc123..." \
  -d '{ ... }'
```

**Response (500 Internal Server Error):**

```json
{
  "request_id": "req_20251223104500_p6q7r8",
  "status": 500,
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "An unexpected error occurred while processing your request",
    "details": {
      "hint": "Please contact support with request_id: req_20251223104500_p6q7r8"
    }
  }
}
```

---

## Async Request Pattern

For long-running evaluations, use async mode:

### Step 1: Submit Async Request

**Request:**

```bash
curl -X POST https://yourdomain/v1/decide \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer sk_live_abc123..." \
  -d '{
    "event": { ... },
    "options": {
      "async": true
    }
  }'
```

**Response (202 Accepted):**

```json
{
  "request_id": "req_20251223110000_s9t0u1",
  "status": 202,
  "message": "Request accepted for processing",
  "status_url": "https://yourdomain/v1/decide/status/req_20251223110000_s9t0u1"
}
```

### Step 2: Poll for Results

**Request:**

```bash
curl https://yourdomain/v1/decide/status/req_20251223110000_s9t0u1 \
  -H "Authorization: Bearer sk_live_abc123..."
```

**Response (Processing):**

```json
{
  "request_id": "req_20251223110000_s9t0u1",
  "status": 200,
  "processing_status": "processing",
  "message": "Request is being processed"
}
```

**Response (Completed):**

```json
{
  "request_id": "req_20251223110000_s9t0u1",
  "status": 200,
  "processing_status": "completed",
  "process_time_ms": 145,
  "pipeline_id": "transaction_fraud_detection",
  "decision": {
    "result": "APPROVE",
    "actions": [],
    "scores": {
      "canonical": 250,
      "raw": 25
    },
    "evidence": {
      "triggered_rules": []
    },
    "cognition": {
      "summary": "Transaction approved with low risk score",
      "reason_codes": ["LOW_RISK", "NORMAL_TRANSACTION"]
    }
  },
  "features": {
    "cnt_userid_txn_24h": 5,
    "sum_userid_txn_amt_24h": 1250.0
  }
}
```

---

## Management APIs

### Repository Reload

The repository reload endpoint allows you to reload all rules, rulesets, pipelines, and configurations from the repository without restarting the server. This is useful for updating rules in production environments.

#### Endpoint

```
POST /v1/repo/reload
```

#### Request Headers

| Header | Required | Description |
|--------|----------|-------------|
| `Authorization` | ✅ Yes | API key: `Bearer <api_key>` |

**Note:** This endpoint does not require a request body.

#### Response Format

**HTTP Status:** `200 OK` (on success) or `500 Internal Server Error` (on failure)

**Success Response:**

```json
{
  "success": true,
  "message": "Repository reloaded successfully"
}
```

**Error Response:**

```json
{
  "request_id": "req_20251223110000_abc123",
  "status": 500,
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "Failed to reload repository: <error details>",
    "details": {
      "hint": "Check server logs for more information"
    }
  }
}
```

#### Behavior

When the reload endpoint is called:

1. **Repository Content Reload**: All content is re-read from the configured repository source:
   - Pipelines
   - Rules
   - Rulesets
   - Templates
   - API configurations
   - Data source configurations
   - Feature definitions
   - List configurations

2. **Recompilation**: All loaded content is recompiled into Intermediate Representation (IR) programs

3. **State Preservation**: The following components are preserved during reload:
   - Feature executor (datasources and feature definitions)
   - List service (list backends)
   - Result writer (database connection pool)
   - Metrics and tracing configuration

4. **Atomic Operation**: The reload operation is atomic - either all content is reloaded successfully, or the operation fails and the previous state is maintained

#### Use Cases

- **Hot Reload**: Update rules in production without downtime
- **Configuration Updates**: Apply new datasource or feature configurations
- **Rule Testing**: Quickly test rule changes by reloading from a development repository
- **A/B Testing**: Switch between different rule configurations

#### Example

**Request:**

```bash
curl -X POST https://yourdomain/v1/repo/reload \
  -H "Authorization: Bearer sk_live_abc123..."
```

**Success Response (200 OK):**

```json
{
  "success": true,
  "message": "Repository reloaded successfully"
}
```

**Error Response (500 Internal Server Error):**

```json
{
  "request_id": "req_20251223110000_xyz789",
  "status": 500,
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "Failed to reload repository: Pipeline 'invalid_pipeline' has syntax errors",
    "details": {
      "hint": "Check server logs for more information"
    }
  }
}
```

#### Best Practices

1. **Idempotency**: This endpoint is idempotent - calling it multiple times has the same effect as calling it once
2. **Error Handling**: Always check the `success` field in the response
3. **Logging**: Monitor server logs after reload to ensure all content loaded correctly
4. **Validation**: Validate rule changes in a staging environment before reloading in production
5. **Backup**: Consider backing up current rule configurations before reloading
6. **Rate Limiting**: Avoid calling this endpoint too frequently (e.g., more than once per minute)

#### Security Considerations

- **Authentication Required**: This endpoint requires valid API key authentication
- **Admin Access**: Consider restricting this endpoint to admin users or service accounts only
- **Audit Logging**: All reload operations should be logged for audit purposes

---

## Best Practices

### 1. Request ID Management

- **Server-generated:** Every request is assigned a unique UUID v4 by the server
- **Logging:** Always log `request_id` from the response for debugging and support
- **Tracing:** Use `request_id` to track requests across systems and logs

### 2. Error Handling

```typescript
async function evaluateEvent(event: Event): Promise<Decision> {
  try {
    const response = await fetch('https://yourdomain/v1/decide', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${API_KEY}`
      },
      body: JSON.stringify({ event })
    });

    const data = await response.json();

    // Always check status field
    if (data.status !== 200) {
      console.error(`Request failed: ${data.error.code}`, {
        request_id: data.request_id,
        message: data.error.message,
        details: data.error.details
      });

      // Handle rate limit
      if (data.status === 429) {
        const retryAfter = data.error.retry_after || 60;
        await sleep(retryAfter * 1000);
        return evaluateEvent(event); // Retry
      }

      throw new Error(data.error.message);
    }

    // Return the decision data
    return {
      result: data.decision.result,
      actions: data.decision.actions,
      score: data.decision.scores.canonical,
      triggered_rules: data.decision.evidence.triggered_rules,
      summary: data.decision.cognition.summary,
      reason_codes: data.decision.cognition.reason_codes
    };

  } catch (error) {
    console.error('Request failed:', error);
    throw error;
  }
}
```

### 3. Retry Logic

Implement exponential backoff for server errors:

```typescript
async function evaluateWithRetry(
  event: Event,
  maxRetries = 3
): Promise<Decision> {
  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      return await evaluateEvent(event);
    } catch (error) {
      if (error.status >= 500 && attempt < maxRetries - 1) {
        const delay = Math.pow(2, attempt) * 1000; // Exponential backoff
        await sleep(delay);
        continue;
      }
      throw error;
    }
  }
}
```

### 4. Response Caching

For identical requests, consider caching based on `request_id`:

- Cache TTL: 5-60 seconds (depending on use case)
- Cache key: Hash of request body
- Invalidation: On user profile updates or rule changes

---

## Security Considerations

1. **API Key Security**
   - Never expose API keys in client-side code
   - Use environment variables
   - Rotate keys regularly

2. **Rate Limiting**
   - Default: 1000 requests/minute per API key
   - Upgrade plans available for higher limits

3. **Data Privacy**
   - PII data is not logged by default
   - Enable `audit_logging` for compliance requirements
   - GDPR/CCPA compliant data handling

4. **Transport Security**
   - All requests must use HTTPS
   - TLS 1.2+ required
   - Certificate pinning recommended for mobile apps

---

## Versioning

API version is included in the URL path: `/v1/decide`

- **Current version:** v1
- **Deprecation policy:** 12 months notice before deprecation
- **Breaking changes:** Only in new major versions (v2, v3, etc.)
- **Backward compatibility:** Maintained within major versions

---

## Support

For API support:
- **Documentation:** Contact your system administrator for documentation
- **Status page:** Check your system status page
- **Support:** Contact your support team
- **Emergency:** Include `request_id` in all support requests
