//! Opt-in, closed-world CDL Core validation. Legacy parsers remain unchanged.
//! The embedded public schema is the single structural contract; semantic checks
//! below use the existing expression AST and compiler, not a second interpreter.

use crate::{Compiler, Diagnostic};
use corint_decision_dsl_parser::{
    ExpressionParser, PipelineParser, RegistryParser, RuleParser, RulesetParser,
};
use corint_decision_model::ast::{
    Condition, ConditionGroup, Expression, LogicalGroupOp, Operator, PipelineRegistry,
    UnaryOperator, WhenBlock,
};
use corint_decision_model::ir::condition_map::{
    ConditionMap, CONDITION_MAP, DECISION_CONDITION_MAP,
};
use corint_decision_model::ir::Instruction;
use corint_decision_model::ir::Program;
use corint_decision_model::types::{FieldType, Schema};
use corint_decision_model::Value;
use jsonschema::{error::ValidationErrorKind, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::OnceLock;

pub const PROFILE: &str = "cdl-core-risk-draft-1";
pub const CORE_SCHEMA: &str = include_str!("../../../docs/cdl/schema/core.json");
pub const CORE_INPUT_SCHEMA: &str = include_str!("../../../docs/cdl/schema/input.json");

/// A source supplied explicitly by the caller, never implicitly loaded from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreSource {
    pub path: String,
    pub yaml: String,
}

#[derive(Debug, thiserror::Error, Serialize)]
#[error("{diagnostic:?}")]
pub struct CoreError {
    #[serde(flatten)]
    pub diagnostic: Box<Diagnostic>,
}

pub type CoreResult<T> = std::result::Result<T, CoreError>;

pub fn diagnostic(
    source: &str,
    path: &str,
    stage: &str,
    code: &str,
    message: impl Into<String>,
) -> CoreError {
    let mut value = Diagnostic::error(code, message);
    value.source = Some(source.into());
    value.field_path = Some(path.into());
    value.stage = Some(stage.into());
    CoreError {
        diagnostic: Box::new(value),
    }
}

/// Validated resource shape. Construction is only possible through the gate.
#[derive(Debug)]
pub struct CoreDocument {
    source: String,
    value: Json,
}

/// Compiled programs and their explicit registry, before runtime assembly.
#[derive(Debug)]
pub struct CompiledCore {
    pub programs: Vec<Program>,
    pub registry: PipelineRegistry,
    /// Compiled with the same Rule compiler, in Registry entry order.
    pub registry_guards: Vec<Program>,
    pub input_schema: Schema,
}

fn schema() -> &'static JSONSchema {
    static SCHEMA: OnceLock<JSONSchema> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        JSONSchema::compile(&serde_json::from_str(CORE_SCHEMA).expect("embedded schema JSON"))
            .expect("embedded Core schema is valid")
    })
}

fn parse_yaml(source: &CoreSource) -> CoreResult<Json> {
    // Parsing directly to YAML Value rejects duplicate keys at every depth.
    let yaml: serde_yaml::Value = serde_yaml::from_str(&source.yaml).map_err(|e| {
        let mut err = diagnostic(
            &source.path,
            "",
            "parse",
            "E_INVALID_STRUCTURE",
            e.to_string(),
        );
        if let Some(location) = e.location() {
            err.diagnostic.line = Some(location.line());
            err.diagnostic.column = Some(location.column());
        }
        err
    })?;
    serde_json::to_value(yaml).map_err(|e| {
        diagnostic(
            &source.path,
            "",
            "parse",
            "E_INVALID_STRUCTURE",
            e.to_string(),
        )
    })
}

