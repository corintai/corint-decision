//! Prompt templates for code generation

/// Prompt template for generating a CORINT Rule
pub const RULE_GENERATION_PROMPT: &str = r#"You are a CORINT decision engine expert. Generate a YAML rule configuration based on the user's description.

CORINT Rule DSL Format:
```yaml
rule:
  id: <unique_identifier>
  description: <clear description>
  when:
    all:  # or 'any'
      - <condition_expression>
      - <condition_expression>
  score: <integer>  # optional
  signal: <approve|decline|review|hold>  # optional
  reason: <string>  # optional
  actions:  # optional
    - <action_name>
```

Expression Examples:
- Field access: event.user.id, event.amount, event.transaction.type
- Comparisons: ==, !=, >, <, >=, <=
- Logical: &&, ||, !
- String operations: contains(), starts_with(), ends_with()
- List operations: in [list], not in [list]
- Feature queries: count(event.user.id, last_24_hours) > 3

User Description:
{description}

Requirements:
1. Generate ONLY valid YAML, no markdown code blocks, no explanations
2. Use proper CORINT DSL syntax
3. Include a descriptive id (snake_case)
4. Add meaningful description
5. Use appropriate operators and field paths
6. Set reasonable score or signal based on risk level
7. DO NOT include any text before or after the YAML
8. The YAML must start with "rule:" at the beginning

Generate the rule now:
"#;

/// Prompt template for generating a CORINT Ruleset
pub const RULESET_GENERATION_PROMPT: &str = r#"You are a CORINT decision engine expert. Generate a YAML ruleset configuration based on the user's description.

CORINT Ruleset DSL Format:
```yaml
ruleset:
  id: <unique_identifier>
  description: <clear description>
  rules:
    - <rule_id_1>
    - <rule_id_2>
  strategy: first_match  # or 'all_match', 'score_sum'
  default_action:
    signal: <signal>
    reason: <string>
```

User Description:
{description}

Requirements:
1. Generate ONLY valid YAML, no markdown code blocks, no explanations
2. Use proper CORINT DSL syntax
3. Include a descriptive id (snake_case)
4. List rule IDs that should be part of this ruleset
5. Choose appropriate strategy
6. DO NOT include any text before or after the YAML
7. The YAML must start with "ruleset:" at the beginning

Generate the ruleset now:
"#;

/// Prompt template for generating a CORINT Pipeline
pub const PIPELINE_GENERATION_PROMPT: &str = r#"You are a CORINT decision engine expert. Generate a YAML pipeline configuration based on the user's description.

CORINT Pipeline DSL Format:
```yaml
pipeline:
  id: <unique_identifier>
  description: <clear description>
  entry: <first_step_id>
  steps:
    - step:
        type: <api|service|ruleset|router>
        id: <step_identifier>
        # For api type:
        api: <api_name>
        endpoint: <endpoint_name>
        output: <variable_name>
        # For ruleset type:
        ruleset: <ruleset_id>
        # For router type:
        routes:
          - when: <condition>
            next: <step_id>
        default: <step_id>
        # Common:
        next: <next_step_id>
```

Step Types:
- api: Call external API
- service: Call internal service
- ruleset: Execute ruleset
- router: Conditional routing based on results

User Description:
{description}

Requirements:
1. Generate ONLY valid YAML, no markdown code blocks, no explanations
2. Use proper CORINT DSL syntax
3. Include a descriptive id (snake_case)
4. Define entry point and all steps
5. Use proper step types and transitions
6. Ensure all step IDs are referenced correctly
7. DO NOT include any text before or after the YAML
8. The YAML must start with "pipeline:" at the beginning

Generate the pipeline now:
"#;

/// Prompt template for generating a complete decision flow
pub const DECISION_FLOW_GENERATION_PROMPT: &str = r#"You are a CORINT decision engine expert. Generate a complete decision flow (pipeline + rulesets + rules) based on the user's description.

User Description:
{description}

Requirements:
1. Analyze the description and identify:
   - What rules are needed (individual conditions)
   - How rules should be grouped into rulesets
   - What pipeline steps are needed
   - What external APIs or services to call

2. Generate multiple YAML documents separated by "---"
3. Start with individual rules, then rulesets, then pipeline
4. Each document must be valid CORINT YAML
5. Ensure all references (rule IDs, ruleset IDs, step IDs) are consistent
6. DO NOT include markdown code blocks or explanations
7. DO NOT include any text before or after the YAML documents

Format:
```
rule:
  id: rule_1
  ...
---
rule:
  id: rule_2
  ...
---
ruleset:
  id: my_ruleset
  rules:
    - rule_1
    - rule_2
  ...
---
pipeline:
  id: my_pipeline
  entry: step_1
  steps:
    - step:
        id: step_1
        type: ruleset
        ruleset: my_ruleset
        ...
```

Generate the complete decision flow now:
"#;

/// Prompt template for generating API configuration
pub const API_CONFIG_GENERATION_PROMPT: &str = r#"You are a CORINT decision engine expert. Generate a YAML API configuration based on the API specification or description.

CORINT API Config DSL Format:
```yaml
name: <api_identifier>
base_url: <base_url>
auth:
  type: header
  name: <header_name>
  value: <value_or_env_var>
timeout_ms: <milliseconds>
endpoints:
  <endpoint_name>:
    method: <GET|POST|PUT|PATCH|DELETE>
    path: <url_path>
    params:
      <param_name>: <context_path>
    query_params:
      - <param_name>
    response:
      mapping:
        <output_field>: <response_field>
      fallback:
        <field>: <value>
```

User Description/API Spec:
{description}

Requirements:
1. Generate ONLY valid YAML, no markdown code blocks, no explanations
2. Use proper CORINT API DSL syntax
3. Include base_url and endpoint definitions
4. Map parameters from context (e.g., event.user.id)
5. Define response mapping if needed
6. DO NOT include any text before or after the YAML
7. The YAML must start with "name:" at the beginning

Generate the API configuration now:
"#;

/// System message for all generation tasks
pub const SYSTEM_MESSAGE: &str = r#"You are an expert in the CORINT decision engine framework. You generate precise, valid YAML configurations following CORINT DSL specifications. You NEVER add explanations, markdown formatting, or any text outside the YAML content. You output ONLY raw YAML that starts immediately with the appropriate top-level key (rule:, ruleset:, pipeline:, or name:)."#;
