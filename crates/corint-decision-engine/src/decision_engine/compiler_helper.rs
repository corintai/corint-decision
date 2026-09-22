//! Rule compilation and loading utilities

use crate::error::{EngineError, Result};
use corint_decision_compiler::Compiler;
use corint_decision_dsl_parser::{PipelineParser, RegistryParser, RuleParser, RulesetParser};
use corint_decision_model::ir::Program;
use std::path::Path;

pub(super) struct CompilerHelper;

impl CompilerHelper {
    pub(super) async fn load_and_compile_rules(
        path: &Path,
        compiler: &mut Compiler,
    ) -> Result<Vec<Program>> {
        use corint_decision_dsl_parser::YamlParser;

        // Read file
        let content = tokio::fs::read_to_string(path).await?;

        tracing::debug!("Loading file: {}", path.display());

        let mut programs = Vec::new();
        let mut has_pipeline = false;
        let mut pipeline_count = 0;

        // Parse multi-document YAML (supports files with --- separators)
        let documents = YamlParser::parse_multi_document(&content)?;

        // Try to parse each document
        for doc in documents.iter() {
            // Try rule first
            if doc.get("rule").is_some() {
                let rule = RuleParser::parse_from_yaml(doc)?;
                let prog = compiler.compile_rule(&rule)?;
                programs.push(prog);
                continue;
            }

            // Try ruleset
            if doc.get("ruleset").is_some() {
                let ruleset = RulesetParser::parse_from_yaml(doc)?;
                let prog = compiler.compile_ruleset(&ruleset)?;
                programs.push(prog);
                continue;
            }

            // Try pipeline
            if doc.get("pipeline").is_some() {
                let pipeline = PipelineParser::parse_from_yaml(doc)?;
                has_pipeline = true;
                pipeline_count += 1;

                // Validate: Pipeline must have when condition
                if pipeline.when.is_none() {
                    return Err(EngineError::InvalidRuleFile(format!(
                        "Pipeline '{}' in file '{}' is missing mandatory 'when' condition. \
                     All pipelines must specify when conditions to filter events.",
                        pipeline.id,
                        path.display()
                    )));
                }

                tracing::debug!(
                    "Parsed pipeline: when={:?}, steps={}",
                    pipeline.when,
                    pipeline.steps.len()
                );
                let prog = compiler.compile_pipeline(&pipeline)?;
                tracing::debug!(
                    "Compiled pipeline: {} instructions",
                    prog.instructions.len()
                );
                programs.push(prog);
                continue;
            }

            // Skip documents that don't match any known type (e.g., metadata sections)
        }

        // If no valid documents were found, return error
        if programs.is_empty() {
            return Err(EngineError::InvalidRuleFile(format!(
                "File does not contain a valid rule, ruleset, or pipeline: {}",
                path.display()
            )));
        }

        // Validate: File must contain at least one pipeline
        if !has_pipeline {
            return Err(EngineError::InvalidRuleFile(format!(
                "Rule file '{}' must contain at least one pipeline definition. \
             Pipelines are the entry points for rule execution and must have 'when' conditions. \
             Rules and rulesets cannot be used as top-level entry points.",
                path.display()
            )));
        }

        tracing::info!(
            "✓ Loaded file '{}': {} pipeline(s), {} total definitions",
            path.display(),
            pipeline_count,
            programs.len()
        );

        Ok(programs)
    }