/// Parse the strict file representation of the existing input Schema. This is
/// shared by local tools; it does not change legacy Schema deserialization.
pub fn parse_core_input_schema(source: &CoreSource) -> CoreResult<Schema> {
    static INPUT_SCHEMA: OnceLock<JSONSchema> = OnceLock::new();
    let validator = INPUT_SCHEMA.get_or_init(|| {
        JSONSchema::compile(&serde_json::from_str(CORE_INPUT_SCHEMA).expect("input schema JSON"))
            .expect("embedded input schema is valid")
    });
    let value = parse_yaml(source)?;
    validate_shape(validator, &source.path, &value)?;
    let input: Schema = serde_json::from_value(value).map_err(|e| {
        diagnostic(
            &source.path,
            "",
            "validate",
            "E_INPUT_SCHEMA",
            e.to_string(),
        )
    })?;
    validate_input_schema(&input).map_err(|mut err| {
        err.diagnostic.source = Some(source.path.clone());
        err
    })?;
    Ok(input)
}

/// Validate one document, including version/capability checks. No compatibility fallback.
pub fn validate_core_document(source: &CoreSource) -> CoreResult<CoreDocument> {
    let mut value = parse_yaml(source)?;
    match value.get("version") {
        Some(Json::String(v)) if v == "0.1" => (),
        Some(Json::String(_)) => {
            return Err(diagnostic(
                &source.path,
                "/version",
                "validate",
                "E_UNSUPPORTED_VERSION",
                "Only explicit language version 0.1 is accepted",
            ))
        }
        _ => {
            return Err(diagnostic(
                &source.path,
                "/version",
                "validate",
                "E_INVALID_VERSION",
                "An explicit string language version is required",
            ))
        }
    }
    capability_gate(&source.path, "", &value)?;
    validate_shape(schema(), &source.path, &value)?;
    // JSON Schema integer means an integral value (60.0 is also an integer).
    // Normalize it losslessly after the i32 range check for the existing parser.
    if let Some(rule) = value.get_mut("rule") {
        let score = rule["score"].as_f64().expect("validated score");
        rule["score"] = Json::from(score as i32);
    }
    Ok(CoreDocument {
        source: source.path.clone(),
        value,
    })
}

fn validate_shape(validator: &JSONSchema, source: &str, value: &Json) -> CoreResult<()> {
    if let Err(mut errors) = validator.validate(value) {
        if let Some(e) = errors.next() {
            let code = match e.kind {
                ValidationErrorKind::AdditionalProperties { .. } => "E_UNKNOWN_FIELD",
                ValidationErrorKind::Required { .. } => "E_MISSING_FIELD",
                _ => "E_INVALID_STRUCTURE",
            };
            return Err(diagnostic(
                source,
                &e.instance_path.to_string(),
                "validate",
                code,
                e.to_string(),
            ));
        }
    }
    Ok(())
}

fn capability_gate(source: &str, path: &str, value: &Json) -> CoreResult<()> {
    match value {
        Json::Object(map) => {
            for (key, child) in map {
                let p = format!("{path}/{key}");
                let unsupported = (path.is_empty()
                    && ["import", "imports"].contains(&key.as_str()))
                    || (path == "/pipeline" && key == "when")
                    || (path.starts_with("/pipeline/steps/")
                        && !path.contains("/routes/")
                        && key == "when")
                    || (path == "/ruleset" && key == "extends")
                    || (path == "/rule" && key == "params")
                    || (path.starts_with("/pipeline/steps/")
                        && key == "type"
                        && child
                            .as_str()
                            .is_some_and(|v| !["ruleset", "router"].contains(&v)));
                if unsupported {
                    return Err(diagnostic(
                        source,
                        &p,
                        "validate",
                        "E_UNSUPPORTED_CAPABILITY",
                        format!("Not enabled by {PROFILE}"),
                    ));
                }
                // Surface spelling errors inside condition objects, including nested groups.
                if (path.ends_with("/when") || path.contains("/when/"))
                    && !["all", "any", "not"].contains(&key.as_str())
                {
                    return Err(diagnostic(
                        source,
                        &p,
                        "validate",
                        "E_UNKNOWN_FIELD",
                        "Unknown condition group",
                    ));
                }
                capability_gate(source, &p, child)?;
            }
        }
        Json::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                capability_gate(source, &format!("{path}/{i}"), item)?;
            }
        }
        _ => (),
    }
    Ok(())
}

