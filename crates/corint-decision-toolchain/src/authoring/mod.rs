//! Static CDL authoring checks. No engine, clients, data sources or actions run.
mod bundle;
mod dependencies;
mod expressions;
mod sources;

use corint_decision_compiler::core::diagnostic;
use corint_decision_compiler::Diagnostic;
use corint_decision_dsl_parser::{PipelineParser, RegistryParser, RuleParser, RulesetParser};
use corint_decision_runtime::{
    feature::FeatureDefinition,
    service::{HttpServiceClient, HttpServiceConfig},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

pub const PROFILE: &str = "cdl-static-1";
pub const SCHEMA: &str = include_str!("../../../../CDL/schema/authoring.json");
pub const INPUT_SCHEMA: &str = include_str!("../../../../CDL/schema/authoring-input.json");

#[derive(Default, Clone)]
pub struct Options {
    /// Explicit files or directories, recursively expanded before validation.
    pub files: Vec<PathBuf>,
    /// Optional root for loading root-relative imports and repository discovery.
    pub root: Option<PathBuf>,
    pub input_schema: Option<PathBuf>,
}

#[derive(Serialize)]
pub struct SkippedSource {
    pub source: String,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub struct Report {
    pub report_version: &'static str,
    pub profile: &'static str,
    pub scope: &'static str,
    pub valid: bool,
    pub execution_checked: bool,
    pub references_checked: bool,
    pub reference_root: Option<String>,
    pub input_schema_checked: bool,
    pub unchecked: Vec<&'static str>,
    pub sources: Vec<String>,
    pub skipped_sources: Vec<SkippedSource>,
    pub diagnostics: Vec<Diagnostic>,
}
impl Report {
    pub fn empty() -> Self {
        Self {
            report_version: "1",
            profile: PROFILE,
            scope: "static",
            valid: false,
            execution_checked: false,
            references_checked: false,
            reference_root: None,
            input_schema_checked: false,
            unchecked: vec![
                "business_behavior",
                "execution_profile_compatibility",
                "external_data_and_services",
                "runtime_result_availability",
            ],
            sources: vec![],
            skipped_sources: vec![],
            diagnostics: vec![],
        }
    }
    pub fn error(
        &mut self,
        source: &str,
        path: &str,
        stage: &str,
        code: &str,
        message: impl Into<String>,
    ) {
        self.diagnostics
            .push(*diagnostic(source, path, stage, code, message).diagnostic);
    }
    pub fn exit_code(&self) -> u8 {
        if self
            .diagnostics
            .iter()
            .any(|d| matches!(d.stage.as_deref(), Some("usage" | "load")))
        {
            2
        } else if self.valid {
            0
        } else {
            1
        }
    }
}

#[derive(Clone)]
struct Document {
    source: String,
    value: Value,
}

fn schema() -> &'static jsonschema::JSONSchema {
    static VALUE: OnceLock<jsonschema::JSONSchema> = OnceLock::new();
    VALUE.get_or_init(|| {
        jsonschema::JSONSchema::compile(&serde_json::from_str(SCHEMA).unwrap())
            .expect("bundled authoring schema")
    })
}

/// Select the resource branch before validation so diagnostics identify the field,
/// rather than merely reporting that the whole document failed `oneOf`.
fn check_shape(doc: &Document, report: &mut Report) -> bool {
    if let Some(kind) = sources::auxiliary_kind(&doc.value) {
        report.error(&doc.source, "", "schema", "E_NOT_CDL", format!("Expected a CDL resource; this file is {kind}. Input Schemas belong in --input-schema, behavior cases in test --cases; reports are not policy resources."));
        return false;
    }
    let root: Value = serde_json::from_str(SCHEMA).unwrap();
    let branch = if ["rule", "ruleset", "pipeline", "registry"]
        .iter()
        .any(|key| doc.value.get(key).is_some())
    {
        0
    } else if doc.value.get("features").is_some() {
        1
    } else if doc.value.get("lists").is_some() {
        3
    } else if doc.value.get("base_url").is_some()
        || doc.value.get("operations").is_some()
        || doc.value.get("name").is_some()
    {
        4
    } else {
        2
    };
    // Full schema remains the authoritative acceptance boundary.
    if schema().is_valid(&doc.value) {
        return true;
    }
    let mut selected = root["oneOf"][branch].clone();
    selected["definitions"] = root["definitions"].clone();
    let compiled = jsonschema::JSONSchema::compile(&selected).unwrap();
    let before = report.diagnostics.len();
    shape_errors(&compiled, &doc.value, &doc.source, "", report);
    if report.diagnostics.len() == before {
        report.error(
            &doc.source,
            "",
            "schema",
            "E_INVALID_STRUCTURE",
            "Expected exactly one CDL resource or resource collection",
        );
    }
    false
}

fn shape_errors(
    schema: &jsonschema::JSONSchema,
    value: &Value,
    source: &str,
    prefix: &str,
    report: &mut Report,
) {
    if let Err(errors) = schema.validate(value) {
        for error in errors.take(32) {
            let path = format!("{prefix}{}", error.instance_path);
            use jsonschema::error::ValidationErrorKind;
            let (code, message) = match error.kind {
                ValidationErrorKind::AdditionalProperties { unexpected } => (
                    "E_UNKNOWN_FIELD",
                    format!("Unknown field(s): {}", unexpected.join(", ")),
                ),
                ValidationErrorKind::Required { property } => {
                    ("E_MISSING_FIELD", format!("Required field: {property}"))
                }
                // Do not echo entire invalid objects: service definitions can contain credentials.
                _ => (
                    "E_INVALID_STRUCTURE",
                    format!("Does not match CDL schema at {}", error.schema_path),
                ),
            };
            report.error(source, &path, "schema", code, message);
        }
    }
}

pub fn validate(options: &Options) -> Report {
    let options = dependencies::options(options);
    let mut report = Report::empty();
    let mut documents = sources::load(&options, &mut report);
    report.reference_root = options.root.as_ref().map(|root| {
        std::fs::canonicalize(root)
            .unwrap_or_else(|_| root.clone())
            .to_string_lossy()
            .into_owned()
    });
    let mut resolver = dependencies::Resolver::new(options.root.clone());
    let checker = expressions::Checker::new(options.input_schema.as_deref(), &mut report);
    report.references_checked = true;
    if options.input_schema.is_none() {
        report.unchecked.push("event_field_schema");
    }
    let mut resources = BTreeMap::<(String, String), (String, Value)>::new();
    let mut refs = vec![];
    let mut graph = BTreeMap::<String, Vec<String>>::new();
    loop {
        for doc in documents {
            if !check_shape(&doc, &mut report) {
                continue;
            }
            let yaml = serde_yaml::to_value(&doc.value).unwrap();
            let parsed = if doc.value.get("rule").is_some() {
                RuleParser::parse_from_yaml(&yaml).map(|_| ())
            } else if doc.value.get("ruleset").is_some() {
                RulesetParser::parse_from_yaml(&yaml).map(|_| ())
            } else if doc.value.get("pipeline").is_some() {
                PipelineParser::parse_from_yaml(&yaml).map(|_| ())
            } else if doc.value.get("registry").is_some() {
                RegistryParser::parse_from_yaml(&yaml).map(|_| ())
            } else {
                Ok(())
            };
            if let Err(error) = parsed {
                let before = report.diagnostics.len();
                checker.conditions(&doc.value, &doc.source, "", &mut refs, &mut report);
                if report.diagnostics.len() == before {
                    report.error(&doc.source, "", "parse", "E_INVALID_CDL", error.to_string());
                }
                continue;
            }
            for kind in ["rule", "ruleset", "pipeline"] {
                if let Some(resource) = doc.value.get(kind) {
                    insert(
                        &mut resources,
                        kind,
                        resource["id"].as_str().unwrap(),
                        resource,
                        &doc.source,
                        &mut report,
                    );
                    checker.conditions(
                        resource,
                        &doc.source,
                        &format!("/{kind}"),
                        &mut refs,
                        &mut report,
                    );
                    if kind == "ruleset" {
                        for (i, rule) in resource["rules"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .enumerate()
                        {
                            refs.push(Reference::new(
                                &doc.source,
                                format!("/ruleset/rules/{i}"),
                                "rule",
                                rule.as_str().unwrap(),
                            ));
                        }
                        if let Some(parent) = resource["extends"].as_str() {
                            refs.push(Reference::new(
                                &doc.source,
                                "/ruleset/extends",
                                "ruleset",
                                parent,
                            ));
                            graph
                                .entry(format!("ruleset:{}", resource["id"].as_str().unwrap()))
                                .or_default()
                                .push(format!("ruleset:{parent}"));
                        }
                    }
                    if kind == "pipeline" {
                        pipeline(resource, &doc.source, &mut refs, &mut graph, &mut report);
                    }
                }
            }
            if let Some(rows) = doc.value["registry"].as_array() {
                insert(
                    &mut resources,
                    "registry",
                    "registry",
                    &doc.value["registry"],
                    &doc.source,
                    &mut report,
                );
                checker.conditions(&doc.value, &doc.source, "", &mut refs, &mut report);
                for (i, row) in rows.iter().enumerate() {
                    refs.push(Reference::new(
                        &doc.source,
                        format!("/registry/{i}/pipeline"),
                        "pipeline",
                        row["pipeline"].as_str().unwrap(),
                    ));
                }
            }
            if let Some(features) = doc.value["features"].as_array() {
                for (i, value) in features.iter().enumerate() {
                    let path = format!("/features/{i}");
                    let name = value["name"].as_str().unwrap();
                    insert(
                        &mut resources,
                        "feature",
                        name,
                        value,
                        &doc.source,
                        &mut report,
                    );
                    let feature: Result<FeatureDefinition, _> =
                        serde_yaml::from_value(serde_yaml::to_value(value).unwrap());
                    match feature {
                        Ok(f) => {
                            if let Err(e) = f.validate() {
                                report.error(
                                    &doc.source,
                                    &path,
                                    "semantic",
                                    "E_INVALID_FEATURE",
                                    e,
                                );
                            }
                        }
                        Err(e) => report.error(
                            &doc.source,
                            &path,
                            "schema",
                            "E_INVALID_FEATURE",
                            e.to_string(),
                        ),
                    }
                    let mut dependencies: Vec<String> = value["dependencies"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect();
                    if let Some(expr) = value["expression"].as_str() {
                        match corint_decision_runtime::feature::expression_dependencies(expr) {
                            Ok(names) => dependencies.extend(names),
                            Err(error) => report.error(
                                &doc.source,
                                &format!("{path}/expression"),
                                "expression",
                                "E_INVALID_EXPRESSION",
                                error.to_string(),
                            ),
                        }
                        checker.expression(
                            expr,
                            &doc.source,
                            &format!("{path}/expression"),
                            false,
                            &mut refs,
                            &mut report,
                        );
                    }
                    checker.templates(value, &doc.source, &path, &mut refs, &mut report);
                    for dep in dependencies {
                        refs.push(Reference::new(&doc.source, &path, "feature", &dep));
                        graph
                            .entry(format!("feature:{name}"))
                            .or_default()
                            .push(format!("feature:{dep}"));
                    }
                }
            }
            if let Some(lists) = doc.value["lists"].as_array() {
                for list in lists {
                    insert(
                        &mut resources,
                        "list",
                        list["id"].as_str().unwrap(),
                        list,
                        &doc.source,
                        &mut report,
                    );
                }
            } else if let Some(id) = doc.value["id"].as_str() {
                insert(
                    &mut resources,
                    "list",
                    id,
                    &doc.value,
                    &doc.source,
                    &mut report,
                );
            }
            if doc.value.get("base_url").is_some() {
                let config: HttpServiceConfig = match serde_json::from_value(doc.value.clone()) {
                    Ok(config) => config,
                    Err(error) => {
                        report.error(
                            &doc.source,
                            "",
                            "schema",
                            "E_INVALID_SERVICE",
                            error.to_string(),
                        );
                        continue;
                    }
                };
                if let Err(error) = HttpServiceClient::validate_config(&config) {
                    report.error(
                        &doc.source,
                        "",
                        "semantic",
                        "E_INVALID_SERVICE",
                        error.to_string(),
                    );
                }
                checker.service(&doc.value, &doc.source, &mut refs, &mut report);
                insert(
                    &mut resources,
                    "service",
                    &config.name,
                    &doc.value,
                    &doc.source,
                    &mut report,
                );
            }
        }
        documents = resolver.load(&refs, &mut report);
        if documents.is_empty() {
            break;
        }
    }
    for reference in refs {
        let key = (reference.kind.clone(), reference.id.clone());
        if let Some((_, value)) = resources.get(&key) {
            if let Some(operation) = reference.operation {
                if value["operations"].get(&operation).is_none() {
                    report.error(
                        &reference.source,
                        &reference.path,
                        "reference",
                        "E_UNKNOWN_OPERATION",
                        format!("Unknown operation {}::{operation}", reference.id),
                    );
                }
            }
        } else {
            report.error(
                &reference.source,
                &reference.path,
                "reference",
                "E_UNRESOLVED_REFERENCE",
                format!(
                    "Unknown {}: {} (not defined in the selected sources; include its definition in the validation inputs)",
                    reference.kind, reference.id
                ),
            );
        }
    }
    if let Some(cycle) = find_cycle(&graph) {
        let source = cycle
            .first()
            .and_then(|key| key.split_once(':'))
            .and_then(|(kind, id)| resources.get(&(kind.into(), id.into())))
            .map(|(source, _)| source.as_str())
            .unwrap_or("");
        report.error(
            source,
            "",
            "reference",
            "E_CYCLE",
            format!("Dependency cycle: {}", cycle.join(" -> ")),
        );
    }
    report.valid = report.diagnostics.is_empty();
    report
}

fn insert(
    resources: &mut BTreeMap<(String, String), (String, Value)>,
    kind: &str,
    id: &str,
    value: &Value,
    source: &str,
    report: &mut Report,
) {
    let key = (kind.to_string(), id.to_string());
    if let Some((previous, _)) = resources.get(&key) {
        report.error(
            source,
            "",
            "reference",
            "E_DUPLICATE_ID",
            format!("Duplicate {kind} {id}; first declared in {previous}"),
        );
    } else {
        resources.insert(key, (source.into(), value.clone()));
    }
}

struct Reference {
    source: String,
    path: String,
    kind: String,
    id: String,
    operation: Option<String>,
}
impl Reference {
    fn new(source: &str, path: impl Into<String>, kind: &str, id: &str) -> Self {
        Self {
            source: source.into(),
            path: path.into(),
            kind: kind.into(),
            id: id.into(),
            operation: None,
        }
    }
}
fn pipeline(
    value: &Value,
    source: &str,
    refs: &mut Vec<Reference>,
    calls: &mut BTreeMap<String, Vec<String>>,
    report: &mut Report,
) {
    let mut edges = BTreeMap::<String, Vec<String>>::new();
    for (i, wrapper) in value["steps"].as_array().unwrap().iter().enumerate() {
        let step = &wrapper["step"];
        let id = step["id"].as_str().unwrap();
        let path = format!("/pipeline/steps/{i}/step");
        let mut next: Vec<String> = ["next", "default"]
            .iter()
            .filter_map(|k| step[k].as_str().map(str::to_owned))
            .collect();
        next.extend(
            step["routes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r["next"].as_str().map(str::to_owned)),
        );
        if edges.insert(id.into(), next).is_some() {
            report.error(
                source,
                &path,
                "reference",
                "E_DUPLICATE_ID",
                format!("Duplicate step: {id}"),
            );
        }
        let kind = step["type"].as_str().unwrap();
        if kind != "router" {
            let target = step[kind].as_str().unwrap();
            let mut reference = Reference::new(source, format!("{path}/{kind}"), kind, target);
            if kind == "service" {
                reference.operation = Some(step["operation"].as_str().unwrap().into());
            }
            refs.push(reference);
            if kind == "pipeline" {
                calls
                    .entry(format!("pipeline:{}", value["id"].as_str().unwrap()))
                    .or_default()
                    .push(format!("pipeline:{target}"));
            }
        }
    }
    let entry = value["entry"].as_str().unwrap();
    for target in std::iter::once(entry).chain(edges.values().flatten().map(String::as_str)) {
        if target != "end" && !edges.contains_key(target) {
            report.error(
                source,
                "/pipeline/steps",
                "reference",
                "E_UNKNOWN_STEP",
                format!("Unknown step: {target}"),
            );
        }
    }
    if let Some(cycle) = find_cycle(&edges) {
        report.error(
            source,
            "/pipeline/steps",
            "reference",
            "E_CYCLE",
            format!("Step cycle: {}", cycle.join(" -> ")),
        );
    }
}

/// Iterative DFS keeps validation bounded by the resource graph, not stack depth.
fn find_cycle(graph: &BTreeMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut done = BTreeSet::new();
    for root in graph.keys() {
        let mut active = BTreeSet::new();
        let mut path = vec![];
        let mut stack = vec![(root.clone(), false)];
        while let Some((node, exit)) = stack.pop() {
            if exit {
                active.remove(&node);
                done.insert(node);
                path.pop();
                continue;
            }
            if active.contains(&node) {
                path.push(node);
                return Some(path);
            }
            if done.contains(&node) {
                continue;
            }
            active.insert(node.clone());
            path.push(node.clone());
            stack.push((node.clone(), true));
            for next in graph.get(&node).into_iter().flatten().rev() {
                stack.push((next.clone(), false));
            }
        }
    }
    None
}
