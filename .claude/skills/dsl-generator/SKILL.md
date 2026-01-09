---
name: dsl-generator
description: Generate syntactically correct YAML files for the Corint Decision Engine repository. Use this skill when the user asks to create, generate, or write DSL configuration files including rules, rulesets, pipelines, features, lists, APIs, or services. This skill guides you through the documentation to understand the DSL syntax and best practices.
---

# DSL Generator Skill

Generate syntactically correct YAML configuration files for Corint Decision Engine by referencing official documentation.

## Capabilities

This skill helps you create:
- **Rules**: Individual detection rules with conditions and scoring
- **Rulesets**: Collections of rules with decision logic
- **Pipelines**: Multi-step workflows with API integrations
- **Features**: Statistical aggregations and transformations
- **Lists**: Custom blocklists/allowlists/watchlists
- **APIs**: External API service definitions
- **Services**: Internal service definitions

## Standard Workflow

**IMPORTANT**: Always follow this workflow when generating DSL files:

### Step 1: Read Overall Documentation First

**ALWAYS** start by reading the overall documentation to understand the framework:

```
Read: docs/dsl/overall.md
```

This gives you:
- Complete DSL architecture overview
- How different components work together
- Import and dependency system
- Context and variable management

### Step 2: Read Specific Documentation

Based on what the user wants to create, read the corresponding detailed documentation:

| User Request | Documentation to Read |
|--------------|----------------------|
| Create a rule | `docs/dsl/rule.md` |
| Create a ruleset | `docs/dsl/ruleset.md` |
| Create a pipeline | `docs/dsl/pipeline.md` |
| Define features | `docs/dsl/feature.md` |
| Create lists | `docs/dsl/list.md` |
| Define external API | `docs/dsl/api.md` |
| Define internal service | `docs/dsl/service.md` |
| Expression syntax | `docs/dsl/expression.md` |
| Context/variables | `docs/dsl/context.md` |
| Import system | `docs/dsl/import.md` |
| Registry system | `docs/dsl/registry.md` |

### Step 3: Generate YAML

After understanding the documentation:
1. Generate syntactically correct YAML
2. Include all required metadata fields
3. Add descriptive comments
4. Use proper indentation (2 spaces)
5. Follow naming conventions (snake_case for IDs)

### Step 4: Validate Against Reference Example

Before saving, validate your generated YAML against the comprehensive reference:

**Reference File**: [`docs/dsl/examples/pipeline_example.yml`](docs/dsl/examples/pipeline_example.yml)

This comprehensive example demonstrates all DSL concepts in a complete, production-ready pipeline:

**What it covers:**
- ✅ All HTTP methods (GET, POST, PUT, PATCH, DELETE)
- ✅ Complex nested conditions (all/any/not with arbitrary nesting)
- ✅ All step types (router, api, service, ruleset, pipeline)
- ✅ Conditional routing and decision logic
- ✅ Context variables and data flow
- ✅ API integration patterns (parallel, fallback)
- ✅ Complete metadata and documentation

**Use it to verify:**
- Syntax correctness (indentation, YAML structure)
- Required fields (id, name, type, version, etc.)
- Naming conventions (snake_case IDs, Title Case names)
- Condition syntax (proper all/any/not nesting)
- Context variable usage (event.*, features.*, api.*, etc.)
- Metadata completeness (version, author, description, tags)
- Best practices (comments, documentation, structure)

**Validation checklist:**
1. ✅ YAML syntax is valid (proper indentation, quotes)
2. ✅ All required fields are present
3. ✅ IDs use snake_case naming
4. ✅ Conditions use correct all/any/not structure
5. ✅ Context variables follow namespace conventions
6. ✅ Metadata includes version, description, timestamps
7. ✅ File structure matches examples in pipeline_example.yml

### Step 5: Confirm Save Location and Save File

**IMPORTANT**: Always ask the user to confirm the save location before writing the file.

**Default Repository Structure:**

```
repository/                                 # Default base directory
├── registry.yaml                          # Pipeline registry
├── library/
│   ├── rules/{category}/{name}.yaml       # Individual rules
│   └── rulesets/{name}.yaml               # Rule collections
├── pipelines/{name}.yaml                  # Pipeline definitions
└── configs/
    ├── features/{name}.yaml               # Feature definitions
    ├── lists/{name}.yaml                  # Custom lists
    ├── apis/{name}.yaml                   # API definitions
    └── services/{name}.yaml               # Service definitions
```

**Before saving, ask the user:**

1. **Confirm base directory** (default: `repository/`)
   - User can specify a different base directory
   - Example: `myrepo/`, etc.

2. **Confirm file path** within the chosen base directory
   - Show the recommended path based on file type
   - Allow user to customize the path

**Example prompts:**

- "I'll save this rule to `repository/library/rules/fraud/high_frequency_login.yaml`. Is this location correct, or would you like to specify a different path?"
- "Where would you like to save this pipeline? (default: `repository/pipelines/fraud_check_flow.yaml`)"
- "Please confirm the save location, or provide a custom path (e.g., `my-rules/custom/my_rule.yaml`):"

## Documentation References

### Core DSL Components