/// Compile a complete in-memory resource closure using the public parsers/compiler.
/// Import resolution is intentionally not enabled in draft increment 1.
pub fn compile_core(sources: &[CoreSource], input_schema: Schema) -> CoreResult<CompiledCore> {
    validate_input_schema(&input_schema)?;
    let docs = sources
        .iter()
        .map(validate_core_document)
        .collect::<CoreResult<Vec<_>>>()?;
    let mut resources = BTreeMap::new();
    let mut registry_doc = None;
    let mut source_names = BTreeSet::new();
    for doc in &docs {
        if !source_names.insert(&doc.source) {
            return Err(diagnostic(
                &doc.source,
                "",
                "resolve",
                "E_DUPLICATE_ID",
                "Duplicate source path",
            ));
        }
        if doc.value.get("registry").is_some() {
            if registry_doc.replace(doc).is_some() {
                return Err(diagnostic(
                    &doc.source,
                    "/registry",
                    "resolve",
                    "E_DUPLICATE_ID",
                    "Exactly one registry is required",
                ));
            }
        } else {
            let kind = ["rule", "ruleset", "pipeline"]
                .into_iter()
                .find(|k| doc.value.get(k).is_some())
                .unwrap();
            let id = doc.value[kind]["id"].as_str().unwrap();
            if resources.insert(id, (kind, doc)).is_some() {
                return Err(diagnostic(
                    &doc.source,
                    &format!("/{kind}/id"),
                    "resolve",
                    "E_DUPLICATE_ID",
                    id,
                ));
            }
        }
    }
    let registry_doc = registry_doc.ok_or_else(|| {
        diagnostic(
            "<bundle>",
            "/registry",
            "resolve",
            "E_MISSING_FIELD",
            "Exactly one registry is required",
        )
    })?;
    let empty = BTreeSet::new();
    // Core source maps address the emitted instruction stream. The legacy
    // optimizer removes instructions without relocating jumps or debug maps.
    let mut compiler = Compiler::with_options(crate::CompilerOptions {
        enable_dead_code_elimination: false,
        ..Default::default()
    });
    let mut programs = Vec::new();
    for (kind, doc) in resources.values() {
        let body = &doc.value[*kind];
        let yaml = serde_yaml::to_value(&doc.value).expect("JSON is YAML-compatible");
        let parse_error = |e: corint_decision_dsl_parser::ParseError| {
            diagnostic(
                &doc.source,
                &format!("/{kind}"),
                "parse",
                "E_INVALID_STRUCTURE",
                e.to_string(),
            )
        };
        let mut program = match *kind {
            "rule" => {
                check_condition(
                    &body["when"],
                    &input_schema,
                    false,
                    &empty,
                    &doc.source,
                    "/rule/when",
                )?;
                let mut rule = RuleParser::parse_from_yaml(&yaml).map_err(parse_error)?;
                normalize_when(&mut rule.when);
                compiler.compile_rule(&rule).and_then(|mut program| {
                    map_rule(&mut program, &rule.when, "/rule/when".into())?;
                    Ok(program)
                })
            }
            "ruleset" => {
                for id in body["rules"].as_array().unwrap() {
                    require_resource(
                        &resources,
                        id.as_str().unwrap(),
                        "rule",
                        &doc.source,
                        "/ruleset/rules",
                    )?;
                }
                check_defaults(&body["conclusion"], &doc.source, "/ruleset/conclusion")?;
                for (i, row) in body["conclusion"].as_array().unwrap().iter().enumerate() {
                    if let Some(when) = row.get("when") {
                        check_condition(
                            when,
                            &input_schema,
                            true,
                            &empty,
                            &doc.source,
                            &format!("/ruleset/conclusion/{i}/when"),
                        )?;
                    }
                }
                let mut ruleset = RulesetParser::parse_from_yaml(&yaml).map_err(parse_error)?;
                for row in &mut ruleset.conclusion {
                    if let Some(expr) = row.condition.take() {
                        row.condition = Some(normalize_expression(expr));
                    }
                }
                compiler.compile_ruleset(&ruleset).and_then(|mut program| {
                    let ends = signal_positions(&program.instructions);
                    let mut maps = Vec::new();
                    for (index, row) in ruleset.conclusion.iter().enumerate() {
                        if let Some(expr) = &row.condition {
                            maps.push(crate::core_trace::condition_map(
                                &program.instructions,
                                expr,
                                ends[index] - 1,
                                format!("/ruleset/conclusion/{index}/when"),
                            )?);
                        }
                    }
                    store_maps(&mut program, CONDITION_MAP, maps);
                    Ok(program)
                })
            }
            "pipeline" => {
                check_pipeline(body, &resources, &input_schema, &doc.source)?;
                let mut pipeline = PipelineParser::parse_from_yaml(&yaml).map_err(parse_error)?;
                for step in &mut pipeline.steps {
                    if let Some(routes) = &mut step.routes {
                        for route in routes {
                            normalize_when(&mut route.when);
                        }
                    }
                }
                for row in pipeline.decision.as_mut().unwrap() {
                    if let Some(when) = &mut row.when {
                        normalize_when(when);
                    }
                }
                compiler
                    .compile_pipeline(&pipeline)
                    .and_then(|mut program| {
                        map_pipeline(&mut program, &pipeline)?;
                        Ok(program)
                    })
            }
            _ => unreachable!(),
        }
        .map_err(|e| {
            diagnostic(
                &doc.source,
                &format!("/{kind}"),
                "compile",
                "E_COMPILE",
                e.to_string(),
            )
        })?;
        program
            .metadata
            .custom
            .insert("core_source".into(), doc.source.clone());
        programs.push(program);
    }
    for (i, entry) in registry_doc.value["registry"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        require_resource(
            &resources,
            entry["pipeline"].as_str().unwrap(),
            "pipeline",
            &registry_doc.source,
            &format!("/registry/{i}/pipeline"),
        )?;
        check_condition(
            &entry["when"],
            &input_schema,
            false,
            &empty,
            &registry_doc.source,
            &format!("/registry/{i}/when"),
        )?;
    }
    let mut registry = RegistryParser::parse(&registry_doc.value.to_string()).map_err(|e| {
        diagnostic(
            &registry_doc.source,
            "/registry",
            "parse",
            "E_INVALID_STRUCTURE",
            e.to_string(),
        )
    })?;
    for entry in &mut registry.registry {
        normalize_when(&mut entry.when);
    }
    // Compile here, not during engine assembly: CLI and Engine must check the
    // same complete bundle, including Registry condition code generation.
    let mut registry_guards = Vec::new();
    for (i, entry) in registry.registry.iter().enumerate() {
        let rule = corint_decision_model::ast::Rule {
            id: format!("__core_registry_{i}"),
            name: "Registry guard".into(),
            description: None,
            params: None,
            metadata: None,
            when: entry.when.clone(),
            score: 1,
        };
        let mut guard = crate::RuleCompiler::compile(&rule)
            .and_then(|mut guard| {
                map_rule(&mut guard, &entry.when, format!("/registry/{i}/when"))?;
                Ok(guard)
            })
            .map_err(|e| {
                diagnostic(
                    &registry_doc.source,
                    &format!("/registry/{i}/when"),
                    "compile",
                    "E_COMPILE",
                    e.to_string(),
                )
            })?;
        guard.metadata.source_type = "registry".into();
        guard.metadata.source_id = "registry".into();
        guard
            .metadata
            .custom
            .insert("core_source".into(), registry_doc.source.clone());
        registry_guards.push(guard);
    }
    Ok(CompiledCore {
        programs,
        registry,
        registry_guards,
        input_schema,
    })
}

