# CORINT Risk Definition Language (RDL)
## Metadata Specification (v0.1)

Metadata provides versioning, authorship, and management information for all RDL components (Rules, Rulesets, and Pipelines).

---

## 1. Overview

All RDL components **may** include an optional `metadata` field. **Strongly recommended** for production use.

When included, metadata should contain:
- **Recommended fields**: `version`, `author`, `updated`
- **Custom fields**: Any additional key/value pairs

---

## 2. BNF Grammar

```bnf
<metadata>        ::= "metadata:" <metadata_fields>

<metadata_fields> ::= <recommended_fields> <custom_fields>?

<recommended_fields> ::= "version:" <version_string> "\n"
                         "author:" <author_string> "\n"
                         "updated:" <timestamp_string> "\n"

<version_string>  ::= <semver>  // Format: "MAJOR.MINOR.PATCH", e.g., "1.0.0"

<author_string>   ::= <string>  // Free text, e.g., "Risk Team"

<timestamp_string> ::= <datetime>  // Format: "YYYY-MM-DD HH:mm:ss", e.g., "2025-12-20 14:30:00"

<custom_fields>   ::= <custom_field> ( "\n" <custom_field> )*

<custom_field>    ::= <field_name> ":" <field_value>

<field_name>      ::= <identifier>  // snake_case recommended

<field_value>     ::= <string> | <number> | <boolean> | <array> | <object>
```

---

## 3. Field Definitions

### 3.1 Recommended Fields

| Field | Type | Format | Description |
|-------|------|--------|-------------|
| `version` | string | `MAJOR.MINOR.PATCH` | Semantic version (e.g., "1.0.0") |
| `author` | string | Free text | Creator/maintainer (e.g., "Risk Team") |
| `updated` | string | `YYYY-MM-DD HH:mm:ss` | Last modification timestamp |

### 3.2 Common Custom Fields

| Field | Type | Description |
|-------|------|-------------|
| `owner` | string | Responsible team/individual |
| `category` | string | Functional category |
| `tags` | array | Searchable tags |
| `environment` | string | Deployment environment (dev/staging/production) |
| `status` | string | Development status (draft/active/deprecated) |

---

## 4. Syntax Examples

### 4.1 Minimal Example

```yaml
rule:
  id: simple_check
  metadata:
    version: "1.0.0"
    author: "Risk Team"
    updated: "2025-12-20 10:00:00"
```

### 4.2 With Custom Fields

```yaml
ruleset:
  id: fraud_detection
  metadata:
    version: "2.0.0"
    author: "Fraud Team"
    updated: "2025-12-20 11:00:00"
    owner: "fraud_team"
    category: "fraud_detection"
    tags: [fraud, real-time]
    environment: "production"
```

---

## 5. Core Concepts

- **Optional**: Metadata field is optional (not enforced by compiler/runtime)
- **Strongly Recommended**: Include metadata for production use
- **Flexible**: Any key/value pairs allowed beyond recommended fields
- **Consistent**: Same format across Rules, Rulesets, and Pipelines
- **Extensible**: Add organization-specific fields as needed

---

## 6. Format Rules

- **Version**: Semantic versioning (`MAJOR.MINOR.PATCH`)
- **Timestamp**: `YYYY-MM-DD HH:mm:ss` (24-hour format)
- **Field names**: Use `snake_case` convention
- **Values**: Support string, number, boolean, array, object

---

**Version**: 0.1  
**Target**: LLM-readable DSL specification
