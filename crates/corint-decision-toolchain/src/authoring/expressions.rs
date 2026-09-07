use super::{shape_errors, sources, Reference, Report, INPUT_SCHEMA};
use corint_decision_compiler::semantic::{TypeChecker, TypeInfo};
use corint_decision_dsl_parser::ExpressionParser;
use corint_decision_model::{
    ast::Expression,
    types::{FieldType, Schema},
};
use corint_decision_runtime::service::HttpServiceClient;
use serde_json::Value;
use std::path::Path;

pub(super) struct Checker {
    types: TypeChecker,
    schema: Option<Schema>,
}
impl Checker {
    pub fn new(path: Option<&Path>, report: &mut Report) -> Self {
        let mut checker = Self {
            types: TypeChecker::new(),
            schema: None,
        };
        if let Some(path) = path {
            if let Some(text) = sources::read(path, report) {
                let source = path.to_string_lossy();
                if let Some(value) = sources::yaml(&text, &source, report) {
                    let compiled = jsonschema::JSONSchema::compile(
                        &serde_json::from_str(INPUT_SCHEMA).unwrap(),
                    )
                    .unwrap();
                    if compiled.is_valid(&value) {
                        match serde_json::from_value::<Schema>(value) {
                            Ok(schema) => {
                                checker.register(&schema, "event", &source, report);
                                checker.schema = Some(schema);
                                report.input_schema_checked = true;
                            }
                            Err(error) => report.error(
                                &source,
                                "",
                                "schema",
                                "E_INPUT_SCHEMA",
                                error.to_string(),
                            ),
                        }
                    } else {
                        shape_errors(&compiled, &value, &source, "", report);
                    }
                }
            }
        }
        checker
    }
    fn register(&mut self, schema: &Schema, prefix: &str, source: &str, report: &mut Report) {
        // BTree order makes input diagnostics reproducible.
        let mut fields: Vec<_> = schema.fields.iter().collect();
        fields.sort_by_key(|(name, _)| *name);
        for (name, field) in fields {
            if name.is_empty() || name != &field.name || name.contains('.') {
                report.error(
                    source,
                    "/fields",
                    "schema",
                    "E_INPUT_SCHEMA",
                    format!("Field name must match its nonempty mapping key without dots: {name}"),
                );
            }
            let path = format!("{prefix}.{name}");
            self.types
                .register_field(path.clone(), type_info(&field.field_type));
            if let FieldType::Object {
                schema: Some(nested),
            } = &field.field_type
            {
                self.register(nested, &path, source, report);
            }
        }
    }
    pub fn expression(
        &self,
        text: &str,
        source: &str,
        path: &str,
        boolean: bool,
        refs: &mut Vec<Reference>,
        report: &mut Report,
    ) {
        match ExpressionParser::parse(text) {
            Err(error) => report.error(
                source,
                path,
                "expression",
                "E_INVALID_EXPRESSION",
                error.to_string(),
            ),
            Ok(ast) => {
                match self.types.check_expression(&ast) {
                    Ok(kind) if boolean && !kind.is_boolean() => report.error(
                        source,
                        path,
                        "type",
                        "E_TYPE_MISMATCH",
                        "Condition must produce a boolean",
                    ),
                    Err(error) => {
                        report.error(source, path, "type", "E_TYPE_MISMATCH", error.to_string())
                    }
                    _ => (),
                }
                let mut pending = vec![&ast];
                while let Some(node) = pending.pop() {
                    match node {
                        Expression::FieldAccess(parts) => {
                            if parts.first().is_some_and(|p| p == "event")
                                && self
                                    .schema
                                    .as_ref()
                                    .is_some_and(|schema| !has_field(schema, &parts[1..]))
                            {
                                report.error(
                                    source,
                                    path,
                                    "type",
                                    "E_UNKNOWN_FIELD",
                                    format!("Undeclared event field: {}", parts.join(".")),
                                );
                            }
                            if parts.first().is_some_and(|p| p == "features") && parts.len() > 1 {
                                refs.push(Reference::new(source, path, "feature", &parts[1]));
                            }
                        }
                        Expression::ListReference { list_id } => {
                            refs.push(Reference::new(source, path, "list", list_id))
                        }
                        Expression::Binary { left, right, .. } => {
                            pending.push(left);
                            pending.push(right);
                        }
                        Expression::Unary { operand, .. } => pending.push(operand),
                        Expression::FunctionCall { args, .. } => pending.extend(args),
                        Expression::Ternary {
                            condition,
                            true_expr,
                            false_expr,
                        } => {
                            pending.push(condition);
                            pending.push(true_expr);
                            pending.push(false_expr);
                        }
                        Expression::LogicalGroup { conditions, .. } => pending.extend(conditions),
                        _ => (),
                    }
                }
            }
        }
    }
    fn when(
        &self,
        value: &Value,
        source: &str,
        path: &str,
        refs: &mut Vec<Reference>,
        report: &mut Report,
    ) {
        match value {
            Value::String(text) => self.expression(text, source, path, true, refs, report),
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    self.when(item, source, &format!("{path}/{i}"), refs, report);
                }
            }
            Value::Object(fields) => {
                for (key, item) in fields {
                    if ["all", "any", "not", "conditions"].contains(&key.as_str()) {
                        self.when(item, source, &format!("{path}/{key}"), refs, report);
                    } else if ["event", "event.type", "event_type"].contains(&key.as_str())
                        && self
                            .schema
                            .as_ref()
                            .is_some_and(|schema| !has_field(schema, &["type".into()]))
                    {
                        report.error(
                            source,
                            path,
                            "type",
                            "E_UNKNOWN_FIELD",
                            "Undeclared event field: event.type",
                        );
                    }
                }
            }
            _ => (), // Shape and shared CDL parsers reject invalid condition nodes.
        }
    }
    pub fn conditions(
        &self,
        value: &Value,
        source: &str,
        path: &str,
        refs: &mut Vec<Reference>,
        report: &mut Report,
    ) {
        match value {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    self.conditions(item, source, &format!("{path}/{i}"), refs, report);
                }
            }
            Value::Object(fields) => {
                if let Some(condition) = fields.get("when") {
                    self.when(condition, source, &format!("{path}/when"), refs, report);
                }
                for key in [
                    "rule",
                    "ruleset",
                    "pipeline",
                    "steps",
                    "step",
                    "routes",
                    "conclusion",
                    "decision",
                    "registry",
                ] {
                    if let Some(item) = fields.get(key) {
                        self.conditions(item, source, &format!("{path}/{key}"), refs, report);
                    }
                }
                if fields.get("type").and_then(Value::as_str) == Some("service") {
                    if let Some(params) = fields.get("params").and_then(Value::as_object) {
                        for (name, param) in params {
                            if let Some(text) = param.as_str() {
                                let param_path = format!("{path}/params/{}", pointer(name));
                                if text.starts_with("${") {
                                    if let Some(expr) =
                                        text.strip_prefix("${").and_then(|s| s.strip_suffix('}'))
                                    {
                                        self.expression(
                                            expr,
                                            source,
                                            &param_path,
                                            false,
                                            refs,
                                            report,
                                        );
                                    } else {
                                        report.error(
                                            source,
                                            &param_path,
                                            "expression",
                                            "E_INVALID_EXPRESSION",
                                            "Unclosed ${expression} parameter",
                                        );
                                    }
                                } else if context_path(text) {
                                    self.expression(text, source, &param_path, false, refs, report);
                                }
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
    pub fn templates(
        &self,
        value: &Value,
        source: &str,
        path: &str,
        refs: &mut Vec<Reference>,
        report: &mut Report,
    ) {
        // Feature templates are supported only in these values, not descriptions or labels.
        for key in ["dimension_value", "key", "when"] {
            if let Some(value) = value.get(key) {
                self.template_values(value, source, &format!("{path}/{key}"), refs, report);
            }
        }
    }
    fn template_values(
        &self,
        value: &Value,
        source: &str,
        path: &str,
        refs: &mut Vec<Reference>,
        report: &mut Report,
    ) {
        match value {
            Value::String(text) => {
                let mut rest = text.as_str();
                while let Some((_, tail)) = rest.split_once("${") {
                    let Some((field, next)) = tail.split_once('}') else {
                        report.error(
                            source,
                            path,
                            "expression",
                            "E_INVALID_TEMPLATE",
                            "Unclosed Feature template",
                        );
                        break;
                    };
                    match ExpressionParser::parse(field) {
                        Ok(Expression::FieldAccess(parts))
                            if parts.len() > 1 && parts[0] == "event" =>
                        {
                            self.expression(field, source, path, false, refs, report)
                        }
                        _ => report.error(
                            source,
                            path,
                            "expression",
                            "E_INVALID_TEMPLATE",
                            "Feature templates require ${event.field} paths",
                        ),
                    }
                    rest = next;
                }
            }
            Value::Object(fields) => {
                for (key, value) in fields {
                    self.template_values(
                        value,
                        source,
                        &format!("{path}/{}", pointer(key)),
                        refs,
                        report,
                    );
                }
            }
            Value::Array(items) => {
                for (i, value) in items.iter().enumerate() {
                    self.template_values(value, source, &format!("{path}/{i}"), refs, report);
                }
            }
            _ => (),
        }
    }
    pub fn service(
        &self,
        value: &Value,
        source: &str,
        refs: &mut Vec<Reference>,
        report: &mut Report,
    ) {
        for (name, operation) in value["operations"].as_object().unwrap() {
            let path = format!("/operations/{}", pointer(name));
            if let Some(body) = operation["request_body"].as_str() {
                if let Err(error) = HttpServiceClient::validate_body_template(body) {
                    report.error(
                        source,
                        &format!("{path}/request_body"),
                        "semantic",
                        "E_INVALID_TEMPLATE",
                        error.to_string(),
                    );
                }
            }
            let raw_path = operation["path"].as_str().unwrap();
            let mut rest = raw_path;
            let mut valid = true;
            while let Some((head, tail)) = rest.split_once('{') {
                let Some((name, next)) = tail.split_once('}') else {
                    valid = false;
                    break;
                };
                if head.contains('}') || name.trim().is_empty() || name.contains('{') {
                    valid = false;
                    break;
                }
                rest = next;
            }
            if !valid || rest.contains('}') {
                report.error(
                    source,
                    &format!("{path}/path"),
                    "semantic",
                    "E_INVALID_TEMPLATE",
                    "Malformed {parameter} in service path",
                );
            }
            if let Some(params) = operation["params"].as_object() {
                for (key, value) in params {
                    if let Some(text) = value.as_str().filter(|s| context_path(s)) {
                        if !matches!(
                            ExpressionParser::parse(text),
                            Ok(Expression::FieldAccess(_))
                        ) {
                            report.error(
                                source,
                                &format!("{path}/params/{}", pointer(key)),
                                "expression",
                                "E_INVALID_EXPRESSION",
                                "Operation defaults accept a context path, not an expression",
                            );
                        } else {
                            self.expression(
                                text,
                                source,
                                &format!("{path}/params/{}", pointer(key)),
                                false,
                                refs,
                                report,
                            );
                        }
                    }
                }
            }
        }
    }
}
fn context_path(text: &str) -> bool {
    ["event.", "features.", "service.", "vars.", "sys.", "env."]
        .iter()
        .any(|p| text.starts_with(p))
}
fn pointer(text: &str) -> String {
    text.replace('~', "~0").replace('/', "~1")
}
fn type_info(kind: &FieldType) -> TypeInfo {
    match kind {
        FieldType::Number => TypeInfo::Number,
        FieldType::Boolean => TypeInfo::Boolean,
        FieldType::String => TypeInfo::String,
        FieldType::Array { item_type } => TypeInfo::Array(Box::new(type_info(item_type))),
        FieldType::Object { .. } => TypeInfo::Object,
        _ => TypeInfo::Any,
    }
}
fn has_field(schema: &Schema, parts: &[String]) -> bool {
    if parts.is_empty() {
        return true;
    }
    let Some(field) = schema.fields.get(&parts[0]) else {
        return false;
    };
    if parts.len() == 1 {
        return true;
    }
    match &field.field_type {
        FieldType::Any | FieldType::Object { schema: None } => true,
        FieldType::Object {
            schema: Some(schema),
        } => has_field(schema, &parts[1..]),
        _ => false,
    }
}