type Resources<'a> = BTreeMap<&'a str, (&'a str, &'a CoreDocument)>;

fn normalized_condition(when: &WhenBlock) -> &Expression {
    &when.conditions.as_ref().expect("normalized when")[0]
}

fn map_pipeline(
    program: &mut Program,
    pipeline: &corint_decision_model::ast::Pipeline,
) -> crate::Result<()> {
    let mut maps = Vec::new();
    for (index, step) in pipeline.steps.iter().enumerate() {
        for (route_index, route) in step.routes.iter().flatten().enumerate() {
            let end = program
                .instructions
                .iter()
                .position(|inst| {
                    matches!(inst, Instruction::MarkStepExecuted {
                    step_id, route_index: Some(i), ..
                } if step_id == &step.id && *i == route_index)
                })
                .and_then(|i| i.checked_sub(1))
                .ok_or_else(|| {
                    crate::CompileError::InvalidExpression(
                        "Missing Core route observation boundary".into(),
                    )
                })?;
            maps.push(crate::core_trace::condition_map(
                &program.instructions,
                normalized_condition(&route.when),
                end,
                format!("/pipeline/steps/{index}/step/routes/{route_index}/when"),
            )?);
        }
    }
    store_maps(program, CONDITION_MAP, maps);
    let instructions = program.decision_instructions.as_ref().unwrap();
    let ends = signal_positions(instructions);
    let mut maps = Vec::new();
    for (index, row) in pipeline.decision.as_ref().unwrap().iter().enumerate() {
        if let Some(when) = &row.when {
            maps.push(crate::core_trace::condition_map(
                instructions,
                normalized_condition(when),
                ends[index] - 1,
                format!("/pipeline/decision/{index}/when"),
            )?);
        }
    }
    store_maps(program, DECISION_CONDITION_MAP, maps);
    Ok(())
}