    /// Compile rules from content string (from repository)
    pub(super) async fn compile_rules_from_content(
        id: &str,
        content: &str,
        compiler: &mut Compiler,
    ) -> Result<Vec<Program>> {
        use corint_decision_dsl_parser::YamlParser;

        tracing::debug!("Compiling content from: {}", id);

        let mut programs = Vec::new();
        let mut has_pipeline = false;
        let mut pipeline_count = 0;

        // First, try to parse as a pipeline with imports (most common case for repository content)
        if let Ok(document) =
            corint_decision_dsl_parser::PipelineParser::parse_with_imports(content)
        {
            has_pipeline = true;
            pipeline_count += 1;

            tracing::debug!(
                "Parsed pipeline with imports: when={:?}, steps={}, imports={:?}",
                document.definition.when,
                document.definition.steps.len(),
                document.imports.is_some()
            );

            // Resolve imports and compile dependencies
            let resolved = compiler
                .import_resolver_mut()
                .resolve_imports(&document)
                .map_err(EngineError::CompileError)?;

            tracing::debug!(
                "Resolved imports: {} rules, {} rulesets",
                resolved.rules.len(),
                resolved.rulesets.len()
            );

            // Compile all resolved rules first
            for rule in &resolved.rules {
                let rule_prog = compiler.compile_rule(rule)?;
                programs.push(rule_prog);
            }

            // Compile all resolved rulesets
            for ruleset in &resolved.rulesets {
                let ruleset_prog = compiler.compile_ruleset(ruleset)?;
                programs.push(ruleset_prog);
            }

            // Finally compile the pipeline itself
            let prog = compiler.compile_pipeline(&document.definition)?;
            tracing::debug!(
                "Compiled pipeline: {} instructions",
                prog.instructions.len()
            );
            programs.push(prog);

            // IMPORTANT: Also parse inline rules and rulesets from the same YAML file
            // This supports the format where pipeline, rules, and rulesets are in the same file
            let documents = YamlParser::parse_multi_document(content)?;
            if documents.len() > 1 {
                tracing::debug!(
                    "Found {} documents in file, checking for inline rules/rulesets",
                    documents.len()
                );

                use corint_decision_dsl_parser::{RuleParser, RulesetParser};

                for doc in &documents {
                    // Skip if it's the pipeline definition (already compiled above)
                    if doc.get("pipeline").is_some() {
                        continue;
                    }

                    // Try to parse as rule
                    if doc.get("rule").is_some() {
                        let rule = RuleParser::parse_from_yaml(doc)?;
                        tracing::debug!("Found inline rule: {}", rule.id);
                        let rule_prog = compiler.compile_rule(&rule)?;
                        programs.push(rule_prog);
                        continue;
                    }

                    // Try to parse as ruleset
                    if doc.get("ruleset").is_some() {
                        let ruleset = RulesetParser::parse_from_yaml(doc)?;
                        tracing::debug!("Found inline ruleset: {}", ruleset.id);
                        let ruleset_prog = compiler.compile_ruleset(&ruleset)?;
                        programs.push(ruleset_prog);
                        continue;
                    }
                }
            }
        } else {
            // Fallback: Parse as multi-document YAML for individual rules/rulesets
            let documents = YamlParser::parse_multi_document(content)?;

            // Try to parse each document
            for doc in documents.iter() {
                // Try rule first
                if doc.get("rule").is_some() {
                    let rule = RuleParser::parse_from_yaml(doc)?;
                    let prog = compiler.compile_rule(&rule)?;
                    programs.push(prog);
                    continue;
                }

                // Try ruleset
                if doc.get("ruleset").is_some() {
                    let ruleset = RulesetParser::parse_from_yaml(doc)?;
                    let prog = compiler.compile_ruleset(&ruleset)?;
                    programs.push(prog);
                    continue;
                }

                // Try pipeline (without imports, since parse_with_imports already failed above)
                if doc.get("pipeline").is_some() {
                    let pipeline = PipelineParser::parse_from_yaml(doc)?;
                    has_pipeline = true;
                    pipeline_count += 1;

                    // Validate: Pipeline must have when condition
                    if pipeline.when.is_none() {
                        return Err(EngineError::InvalidRuleFile(format!(
                            "Pipeline '{}' from '{}' is missing mandatory 'when' condition. \
                     All pipelines must specify when conditions to filter events.",
                            pipeline.id, id
                        )));
                    }

                    let prog = compiler.compile_pipeline(&pipeline)?;
                    tracing::debug!(
                        "Compiled pipeline (no imports): {} instructions",
                        prog.instructions.len()
                    );
                    programs.push(prog);
                    continue;
                }

                // Skip documents that don't match any known type (e.g., metadata sections)
            }
        } // Close the else block

        // If no valid documents were found, return error
        if programs.is_empty() {
            return Err(EngineError::InvalidRuleFile(format!(
                "Content from '{}' does not contain a valid rule, ruleset, or pipeline",
                id
            )));
        }

        // Validate: Content must contain at least one pipeline
        if !has_pipeline {
            return Err(EngineError::InvalidRuleFile(format!(
                "Content from '{}' must contain at least one pipeline definition. \
             Pipelines are the entry points for rule execution and must have 'when' conditions. \
             Rules and rulesets cannot be used as top-level entry points.",
                id
            )));
        }

        tracing::info!(
            "✓ Loaded content '{}': {} pipeline(s), {} total definitions",
            id,
            pipeline_count,
            programs.len()
        );

        Ok(programs)
    }

