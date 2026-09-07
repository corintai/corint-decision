use corint_decision_compiler::core::{
    compile_core, parse_core_input_schema, validate_core_document, CoreSource,
};
use corint_decision_dsl_parser::{PipelineParser, RuleParser, RulesetParser};
use serde_json::Value;
use std::path::PathBuf;

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

#[test]
fn historical_snippets_run_declared_admission_and_wrappers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inventory: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("tests/conformance/documentation/snippets.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(inventory["version"], 2);
    assert_eq!(inventory["path_base"], "repository");
    let mut checked = 0;
    for snippet in inventory["snippets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] != "syntax-reference")
    {
        let page = snippet["page"].as_str().unwrap();
        let text = std::fs::read_to_string(root.join(page)).unwrap();
        let fragment = CoreSource {
            path: format!("{page}#{}", snippet["id"].as_str().unwrap()),
            yaml: blocks(&text)[snippet["block"].as_u64().unwrap() as usize - 1].clone(),
        };
        if snippet["gate"] == "metadata_parse_and_core_rejection" {
            let kind = snippet["resource"].as_str().unwrap();
            let mut document: Value = serde_yaml::from_str(&fragment.yaml).unwrap();
            let parsed_metadata = match kind {
                "rule" => RuleParser::parse(&fragment.yaml).unwrap().metadata.unwrap(),
                "ruleset" => RulesetParser::parse(&fragment.yaml)
                    .unwrap()
                    .metadata
                    .unwrap(),
                "pipeline" => serde_json::to_value(
                    PipelineParser::parse(&fragment.yaml)
                        .unwrap()
                        .metadata
                        .unwrap(),
                )
                .unwrap(),
                _ => panic!("Unknown metadata example resource: {kind}"),
            };
            assert!(document[kind]["metadata"].is_object());
            assert_eq!(parsed_metadata, document[kind]["metadata"]);

            let error = validate_core_document(&fragment).unwrap_err();
            let expected = &snippet["rejection"];
            assert_eq!(error.diagnostic.code, expected["code"].as_str().unwrap());
            assert_eq!(
                error.diagnostic.stage.as_deref(),
                Some(expected["stage"].as_str().unwrap())
            );
            assert_eq!(
                error.diagnostic.field_path.as_deref(),
                Some(expected["field_path"].as_str().unwrap())
            );

            // The rejection must be caused only by metadata, not incomplete
            // resource syntax or a missing reference in the example.
            document[kind].as_object_mut().unwrap().remove("metadata");
            let base = root.join("tests/conformance/cdl_core");
            let read = |name: &str| CoreSource {
                path: name.into(),
                yaml: std::fs::read_to_string(base.join(name)).unwrap(),
            };
            let mut sources: Vec<_> = [
                "rule.yaml",
                "ruleset.yaml",
                "pipeline.yaml",
                "registry.yaml",
            ]
            .into_iter()
            .map(read)
            .collect();
            let index = sources
                .iter()
                .position(|s| s.path == format!("{kind}.yaml"))
                .unwrap();
            let control: Value = serde_yaml::from_str(&sources[index].yaml).unwrap();
            assert_eq!(
                document, control,
                "Example logic must match its Core fixture"
            );
            sources[index] = source(&fragment.path, document);
            let input = parse_core_input_schema(&read("input-schema.yaml")).unwrap();
            compile_core(&sources, input).unwrap();
        } else {
            assert!(
                validate_core_document(&fragment).is_err(),
                "{} unexpectedly admitted; classify and bind a complete execution wrapper",
                fragment.path
            );
        }
        checked += 1;
    }
    // Document removals change the corpus size; check_docs.py verifies complete bindings.
    assert!(
        checked > 0,
        "Snippet inventory must include admission checks"
    );
}