fn signal_positions(instructions: &[Instruction]) -> Vec<usize> {
    instructions
        .iter()
        .enumerate()
        .filter_map(|(i, inst)| matches!(inst, Instruction::SetSignal { .. }).then_some(i))
        .collect()
}

fn store_maps(program: &mut Program, key: &str, maps: Vec<ConditionMap>) {
    program.metadata.custom.insert(
        key.into(),
        serde_json::to_string(&maps).expect("source map JSON"),
    );
}

fn map_rule(program: &mut Program, when: &WhenBlock, path: String) -> crate::Result<()> {
    let expr = normalized_condition(when);
    let end = crate::codegen::ExpressionCompiler::compile(expr)?.len();
    let map = crate::core_trace::condition_map(&program.instructions, expr, end, path)?;
    store_maps(program, CONDITION_MAP, vec![map]);
    Ok(())
}

fn require_resource(
    resources: &Resources<'_>,
    id: &str,
    kind: &str,
    source: &str,
    path: &str,
) -> CoreResult<()> {
    if resources.get(id).is_some_and(|(k, _)| *k == kind) {
        Ok(())
    } else {
        Err(diagnostic(
            source,
            path,
            "resolve",
            "E_UNRESOLVED_REF",
            format!("Unknown {kind}: {id}"),
        ))
    }
}

fn check_defaults(rows: &Json, source: &str, path: &str) -> CoreResult<()> {
    let rows = rows.as_array().unwrap();
    let defaults: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.get("default") == Some(&Json::Bool(true)))
        .map(|(i, _)| i)
        .collect();
    if defaults != [rows.len() - 1] {
        return Err(diagnostic(
            source,
            path,
            "validate",
            "E_INVALID_STRUCTURE",
            "Exactly one default must be last",
        ));
    }
    Ok(())
}

