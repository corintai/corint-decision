# CORINT Risk Definition Language (RDL)
## Schema and Type System Specification (v0.1)

A robust type system ensures data integrity, enables static analysis, and improves developer experience.  
This document defines CORINT's schema definition language and type system for risk decisioning.

---

## 1. Overview

### 1.1 Type System Goals

- **Type safety**: Catch errors at compile/validation time
- **Documentation**: Self-documenting schemas
- **IDE support**: Enable auto-completion and validation
- **Performance**: Optimize execution based on types
- **Interoperability**: Compatible with JSON Schema, OpenAPI, etc.

---

## 2. Primitive Types

### 2.1 Basic Types

| Type | Description | Examples |
|------|-------------|----------|
| `string` | Text data | `"hello"`, `"user@example.com"` |
| `number` | Numeric (int or float) | `42`, `3.14`, `-100` |
| `integer` | Integer only | `42`, `-100` |
| `float` | Floating point | `3.14`, `0.5` |
| `boolean` | True or false | `true`, `false` |
| `datetime` | ISO 8601 timestamp | `"2024-01-15T10:30:00Z"` |
| `date` | Date only | `"2024-01-15"` |
| `null` | Null value | `null` |

### 2.2 Composite Types

| Type | Description | Examples |
|------|-------------|----------|
| `array<T>` | Array of type T | `array<string>`, `array<number>` |
| `object` | Key-value map | `{key: value}` |
| `map<K, V>` | Typed map | `map<string, number>` |
| `tuple<T1, T2>` | Fixed-size array | `tuple<string, number>` |
| `union<T1, T2>` | One of multiple types | `union<string, number>` |
| `optional<T>` | Nullable type | `optional<string>` |

---

## 3. Schema Definition

### 3.1 Event Schema

```yaml
schema:
  version: "0.1"
  
  # Define event structure
  event:
    type: string
    enum: [login, payment, transfer, withdrawal, registration]
    required: true
    
    timestamp:
      type: datetime
      required: true
      
    id:
      type: string
      format: uuid
      required: true
      
    # Nested objects
    user:
      type: object
      required: true
      properties:
        id:
          type: string
          required: true
          
        email:
          type: string
          format: email
          
        age:
          type: integer
          minimum: 0
          maximum: 150
          
        created_at:
          type: datetime
          
        profile:
          type: object
          properties:
            tier:
              type: string
              enum: [basic, premium, enterprise]
              default: basic
              
            kyc_level:
              type: integer
              minimum: 0
              maximum: 3
              
    device:
      type: object
      properties:
        id: string
        type:
          type: string
          enum: [mobile, desktop, tablet, unknown]
        is_new:
          type: boolean
          default: false
        fingerprint:
          type: string
          pattern: "^[a-f0-9]{32}$"
          
    geo:
      type: object
      properties:
        country:
          type: string
          pattern: "^[A-Z]{2}$"    # ISO 3166-1 alpha-2
        ip:
          type: string
          format: ipv4             # or ipv6
        city: string
        latitude: float
        longitude: float
```

### 3.2 Context Schema

```yaml
schema:
  # Define expected context structure
  context:
    # Results from extract steps
    device_features:
      type: object
      properties:
        device_score:
          type: number
          minimum: 0
          maximum: 100
        is_trusted: boolean
        first_seen: datetime
        
    # Results from LLM reasoning
    llm_analysis:
      type: object
      required: [risk_score, reason]
      properties:
        risk_score:
          type: float
          minimum: 0.0
          maximum: 1.0
          
        risk_level:
          type: string
          enum: [low, medium, high, critical]
          
        reason:
          type: string
          maxLength: 1000
          
        tags:
          type: array<string>
          items:
            type: string
            enum: [
              device_mismatch,
              geo_anomaly,
              behavior_change,
              velocity_spike,
              suspicious_pattern
            ]
            
        confidence:
          type: float
          minimum: 0.0
          maximum: 1.0
```

---

## 4. Type Constraints

### 4.1 Numeric Constraints

```yaml
properties:
  age:
    type: integer
    minimum: 0
    maximum: 150
    
  amount:
    type: number
    minimum: 0
    exclusiveMinimum: true        # > 0, not >= 0
    multipleOf: 0.01              # Allow 2 decimal places
    
  risk_score:
    type: float
    minimum: 0.0
    maximum: 1.0
```

