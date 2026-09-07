//! Bounded, iterative dependency validation shared by loading and execution.
use super::definition::FeatureDefinition;
use anyhow::{bail, Result};
use std::collections::HashMap;

pub(super) fn order(
    features: &HashMap<String, FeatureDefinition>,
    roots: &[String],
    allow_missing: bool,
) -> Result<Vec<String>> {
    walk(features, roots, allow_missing, false)
}

pub(super) fn execution_order(
    features: &HashMap<String, FeatureDefinition>,
    roots: &[String],
) -> Result<Vec<String>> {
    walk(features, roots, false, true)
}

fn walk(
    features: &HashMap<String, FeatureDefinition>,
    roots: &[String],
    allow_missing: bool,
    skip_disabled: bool,
) -> Result<Vec<String>> {
    if features.len() > 4096 {
        bail!("Feature graph exceeds 4096 nodes");
    }
    let mut state = HashMap::new();
    let mut result = Vec::new();
    let mut heights = HashMap::new();
    for root in roots {
        let mut stack = vec![(root.clone(), false, 1usize)];
        while let Some((name, exiting, depth)) = stack.pop() {
            if exiting {
                let height = 1 + features[&name]
                    .dependencies
                    .iter()
                    .filter_map(|dep| heights.get(dep))
                    .copied()
                    .max()
                    .unwrap_or(0usize);
                if height > 128 {
                    bail!("Feature dependency depth exceeds 128 at '{name}'");
                }
                heights.insert(name.clone(), height);
                state.insert(name.clone(), 2);
                result.push(name);
                continue;
            }
            match state.get(&name) {
                Some(2) => continue,
                Some(1) => bail!("Circular feature dependency: {name}"),
                _ => {}
            }
            if depth > 128 {
                bail!("Feature dependency depth exceeds 128 at '{name}'");
            }
            let Some(feature) = features.get(&name) else {
                if allow_missing {
                    continue;
                }
                bail!("Feature dependency '{name}' not found");
            };
            if feature.dependencies.len() > 4096 {
                bail!("Too many dependencies for '{name}'");
            }
            state.insert(name.clone(), 1);
            stack.push((name, true, depth));
            if skip_disabled && !feature.enabled {
                continue;
            }
            stack.extend(
                feature
                    .dependencies
                    .iter()
                    .rev()
                    .map(|dep| (dep.clone(), false, depth + 1)),
            );
        }
    }
    Ok(result)
}

pub(super) fn infer_dependencies(feature: &mut FeatureDefinition) -> Result<()> {
    if let Some(expression) = feature
        .expression
        .as_ref()
        .and_then(|c| c.expression.as_deref())
    {
        feature
            .dependencies
            .extend(super::expression::ExpressionEvaluator::extract_dependencies(expression)?);
        feature.dependencies.sort();
        feature.dependencies.dedup();
    }
    Ok(())
}