- **[overall.md](docs/dsl/overall.md)** ⭐ - Complete framework overview (READ THIS FIRST)
- **[expression.md](docs/dsl/expression.md)** - Expression language and operators
- **[rule.md](docs/dsl/rule.md)** - Rule specification and patterns
- **[ruleset.md](docs/dsl/ruleset.md)** - Ruleset and decision logic
- **[pipeline.md](docs/dsl/pipeline.md)** - Pipeline orchestration

### Comprehensive Example

- **[pipeline_example.yml](docs/dsl/examples/pipeline_example.yml)** ⭐⭐ - Complete reference example covering all DSL concepts (783 lines, production-ready)

### Advanced Features

- **[feature.md](docs/dsl/feature.md)** ⭐ - Feature engineering and statistical analysis
- **[list.md](docs/dsl/list.md)** ⭐ - Custom lists (blocklists/allowlists)
- **[api.md](docs/dsl/api.md)** - External API definitions
- **[service.md](docs/dsl/service.md)** - Internal service definitions

### Supporting Documentation

- **[import.md](docs/dsl/import.md)** - Import rules or rulesets
- **[context.md](docs/dsl/context.md)** - Context and variable management
- **[registry.md](docs/dsl/registry.md)** - Pipeline registry

## Usage Instructions

When the user asks to generate a DSL file:

1. **Understand the Request**
   - Clarify what they want to create
   - Identify the DSL component type
   - Ask about requirements and constraints

2. **Read Documentation**
   - **ALWAYS** read `overall.md` first
   - Then read the specific documentation for the component
   - Understand syntax, fields, and best practices

3. **Generate YAML**
   - Use syntax from the documentation
   - Include required metadata
   - Add helpful comments
   - Validate structure

4. **Validate Against Reference**
   - Compare with `docs/dsl/examples/pipeline_example.yml`
   - Check syntax, fields, naming conventions
   - Verify metadata completeness

5. **Confirm Location and Save**
   - Ask user to confirm save location (default: `repository/`)
   - Show recommended path based on file type
   - Allow user to specify custom path
   - Write file to confirmed location

## Example Workflow

### Example 1: User Asks to Create a Fraud Detection Rule

```
User: "Create a rule to detect high-frequency login attempts"

Assistant Actions:
1. Read docs/dsl/overall.md (understand framework)
2. Read docs/dsl/rule.md (understand rule syntax)
3. Read docs/dsl/expression.md (understand conditions)
4. Generate rule YAML with:
   - ID: high_frequency_login
   - Conditions using features
   - Appropriate score
   - Complete metadata
5. Validate against docs/dsl/examples/pipeline_example.yml
   - Check syntax and structure
   - Verify metadata fields
6. Ask user to confirm save location:
   - Recommended: repository/library/rules/fraud/high_frequency_login.yaml
   - User can specify custom path
   - Save to confirmed location
```

### Example 2: User Asks to Create a Feature

```
User: "Create a feature to count transactions in the last 24 hours"

Assistant Actions:
1. Read docs/dsl/overall.md (understand framework)
2. Read docs/dsl/feature.md (understand feature types)
3. Generate feature YAML with:
   - Type: aggregation
   - Method: count
   - Window: 24h
   - Datasource configuration
4. Validate against docs/dsl/examples/pipeline_example.yml
   - Check YAML structure
   - Verify required fields
5. Ask user to confirm save location:
   - Recommended: repository/configs/features/transaction_count_24h.yaml
   - User can specify custom path
   - Save to confirmed location
```

### Example 3: User Asks to Create a Pipeline

```
User: "Create a pipeline that checks fraud rules then calls an external API"

Assistant Actions:
1. Read docs/dsl/overall.md (understand framework)
2. Read docs/dsl/pipeline.md (understand pipeline syntax)
3. Read docs/dsl/api.md (understand API step syntax)
4. Generate pipeline YAML with:
   - Entry step
   - Ruleset step
   - API call step
   - Decision logic
5. Validate against docs/dsl/examples/pipeline_example.yml
   - Compare structure with comprehensive example
   - Verify all/any/not condition syntax
   - Check context variable usage
6. Ask user to confirm save location:
   - Recommended: repository/pipelines/fraud_check_flow.yaml
   - User can specify custom path
   - Save to confirmed location
```

## Best Practices

### Naming Conventions
- **IDs**: snake_case (e.g., `fraud_farm_pattern`)
- **Files**: snake_case (e.g., `fraud_farm.yaml`)
- **Names**: Title Case (e.g., "Fraud Farm Detection")

### File Organization
- Group rules by category: fraud, account, device, payment
- Group rulesets by use case: fraud_detection, login_risk
- Group pipelines by workflow: fraud_check, user_verification

### Documentation
- Add clear descriptions
- Document feature requirements
- Explain detection logic
- Include example usage

### Metadata
- Always include: version, author, updated
- Use ISO date format: "2026-01-09 10:00:00"
- Add tags for searchability
- Include category and severity

## Error Prevention

✅ **Do:**
- Read overall.md first
- Read specific documentation for the component
- Use `version: "0.1"` at the top of files
- Use 2-space indentation
- Quote string values with special characters
- Use snake_case for all IDs
- Include all required metadata fields

❌ **Don't:**
- Skip reading documentation
- Guess syntax without checking docs
- Use tabs for indentation
- Use camelCase for IDs
- Omit metadata fields
- Forget the `---` separator in files with imports

---

**Remember**: Documentation is the source of truth. Always read the docs before generating files!