### 4.2 String Constraints

```yaml
properties:
  email:
    type: string
    format: email
    maxLength: 255
    
  country_code:
    type: string
    pattern: "^[A-Z]{2}$"
    minLength: 2
    maxLength: 2
    
  phone:
    type: string
    pattern: "^\\+[1-9]\\d{1,14}$"     # E.164 format
    
  user_id:
    type: string
    format: uuid
```

### 4.3 Array Constraints

```yaml
properties:
  tags:
    type: array<string>
    minItems: 0
    maxItems: 10
    uniqueItems: true
    
  coordinates:
    type: array<float>
    minItems: 2
    maxItems: 2                   # [lat, lon]
    
  transaction_history:
    type: array<object>
    maxItems: 100
```

### 4.4 Object Constraints

```yaml
properties:
  metadata:
    type: object
    additionalProperties: false   # No extra fields allowed
    
  custom_data:
    type: object
    additionalProperties: true    # Allow any extra fields
    
  scores:
    type: map<string, number>
    minProperties: 1
    maxProperties: 10
```

---

## 5. Custom Types

### 5.1 Type Definitions

```yaml
types:
  # Define reusable types
  CountryCode:
    type: string
    pattern: "^[A-Z]{2}$"
    description: "ISO 3166-1 alpha-2 country code"
    
  RiskScore:
    type: float
    minimum: 0.0
    maximum: 1.0
    description: "Normalized risk score"
    
  RiskLevel:
    type: string
    enum: [low, medium, high, critical]
    
  UserId:
    type: string
    format: uuid
    
  TransactionAmount:
    type: number
    minimum: 0
    exclusiveMinimum: true
    multipleOf: 0.01
    
  # Complex type
  Address:
    type: object
    required: [country, city]
    properties:
      street: string
      city: string
      country: CountryCode         # Reference another type
      postal_code: string
      coordinates:
        type: tuple<float, float>
```

### 5.2 Using Custom Types

```yaml
schema:
  event:
    user:
      id: UserId                    # Use custom type
      country: CountryCode
      
    transaction:
      amount: TransactionAmount
      
  context:
    risk_assessment:
      risk_score: RiskScore
      risk_level: RiskLevel
```

---

## 6. Format Validators

### 6.1 Built-in Formats

```yaml
properties:
  # Date/Time formats
  created_at:
    type: string
    format: datetime              # ISO 8601
    
  birth_date:
    type: string
    format: date                  # YYYY-MM-DD
    
  # Network formats
  email:
    type: string
    format: email
    
  ip_address:
    type: string
    format: ipv4                  # or ipv6
    
  website:
    type: string
    format: uri
    
  # Identifiers
  user_id:
    type: string
    format: uuid
    
  # Other
  phone:
    type: string
    format: phone                 # E.164
    
  ssn:
    type: string
    format: ssn
```

### 6.2 Custom Format Validators

```yaml
formats:
  # Define custom formats
  credit_card:
    pattern: "^[0-9]{13,19}$"
    validator: luhn_check
    
  iban:
    pattern: "^[A-Z]{2}[0-9]{2}[A-Z0-9]+$"
    validator: iban_check
    
  btc_address:
    pattern: "^[13][a-km-zA-HJ-NP-Z1-9]{25,34}$"
    validator: base58_check
    
  eth_address:
    pattern: "^0x[a-fA-F0-9]{40}$"
    validator: checksum_address
```

---

## 7. Validation Rules

### 7.1 Field-Level Validation

```yaml
schema:
  event:
    transaction:
      amount:
        type: number
        minimum: 0
        
      currency:
        type: string
        enum: [USD, EUR, GBP, JPY, CNY]
        
      # Cross-field validation
      validation:
        - rule: amount_currency_check
          condition: |
            if currency == "JPY" then
              amount >= 100         # Minimum for JPY
            else
              amount >= 1           # Minimum for others
```

### 7.2 Object-Level Validation