fn check_pipeline(
    body: &Json,
    resources: &Resources<'_>,
    schema: &Schema,
    source: &str,
) -> CoreResult<()> {
    check_defaults(&body["decision"], source, "/pipeline/decision")?;
    let mut steps = BTreeMap::new();
    let mut called_rulesets = BTreeSet::new();
    for (i, wrapper) in body["steps"].as_array().unwrap().iter().enumerate() {
        let step = &wrapper["step"];
        if let Some(id) = step.get("ruleset").and_then(Json::as_str) {
            if !called_rulesets.insert(id) {
                return Err(diagnostic(
                    source,
                    &format!("/pipeline/steps/{i}/step/ruleset"),
                    "resolve",
                    "E_INVALID_GRAPH",
                    "Increment 1 allows one call site per ruleset in a pipeline",
                ));
            }
        }
        if steps
            .insert(step["id"].as_str().unwrap(), (i, step))
            .is_some()
        {
            return Err(diagnostic(
                source,
                &format!("/pipeline/steps/{i}/step/id"),
                "resolve",
                "E_DUPLICATE_ID",
                "Duplicate step ID",
            ));
        }
    }
    let entry = body["entry"].as_str().unwrap();
    if !steps.contains_key(entry) {
        return Err(diagnostic(
            source,
            "/pipeline/entry",
            "resolve",
            "E_INVALID_GRAPH",
            "Unknown entry",
        ));
    }
    let mut edges = BTreeMap::new();
    let mut incoming: BTreeMap<&str, usize> = steps.keys().map(|id| (*id, 0)).collect();
    incoming.insert("end", 0);
    for (id, (i, step)) in &steps {
        let targets = if step["type"] == "ruleset" {
            require_resource(
                resources,
                step["ruleset"].as_str().unwrap(),
                "ruleset",
                source,
                &format!("/pipeline/steps/{i}/step/ruleset"),
            )?;
            vec![step["next"].as_str().unwrap()]
        } else {
            step["routes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["next"].as_str().unwrap())
                .chain(std::iter::once(step["default"].as_str().unwrap()))
                .collect()
        };
        for target in &targets {
            let count = incoming.get_mut(target).ok_or_else(|| {
                diagnostic(
                    source,
                    &format!("/pipeline/steps/{i}"),
                    "resolve",
                    "E_INVALID_GRAPH",
                    format!("Unknown target: {target}"),
                )
            })?;
            *count += 1;
        }
        edges.insert(*id, targets);
    }
    if incoming[entry] != 0 {
        return Err(diagnostic(
            source,
            "/pipeline/entry",
            "resolve",
            "E_INVALID_GRAPH",
            "Entry must have no predecessor",
        ));
    }
    // Intersect completed results at every join. A reference must be defined on
    // ALL incoming paths, including early exits; no missing-result-as-null fallback.
    let mut available: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
    available.insert(entry, BTreeSet::new());
    let mut queue = VecDeque::from([entry]);
    let mut visited = 0;
    while let Some(id) = queue.pop_front() {
        if id == "end" {
            continue;
        }
        visited += 1;
        let (i, step) = steps[id];
        let mut output = available[id].clone();
        if step["type"] == "ruleset" {
            if !output.insert(step["ruleset"].as_str().unwrap().to_owned()) {
                return Err(diagnostic(
                    source,
                    &format!("/pipeline/steps/{i}"),
                    "resolve",
                    "E_INVALID_GRAPH",
                    "A ruleset may execute at most once per path in this increment",
                ));
            }
        } else {
            for (j, route) in step["routes"].as_array().unwrap().iter().enumerate() {
                check_condition(
                    &route["when"],
                    schema,
                    false,
                    &output,
                    source,
                    &format!("/pipeline/steps/{i}/step/routes/{j}/when"),
                )?;
            }
        }
        for target in &edges[id] {
            available
                .entry(target)
                .and_modify(|v| v.retain(|x| output.contains(x)))
                .or_insert_with(|| output.clone());
            *incoming.get_mut(target).unwrap() -= 1;
            if incoming[target] == 0 {
                queue.push_back(target);
            }
        }
    }
    if visited != steps.len() || incoming["end"] != 0 || !available.contains_key("end") {
        return Err(diagnostic(
            source,
            "/pipeline/steps",
            "resolve",
            "E_INVALID_GRAPH",
            "Cycle, unreachable step, or missing exit",
        ));
    }
    for (i, row) in body["decision"].as_array().unwrap().iter().enumerate() {
        if let Some(when) = row.get("when") {
            check_condition(
                when,
                schema,
                false,
                &available["end"],
                source,
                &format!("/pipeline/decision/{i}/when"),
            )?;
        }
    }
    Ok(())
}

fn check_condition(
    value: &Json,
    schema: &Schema,
    aggregate: bool,
    available: &BTreeSet<String>,
    source: &str,
    path: &str,
) -> CoreResult<()> {
    if let Some(text) = value.as_str() {
        let expr = ExpressionParser::parse(text).map_err(|e| {
            diagnostic(source, path, "parse", "E_INVALID_EXPRESSION", e.to_string())
        })?;
        if expression_type(&expr, schema, aggregate, available, source, path)? != FieldType::Boolean
        {
            return Err(diagnostic(
                source,
                path,
                "type",
                "E_TYPE",
                "Condition must be boolean",
            ));
        }
    } else {
        for (group, items) in value.as_object().unwrap() {
            for (i, child) in items.as_array().unwrap().iter().enumerate() {
                check_condition(
                    child,
                    schema,
                    aggregate,
                    available,
                    source,
                    &format!("{path}/{group}/{i}"),
                )?;
            }
        }
    }
    Ok(())
}

fn expression_type(
    expr: &Expression,
    schema: &Schema,
    aggregate: bool,
    available: &BTreeSet<String>,
    source: &str,
    path: &str,
) -> CoreResult<FieldType> {
    let err = |code, message: &str| diagnostic(source, path, "type", code, message);
    match expr {
        Expression::Literal(Value::Bool(_)) => Ok(FieldType::Boolean),
        Expression::Literal(Value::String(_)) => Ok(FieldType::String),
        Expression::Literal(Value::Number(n)) if n.is_finite() => Ok(FieldType::Number),
        Expression::FieldAccess(fields) if aggregate && fields == &["total_score"] => {
            Ok(FieldType::Number)
        }
        Expression::FieldAccess(fields) if fields.len() == 2 && fields[0] == "event" => schema
            .fields
            .get(&fields[1])
            .map(|f| f.field_type.clone())
            .ok_or_else(|| err("E_INVALID_REF", "Undeclared event field")),
        Expression::ResultAccess {
            ruleset_id: Some(id),
            field,
        } if available.contains(id) => match field.as_str() {
            "score" | "total_score" => Ok(FieldType::Number),
            "signal" => Ok(FieldType::String),
            _ => Err(err("E_INVALID_REF", "Unsupported result field")),
        },
        Expression::FieldAccess(_) | Expression::ResultAccess { .. } => Err(err(
            "E_INVALID_REF",
            "Reference is out of scope or not completed on every incoming path",
        )),
        Expression::Unary { op, operand } => {
            let t = expression_type(operand, schema, aggregate, available, source, path)?;
            match (op, &t) {
                (UnaryOperator::Not, FieldType::Boolean)
                | (UnaryOperator::Negate, FieldType::Number) => Ok(t),
                _ => Err(err("E_TYPE", "Invalid unary operand")),
            }
        }
        Expression::Binary { left, op, right } => {
            let l = expression_type(left, schema, aggregate, available, source, path)?;
            let r = expression_type(right, schema, aggregate, available, source, path)?;
            let valid = match op {
                Operator::Eq | Operator::Ne => l == r,
                Operator::Gt | Operator::Ge | Operator::Lt | Operator::Le => {
                    l == FieldType::Number && r == l
                }
                Operator::And | Operator::Or => l == FieldType::Boolean && r == l,
                _ => {
                    return Err(err(
                        "E_UNSUPPORTED_CAPABILITY",
                        "Operator is outside this Core increment",
                    ))
                }
            };
            if valid {
                Ok(FieldType::Boolean)
            } else {
                Err(err(
                    "E_TYPE",
                    "Operand types do not match; no implicit coercion",
                ))
            }
        }
        _ => Err(err(
            "E_UNSUPPORTED_CAPABILITY",
            "Expression is outside this Core increment",
        )),
    }
}

// Normalize every accepted boolean spelling to the existing short-circuit AST.
// Legacy compiler entry points retain their prior behavior.
fn normalize_expression(expr: Expression) -> Expression {
    match expr {
        Expression::Binary { left, op, right } => {
            let left = normalize_expression(*left);
            let right = normalize_expression(*right);
            match op {
                Operator::And | Operator::Or => Expression::LogicalGroup {
                    op: if op == Operator::And {
                        LogicalGroupOp::All
                    } else {
                        LogicalGroupOp::Any
                    },
                    conditions: vec![left, right],
                },
                _ => Expression::binary(left, op, right),
            }
        }
        Expression::Unary { op, operand } => Expression::unary(op, normalize_expression(*operand)),
        Expression::LogicalGroup { op, conditions } => Expression::LogicalGroup {
            op,
            conditions: conditions.into_iter().map(normalize_expression).collect(),
        },
        other => other,
    }
}

fn group_expression(group: ConditionGroup) -> Expression {
    let (op, conditions, negate) = match group {
        ConditionGroup::All(items) => (LogicalGroupOp::All, items, false),
        ConditionGroup::Any(items) => (LogicalGroupOp::Any, items, false),
        ConditionGroup::Not(items) => (LogicalGroupOp::Any, items, true),
    };
    let expr = Expression::LogicalGroup {
        op,
        conditions: conditions
            .into_iter()
            .map(|c| match c {
                Condition::Expression(e) => normalize_expression(e),
                Condition::Group(g) => group_expression(*g),
            })
            .collect(),
    };
    if negate {
        Expression::unary(UnaryOperator::Not, expr)
    } else {
        expr
    }
}

fn normalize_when(when: &mut WhenBlock) {
    let expr = if let Some(group) = when.condition_group.take() {
        group_expression(group)
    } else {
        Expression::LogicalGroup {
            op: LogicalGroupOp::All,
            conditions: when
                .conditions
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(normalize_expression)
                .collect(),
        }
    };
    when.conditions = Some(vec![expr]);
}

fn validate_input_schema(schema: &Schema) -> CoreResult<()> {
    // Stable first-error order across processes, including CLI/Agent retries.
    for (name, field) in schema.fields.iter().collect::<BTreeMap<_, _>>() {
        if name != &field.name
            || name.is_empty()
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            || !field.required
            || field.default.is_some()
            || !matches!(
                field.field_type,
                FieldType::Boolean | FieldType::Number | FieldType::String
            )
        {
            return Err(diagnostic(
                "<input-schema>",
                &format!("/fields/{}", name.replace('~', "~0").replace('/', "~1")),
                "type",
                "E_UNSUPPORTED_CAPABILITY",
                "Increment 1 requires flat, required, non-null scalar fields without defaults",
            ));
        }
    }
    Ok(())
}

pub fn validate_core_input(schema: &Schema, event: &HashMap<String, Value>) -> CoreResult<()> {
    for (name, field) in schema.fields.iter().collect::<BTreeMap<_, _>>() {
        let valid = match (event.get(name), &field.field_type) {
            (Some(Value::Bool(_)), FieldType::Boolean)
            | (Some(Value::String(_)), FieldType::String) => true,
            (Some(Value::Number(n)), FieldType::Number) => n.is_finite(),
            _ => false,
        };
        if !valid {
            return Err(diagnostic(
                "<request>",
                &format!("/event/{name}"),
                "input",
                "E_INPUT_SCHEMA",
                "Missing or invalid required input",
            ));
        }
    }
    if let Some(name) = event
        .keys()
        .filter(|k| !schema.fields.contains_key(*k))
        .min()
    {
        return Err(diagnostic(
            "<request>",
            &format!("/event/{}", name.replace('~', "~0").replace('/', "~1")),
            "input",
            "E_INPUT_SCHEMA",
            "Undeclared event input",
        ));
    }
    Ok(())
}
