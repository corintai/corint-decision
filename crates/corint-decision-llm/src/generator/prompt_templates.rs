//! Prompt templates for code generation

/// Shared, executable Core examples; compatibility generators still return candidates.
const CORINT_CDL_SPEC: &str = include_str!("../../../../docs/cdl/cdl-core.md");

pub const RULE_GENERATION_PROMPT: &str = concat!(
    "Generate a single Rule candidate. Output only YAML beginning with rule:. Use this executable shape:\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/rule.yaml"),
    "```\nUse id, name, when and integer score. Matching adds score once; no Rule signal, actions or implicit feature calls. Use declared event fields, scalar comparisons, boolean short circuit and checked arithmetic.\nUser Description:\n{description}\n"
);

pub const RULESET_GENERATION_PROMPT: &str = concat!(
    "Generate a single Ruleset candidate. Output only YAML beginning with ruleset:. Use this executable shape:\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/ruleset.yaml"),
    "```\nRules are ordered references. Conclusion is first-match with one final default. Do not invent strategy, default_action, inheritance or parameters. Resolve references and test the complete closure before use.\nUser Description:\n{description}\n"
);

pub const PIPELINE_GENERATION_PROMPT: &str = concat!(
    "Generate a Pipeline candidate, output only YAML beginning with pipeline:. Executable example:\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/pipeline.yaml"),
    "```\nAll transitions must be explicit. Use one final default decision. Actions are opaque intent strings. Connector, dynamic Feature and Model calls are outside this profile.\nUser Description:\n{description}\n"
);

pub fn build_pipeline_prompt(description: &str) -> String {
    format!(
        "{}\n{}",
        CORINT_CDL_SPEC,
        PIPELINE_GENERATION_PROMPT.replace("{description}", description)
    )
}

pub const DECISION_FLOW_GENERATION_PROMPT: &str = concat!(
    "Generate a complete candidate closure using these executable shapes. Return YAML documents separated by ---; no fences or explanations. All references must resolve. Final decisions belong to Pipeline; no external calls. These are examples, not business acceptance evidence.\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/rule.yaml"),
    "---\n", include_str!("../../../../tests/conformance/generation/ruleset.yaml"),
    "---\n", include_str!("../../../../tests/conformance/generation/pipeline.yaml"),
    "```\nUser Description:\n{description}\n"
);

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