```yaml
schema:
  event:
    type: object
    
    # Object-level constraints
    validation:
      # At least one contact method required
      - name: contact_required
        condition: email exists OR phone exists
        
      # If KYC level 2+, need verified address
      - name: kyc_address_check
        condition: |
          if user.kyc_level >= 2 then
            user.address exists AND user.address_verified == true
            
      # Transaction limits based on user tier
      - name: tier_limit_check
        condition: |
          user.tier == "basic" ? transaction.amount <= 1000 :
          user.tier == "premium" ? transaction.amount <= 10000 :
          transaction.amount <= 100000
```

---

## 8. Schema Versioning

### 8.1 Version Declaration

```yaml
schema:
  version: "1.2.0"
  
  # Backward compatibility
  compatible_with:
    - "1.1.0"
    - "1.0.0"
    
  # Breaking changes from
  breaking_changes_from:
    - "0.9.0"
```

### 8.2 Field Evolution

```yaml
schema:
  event:
    user:
      # Deprecated field
      old_field:
        type: string
        deprecated: true
        deprecated_in: "1.1.0"
        removed_in: "2.0.0"
        replacement: new_field
        
      # New field
      new_field:
        type: string
        added_in: "1.1.0"
        default: ""
```

---

## 9. Schema Inheritance

### 9.1 Base Schemas

```yaml
schemas:
  # Base event schema
  BaseEvent:
    type: object
    required: [type, timestamp, id]
    properties:
      type: string
      timestamp: datetime
      id: string
      
  # Extend base schema
  LoginEvent:
    extends: BaseEvent
    properties:
      type:
        const: login              # Override with specific value
      user: object
      device: object
      geo: object
      
  PaymentEvent:
    extends: BaseEvent
    properties:
      type:
        const: payment
      transaction: object
      merchant: object
```

---

## 10. Schema Composition

### 10.1 AllOf (Intersection)

```yaml
types:
  # Must match all schemas
  VerifiedUser:
    allOf:
      - $ref: "#/types/User"
      - type: object
        required: [email_verified, kyc_completed]
        properties:
          email_verified:
            const: true
          kyc_completed:
            const: true
```

### 10.2 OneOf (Union)

```yaml
types:
  # Must match exactly one schema
  Identifier:
    oneOf:
      - type: string
        format: uuid
        
      - type: string
        format: email
        
      - type: integer
        minimum: 1
```

### 10.3 AnyOf (Union, can match multiple)

```yaml
types:
  ContactMethod:
    anyOf:
      - properties:
          email:
            type: string
            format: email
            
      - properties:
          phone:
            type: string
            format: phone
```

---

## 11. Dynamic Schemas

### 11.1 Conditional Schemas

```yaml
schema:
  event:
    transaction:
      type: object
      
      # Schema depends on transaction type
      if:
        properties:
          type:
            const: crypto_transfer
      then:
        required: [wallet_address, blockchain]
        properties:
          wallet_address: string
          blockchain:
            enum: [bitcoin, ethereum, polygon]
      else:
        required: [account_number, routing_number]
        properties:
          account_number: string
          routing_number: string
```

### 11.2 Dependencies

```yaml
schema:
  event:
    user:
      type: object
      
      # Field dependencies
      dependencies:
        # If credit_card exists, need cvv and expiry
        credit_card:
          required: [cvv, expiry_date]
          
        # If shipping_address exists, need shipping_method
        shipping_address:
          required: [shipping_method]
```

---

## 12. Schema Validation

### 12.1 Validation Configuration

```yaml
pipeline:
  # Validate input at pipeline start
  - type: validate
    id: input_validation
    
    schema: EventSchema           # Reference schema
    
    strict: true                  # Fail on extra fields
    
    on_validation_error:
      action: fail
      return_errors: true         # Return detailed errors
      
    # Coercion
    coerce_types: true            # "123" -> 123
```

### 12.2 Validation Output

```yaml
# Validation error format
validation_errors:
  - field: "event.user.age"
    error: "must be >= 0"
    value: -5
    constraint: "minimum: 0"
    
  - field: "event.transaction.amount"
    error: "required field missing"
    
  - field: "event.geo.country"
    error: "must match pattern ^[A-Z]{2}$"
    value: "usa"
```

---

## 13. Type Inference

### 13.1 Automatic Type Inference