    /// Load registry from file
    pub(super) async fn load_registry(
        path: &Path,
    ) -> Result<corint_decision_model::ast::PipelineRegistry> {
        let content = tokio::fs::read_to_string(path).await?;
        let registry = RegistryParser::parse(&content)?;
        Ok(registry)
    }
}

#[cfg(test)]
mod resource_list_tests {
    use super::*;

    const RULES: &str = "rule:\n  - {id: first, name: First, when: 'true', score: 10}\n  - {id: second, name: Second, when: 'true', score: 20}\n";
    const RULESETS: &str = "ruleset:\n  - id: one\n    rules:\n      - first\n    conclusion:\n      - default: true\n        signal: pass\n  - id: two\n    rules:\n      - second\n    conclusion:\n      - default: true\n        signal: pass\n";
    const PIPELINE: &str = "pipeline:\n  id: payment\n  name: Payment\n  when: 'true'\n  entry: check\n  steps:\n    - step: {id: check, name: Check, type: ruleset, ruleset: two, next: end}\n  decision:\n    - default: true\n      result: approve\n";

    fn assert_programs(programs: &[Program]) {
        let mut ids: Vec<_> = programs
            .iter()
            .map(|p| p.metadata.source_id.as_str())
            .collect();
        ids.sort();
        assert_eq!(ids, ["first", "one", "payment", "second", "two"]);
    }

    #[tokio::test]
    async fn resource_lists_load_inline_and_through_imports() {
        let dir = tempfile::tempdir().unwrap();
        // No header and rules before pipeline: the first rule must not be skipped.
        let source = format!("{RULES}{RULESETS}{PIPELINE}");
        let programs =
            CompilerHelper::compile_rules_from_content("inline", &source, &mut Compiler::new())
                .await
                .unwrap();
        assert_programs(&programs);
        let path = dir.path().join("policy.yaml");
        std::fs::write(&path, &source).unwrap();
        assert_programs(
            &CompilerHelper::load_and_compile_rules(&path, &mut Compiler::new())
                .await
                .unwrap(),
        );

        std::fs::write(dir.path().join("rules.yaml"), RULES).unwrap();
        std::fs::write(dir.path().join("rulesets.yaml"), RULESETS).unwrap();
        let imported =
            format!("import:\n  rules: [rules.yaml]\n  rulesets: [rulesets.yaml]\n{PIPELINE}");
        let mut compiler = Compiler::with_options(corint_decision_compiler::CompilerOptions {
            library_base_path: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        });
        let programs =
            CompilerHelper::compile_rules_from_content("imported", &imported, &mut compiler)
                .await
                .unwrap();
        assert_programs(&programs);

        // Rulesets imported alone can carry both local rules and resource lists.
        std::fs::write(dir.path().join("bundle.yaml"), format!("{RULES}{RULESETS}")).unwrap();
        let bundled = format!("import:\n  rulesets: [bundle.yaml]\n{PIPELINE}");
        let mut compiler = Compiler::with_options(corint_decision_compiler::CompilerOptions {
            library_base_path: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        });
        assert_programs(
            &CompilerHelper::compile_rules_from_content("bundle", &bundled, &mut compiler)
                .await
                .unwrap(),
        );
        let invalid = source.replace("score: 20", "score: bad");
        assert!(CompilerHelper::compile_rules_from_content(
            "invalid",
            &invalid,
            &mut Compiler::new()
        )
        .await
        .is_err());

        // Duplicate IDs inside one list must fail before import deduplication.
        std::fs::write(
            dir.path().join("rules.yaml"),
            RULES.replace("id: second", "id: first"),
        )
        .unwrap();
        let mut compiler = Compiler::with_options(corint_decision_compiler::CompilerOptions {
            library_base_path: dir.path().to_string_lossy().into_owned(),
            ..Default::default()
        });
        assert!(
            CompilerHelper::compile_rules_from_content("duplicate", &imported, &mut compiler)
                .await
                .is_err()
        );
    }
}
