use corint_decision_compiler::core::{
    compile_core, parse_core_input_schema, validate_core_document, CoreSource,
};
use corint_decision_engine::{DecisionEngine, DecisionRequest};
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf};

fn blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut open = false;
    let mut code = String::new();
    for line in text.lines() {
        if line.starts_with("```") {
            if open {
                blocks.push(std::mem::take(&mut code));
            }
            open = !open;
        } else if open {
            code.push_str(line);
            code.push('\n');
        }
    }
    blocks
}
fn source(name: &str, value: Value) -> CoreSource {
    CoreSource {
        path: name.into(),
        yaml: serde_json::to_string(&value).unwrap(),
    }
}

#[tokio::test]
async fn historical_snippets_run_declared_admission_and_wrappers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let docs = root.join("docs/cdl");
    let inventory: Value =
        serde_json::from_str(&std::fs::read_to_string(docs.join("snippets.json")).unwrap())
            .unwrap();
    let mut checked = 0;
    for snippet in inventory["snippets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] != "syntax-reference")
    {
        let page = snippet["page"].as_str().unwrap();
        let text = std::fs::read_to_string(docs.join(page)).unwrap();
        let fragment = CoreSource {
            path: format!("{page}#{}", snippet["id"].as_str().unwrap()),
            yaml: blocks(&text)[snippet["block"].as_u64().unwrap() as usize - 1].clone(),
        };
        if snippet["kind"] == "core-fragment" {
            assert_eq!(snippet["wrapper"], "registry_first_match");
            let registry: Value = serde_yaml::from_str(&fragment.yaml).unwrap();
            let mut sources = vec![fragment];
            for entry in registry["registry"].as_array().unwrap() {
                let id = entry["pipeline"].as_str().unwrap();
                sources.push(source(&format!("{id}.yaml"),json!({"version":"0.1","pipeline":{"id":id,"name":id,"entry":"r","steps":[{"step":{"id":"r","name":"r","type":"router","routes":[{"when":"true","next":"end"}],"default":"end"}}],"decision":[{"default":true,"result":"review"}]}})));
            }
            let input = source(
                "input.json",
                json!({"name":"registry","fields":{"type":{"name":"type","field_type":"string","required":true},"shadow":{"name":"shadow","field_type":"boolean","required":true},"geo":{"name":"geo","required":true,"field_type":{"object":{"schema":{"name":"geo","fields":{"country":{"name":"country","field_type":"string","required":true}}}}}}}}),
            );
            let engine =
                DecisionEngine::from_core(&sources, parse_core_input_schema(&input).unwrap())
                    .unwrap();
            for (kind, country, expected) in [
                ("login", "US", "login_pipeline"),
                ("payment", "BR", "payment_br_pipeline"),
                ("payment", "US", "payment_main_pipeline"),
                ("loan_application", "US", "loan_pipeline"),
            ] {
                let event: HashMap<_, _> = serde_json::from_value(
                    json!({"type":kind,"geo":{"country":country},"shadow":true}),
                )
                .unwrap();
                for trace in [false, true] {
                    let r = DecisionRequest::new(event.clone());
                    let result = engine
                        .decide(if trace { r.with_trace() } else { r })
                        .await
                        .unwrap();
                    assert_eq!(result.pipeline_id.as_deref(), Some(expected));
                }
            }
        } else if snippet["gate"] == "compile_with_core_wrapper" {
            validate_core_document(&fragment).unwrap();
            let rule: Value = serde_yaml::from_str(&fragment.yaml).unwrap();
            let base = root.join("tests/conformance/cdl_core");
            let read = |name: &str| CoreSource {
                path: name.into(),
                yaml: std::fs::read_to_string(base.join(name)).unwrap(),
            };
            let mut ruleset: Value = serde_yaml::from_str(&read("ruleset.yaml").yaml).unwrap();
            ruleset["ruleset"]["rules"] = json!([rule["rule"]["id"]]);
            let sources = vec![
                fragment,
                source("ruleset.yaml", ruleset),
                read("pipeline.yaml"),
                read("registry.yaml"),
            ];
            assert!(
                compile_core(
                    &sources,
                    parse_core_input_schema(&read("input-schema.yaml")).unwrap()
                )
                .is_err(),
                "{snippet}"
            );
        } else {
            assert!(
                validate_core_document(&fragment).is_err(),
                "{} unexpectedly admitted; classify and bind a complete execution wrapper",
                fragment.path
            );
        }
        checked += 1;
    }
    assert!(checked > 100);
}