```yaml
# Infer types from usage
pipeline:
  - vars:
      threshold: 80               # Inferred as integer
      rate: 0.5                   # Inferred as float
      name: "test"                # Inferred as string
      active: true                # Inferred as boolean
      countries: ["US", "UK"]     # Inferred as array<string>
```

### 13.2 Type Annotations

```yaml
# Explicit type annotations
pipeline:
  - vars:
      threshold:
        value: 80
        type: number              # Explicitly specify
        
      rate:
        value: "0.5"
        type: float
        coerce: true              # Convert string to float
```

---

## 14. Schema Documentation

### 14.1 Annotated Schema

```yaml
schema:
  event:
    user:
      id:
        type: string
        format: uuid
        description: "Unique user identifier"
        examples:
          - "123e4567-e89b-12d3-a456-426614174000"
          
      risk_score:
        type: float
        minimum: 0.0
        maximum: 1.0
        description: "User risk score (0=safe, 1=high risk)"
        examples:
          - 0.25
          - 0.87
        default: 0.5
```

### 14.2 Schema Generation

```yaml
# Generate documentation from schema
generate:
  markdown: docs/schema.md
  json_schema: schema/event.json
  typescript: types/event.ts
  rust: src/types/event.rs
```

---

## 15. Integration with Rules

### 15.1 Type-Safe Conditions

```yaml
rule:
  id: typed_rule
  
  # Schema aware - IDE can auto-complete
  when:
    all:
      - event.type == "login"     # Type: string, enum
      - event.user.age > 18       # Type: integer
      - event.device.is_new == true  # Type: boolean
      - event.geo.country in ["US", "UK"]  # Type: string
```

### 15.2 Type Checking

```yaml
rule:
  id: type_checked_rule
  
  # Enable compile-time type checking
  type_check: true
  
  when:
    all:
      - event.type == "payment"
      # ✓ Valid: comparing numbers
      - event.transaction.amount > 1000

      # ✗ Error: comparing number to string
      # - event.transaction.amount > "1000"

      # ✓ Valid: string in array
      - event.geo.country in ["US", "UK"]

      # ✗ Error: number in string array
      # - 123 in ["US", "UK"]
```

---

## 16. Performance Optimization

### 16.1 Schema-Based Optimization

```yaml
schema:
  event:
    # Mark fields for indexing
    user:
      id:
        type: string
        index: true               # Enable fast lookup
        
    # Mark fields for caching
    geo:
      country:
        type: string
        cache: true
        cache_ttl: 3600
```

### 16.2 Lazy Loading

```yaml
schema:
  event:
    user:
      detailed_profile:
        type: object
        lazy: true                # Load only when accessed
        source: database
```

---

## 17. Best Practices

### 17.1 Schema Design

✅ **Good Practices:**
- Define clear, explicit types
- Use enums for known values
- Add constraints (min, max, pattern)
- Document with descriptions and examples
- Version your schemas
- Keep schemas DRY with custom types

❌ **Avoid:**
- Overly permissive types (any, object without properties)
- Missing required constraints
- Undocumented fields
- Breaking changes without versioning

### 17.2 Example: Well-Designed Schema

```yaml
schema:
  version: "1.0.0"
  
  types:
    # Reusable types
    RiskScore:
      type: float
      minimum: 0.0
      maximum: 1.0
      description: "Normalized risk score"
      
    CountryCode:
      type: string
      pattern: "^[A-Z]{2}$"
      description: "ISO 3166-1 alpha-2"
      
  event:
    type:
      type: string
      enum: [login, payment, transfer]
      required: true
      description: "Event type"
      
    user:
      type: object
      required: [id, email]
      properties:
        id:
          type: string
          format: uuid
          description: "Unique user ID"
          
        email:
          type: string
          format: email
          maxLength: 255
          
        country:
          type: CountryCode        # Reuse type
          
        risk_score:
          type: RiskScore          # Reuse type
```

---

## 18. Summary

CORINT's type system provides:

- **Strong typing** for data validation and safety
- **Flexible schemas** with inheritance and composition
- **Rich constraints** for precise data validation
- **Custom types** for reusability
- **Versioning** for schema evolution
- **Documentation** generation
- **IDE integration** for better developer experience

A well-defined type system enables:
- Early error detection
- Better tooling support
- Improved performance
- Clearer documentation
- Safer rule execution

