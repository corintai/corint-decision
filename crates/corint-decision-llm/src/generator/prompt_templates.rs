//! Prompt templates for code generation

/// Read the normative sections from their owning language references. Historical
/// Registry fallback sketches and compatibility namespaces must not enter a Core prompt.
pub(crate) fn core_language_reference() -> String {
    fn section<'a>(document: &'a str, start: &str, end: &str) -> &'a str {
        let start = document.find(start).expect("CDL section start must exist");
        let section = &document[start..];
        let end = section.find(end).expect("CDL section end must exist");
        &section[..end]
    }

    let expressions = include_str!("../../../../CDL/expression.md");
    [
        section(
            include_str!("../../../../CDL/overall.md"),
            "### Document and resource constraints\n",
            "## 2. From sources to a decision\n",
        ),
        section(
            include_str!("../../../../CDL/rule.md"),
            "## 1. Document and fields\n",
            "## 4. Related documentation\n",
        ),
        section(
            include_str!("../../../../CDL/ruleset.md"),
            "## 1. Document and fields\n",
            "## 4. Related documentation\n",
        ),
        section(
            include_str!("../../../../CDL/pipeline.md"),
            "## 1. Pipeline structure\n",
            "## 4. Invalid structures and unsupported capabilities\n",
        ),
        include_str!("../../../../CDL/registry.md"),
        section(
            expressions,
            "## Operator Precedence\n",
            "## Condition Fragments\n",
        ),
        section(
            expressions,
            "### Array Membership\n",
            "### List Membership\n",
        ),
        section(
            expressions,
            "## String Operators\n",
            "## Arithmetic Operators\n",
        ),
        section(
            include_str!("../../../../CDL/context.md"),
            "## Strict Core input and results\n",
            "## Compatibility namespaces\n",
        ),
    ]
    .join("\n")
}

pub const RULE_GENERATION_PROMPT: &str = concat!(
    "Generate a single Rule candidate. Output only YAML beginning with rule:. Use this executable shape:\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/rule.yaml"),
    "```\nUse id, name, when and integer score. Matching adds score once; no Rule signal, actions or implicit feature calls. Use declared event fields, scalar comparisons, boolean short circuit and checked arithmetic.\nUser Description:\n{description}\n"
);

pub const RULESET_GENERATION_PROMPT: &str = concat!(
    "Generate a single Ruleset candidate. Output only YAML beginning with ruleset:. Use this executable shape:\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/ruleset.yaml"),
    "```\nRules are ordered references. Write ruleset.rules as a nonempty YAML block sequence, one - rule_id per line; never use flow lists or aliases. Conclusion is first-match with one final default. Do not invent strategy, default_action, inheritance or parameters. Resolve references and test the complete closure before use.\nUser Description:\n{description}\n"
);

pub const PIPELINE_GENERATION_PROMPT: &str = concat!(
    "Generate a Pipeline candidate, output only YAML beginning with pipeline:. Executable example:\n```yaml\n",
    include_str!("../../../../tests/conformance/generation/pipeline.yaml"),
    "```\nAll transitions must be explicit. Use one final default decision. Actions are opaque intent strings. Connector, dynamic Feature and Model calls are outside this profile.\nUser Description:\n{description}\n"
);

pub fn build_pipeline_prompt(description: &str) -> String {
    format!(
        "{}\n{}",
        core_language_reference(),
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

/// Prompt template for generating HTTP service binding
pub const SERVICE_CONFIG_GENERATION_PROMPT: &str = r#"You are a CORINT decision engine expert. Generate a YAML HTTP service binding based on the API specification or description.

CORINT Service Binding DSL Format:
```yaml
name: <service_identifier>
base_url: <base_url>
auth:
  type: header
  name: <header_name>
  value: <resolved_header_value>
timeout_ms: <milliseconds>
operations:
  <operation_name>:
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
2. Use proper CORINT Service DSL syntax
3. Include base_url and operation definitions
4. Map parameters from context (e.g., event.user.id)
5. Define response mapping if needed
6. DO NOT include any text before or after the YAML
7. The YAML must start with "name:" at the beginning

Generate the HTTP service binding now:
"#;

/// System message for all generation tasks
pub const SYSTEM_MESSAGE: &str = r#"You are an expert in the CORINT decision engine framework. You generate precise, valid YAML configurations following CORINT DSL specifications. You NEVER add explanations, markdown formatting, or any text outside the YAML content. You output ONLY raw YAML that starts immediately with the appropriate top-level key (rule:, ruleset:, pipeline:, or name:)."#;
