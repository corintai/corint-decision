//! Import resolver for loading and merging library components
//!
//! This module implements compile-time dependency resolution for the CORINT import system.
//! It loads rules, rulesets, and pipelines from the file system, resolves dependencies
//! transitively, and validates ID uniqueness.

use crate::error::{CompileError, Result};
use corint_core::ast::{RdlDocument, Rule, Ruleset};
use corint_parser::{RuleParser, RulesetParser};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Import resolver for loading and merging library components
pub struct ImportResolver {
    /// Base path for library files (e.g., "repository" or "examples")
    library_base_path: PathBuf,

    /// Cached loaded rules (path -> (Rule, source_path))
    rule_cache: HashMap<String, (Rule, String)>,

    /// Cached loaded rulesets (path -> (Ruleset, source_path))
    ruleset_cache: HashMap<String, (Ruleset, String)>,

    /// Track loading stack to detect circular dependencies
    loading_stack: Vec<String>,
}

impl ImportResolver {
    /// Create a new import resolver
    pub fn new(library_base_path: impl Into<PathBuf>) -> Self {
        Self {
            library_base_path: library_base_path.into(),
            rule_cache: HashMap::new(),
            ruleset_cache: HashMap::new(),
            loading_stack: Vec::new(),
        }
    }

    /// Resolve all imports in a document (with dependency propagation)
    ///
    /// This is the main entry point for resolving imports. It:
    /// 1. Loads all directly imported rules and rulesets
    /// 2. Recursively loads dependencies (e.g., rules imported by rulesets)
    /// 3. Deduplicates rules and rulesets
    /// 4. Validates ID uniqueness
    pub fn resolve_imports<T>(&mut self, document: &RdlDocument<T>) -> Result<ResolvedDocument> {
        let mut resolved_rules = Vec::new();
        let mut resolved_rulesets = Vec::new();

        // 1. Load imported rules and rulesets
        if let Some(imports) = &document.imports {
            // Load imported rules
            for rule_path in &imports.rules {
                let (rule, _) = self.load_rule(rule_path)?;
                resolved_rules.push(rule);
            }

            // Load imported rulesets (with their dependencies)
            for ruleset_path in &imports.rulesets {
                let (ruleset, deps) = self.load_ruleset_with_deps(ruleset_path)?;

                // Add ruleset's dependent rules
                resolved_rules.extend(deps.rules);

                // Add the ruleset itself
                resolved_rulesets.push(ruleset);
            }
        }

        // 2. Deduplicate (same rule/ruleset may be referenced multiple times)
        resolved_rules = self.deduplicate_rules(resolved_rules)?;
        resolved_rulesets = self.deduplicate_rulesets(resolved_rulesets)?;

        // 3. Validate ID uniqueness
        self.validate_unique_ids(&resolved_rules, &resolved_rulesets)?;

        Ok(ResolvedDocument {
            rules: resolved_rules,
            rulesets: resolved_rulesets,
        })
    }

    /// Resolve imports for a ruleset document and include the main ruleset
    ///
    /// This is a specialized version that also processes the main ruleset definition,
    /// loading its dependencies.
    pub fn resolve_ruleset_imports(
        &mut self,
        document: &RdlDocument<Ruleset>,
    ) -> Result<ResolvedDocument> {
        let mut resolved_rules = Vec::new();
        let mut resolved_rulesets = Vec::new();

        // 1. Load imported rules and rulesets
        if let Some(imports) = &document.imports {
            // Load imported rules
            for rule_path in &imports.rules {
                let (rule, _) = self.load_rule(rule_path)?;
                resolved_rules.push(rule);
            }

            // Load imported rulesets (with their dependencies)
            for ruleset_path in &imports.rulesets {
                let (ruleset, deps) = self.load_ruleset_with_deps(ruleset_path)?;

                // Add ruleset's dependent rules
                resolved_rules.extend(deps.rules);

                // Add the ruleset itself
                resolved_rulesets.push(ruleset);
            }
        }

        // 2. Process the main ruleset definition
        let main_ruleset = document.definition.clone();

        // Add the main ruleset
        resolved_rulesets.push(main_ruleset);

        // 3. Deduplicate (same rule/ruleset may be referenced multiple times)
        resolved_rules = self.deduplicate_rules(resolved_rules)?;
        resolved_rulesets = self.deduplicate_rulesets(resolved_rulesets)?;

        // 4. Validate ID uniqueness
        self.validate_unique_ids(&resolved_rules, &resolved_rulesets)?;

        Ok(ResolvedDocument {
            rules: resolved_rules,
            rulesets: resolved_rulesets,
        })
    }

    /// Load a rule from file with caching
    fn load_rule(&mut self, path: &str) -> Result<(Rule, String)> {
        // Check cache first
        if let Some(cached) = self.rule_cache.get(path) {
            return Ok(cached.clone());
        }

        // Resolve full path
        let full_path = self.library_base_path.join(path);

        // Load and parse YAML
        let content =
            std::fs::read_to_string(&full_path).map_err(|e| CompileError::ImportNotFound {
                path: path.to_string(),
                source: e,
            })?;

        let document =
            RuleParser::parse_with_imports(&content).map_err(|e| CompileError::ParseError {
                path: path.to_string(),
                message: e.to_string(),
            })?;

        let rule = document.definition;

        // Cache it
        self.rule_cache
            .insert(path.to_string(), (rule.clone(), path.to_string()));

        Ok((rule, path.to_string()))
    }

    /// Load a ruleset with its dependencies (recursive loading)
    fn load_ruleset_with_deps(&mut self, path: &str) -> Result<(Ruleset, Dependencies)> {
        // Check for circular dependencies
        if self.loading_stack.contains(&path.to_string()) {
            return Err(CompileError::CircularDependency {
                path: path.to_string(),
                stack: self.loading_stack.clone(),
            });
        }

        // Add to loading stack
        self.loading_stack.push(path.to_string());

        // Check cache
        if let Some((cached_ruleset, _)) = self.ruleset_cache.get(path) {
            self.loading_stack.pop();
            // Return cached ruleset but no dependencies (already processed)
            return Ok((cached_ruleset.clone(), Dependencies { rules: vec![] }));
        }

        // 1. Load ruleset file
        let full_path = self.library_base_path.join(path);
        let content =
            std::fs::read_to_string(&full_path).map_err(|e| CompileError::ImportNotFound {
                path: path.to_string(),
                source: e,
            })?;

        let document =
            RulesetParser::parse_with_imports(&content).map_err(|e| CompileError::ParseError {
                path: path.to_string(),
                message: e.to_string(),
            })?;

        // 2. Recursively resolve ruleset's imports (dependency propagation)
        let mut deps_rules = Vec::new();
        if let Some(imports) = &document.imports {
            // Load imported rules
            if !imports.rules.is_empty() {
                for rule_path in &imports.rules {
                    let (rule, _) = self.load_rule(rule_path)?;
                    deps_rules.push(rule);
                }
            }

            // Support rulesets importing other rulesets (deeper propagation)
            if !imports.rulesets.is_empty() {
                for ruleset_path in &imports.rulesets {
                    let (_, sub_deps) = self.load_ruleset_with_deps(ruleset_path)?;
                    deps_rules.extend(sub_deps.rules);
                }
            }
        }

        // 3. Extract ruleset
        let mut ruleset = document.definition;

        // 4. Handle inheritance if ruleset extends another
        if let Some(extends_id) = ruleset.extends.clone() {
            ruleset = self.apply_inheritance(ruleset, &extends_id, path)?;
        }

        // Cache it
        self.ruleset_cache
            .insert(path.to_string(), (ruleset.clone(), path.to_string()));

        // Remove from loading stack
        self.loading_stack.pop();

        Ok((ruleset, Dependencies { rules: deps_rules }))
    }

    /// Apply inheritance from parent ruleset to child ruleset
    ///
    /// Inheritance strategy:
    /// - rules: Merge parent + child rules (auto-dedup)
    /// - conclusion: Child completely overrides parent (if present)
    /// - name: Child overrides parent (if present)
    /// - description: Child overrides parent (if present)
    /// - metadata: Child overrides parent (if present)
    fn apply_inheritance(
        &self,
        mut child: Ruleset,
        extends_id: &str,
        child_path: &str,
    ) -> Result<Ruleset> {
        // Find parent ruleset in cache
        let parent =
            self.find_ruleset_by_id(extends_id)
                .ok_or_else(|| CompileError::ExtendsNotFound {
                    child_id: child.id.clone(),
                    extends_id: extends_id.to_string(),
                    child_path: child_path.to_string(),
                })?;

        // Check for circular inheritance
        if self.has_circular_extends(&parent, extends_id)? {
            return Err(CompileError::CircularExtends {
                child_id: child.id.clone(),
                extends_id: extends_id.to_string(),
            });
        }

        // Merge rules: parent + child (auto-dedup)
        let mut merged_rules = parent.rules.clone();
        for rule_id in &child.rules {
            if !merged_rules.contains(rule_id) {
                merged_rules.push(rule_id.clone());
            }
        }
        child.rules = merged_rules;

        // Override conclusion only if child has it
        if child.conclusion.is_empty() && !parent.conclusion.is_empty() {
            child.conclusion = parent.conclusion.clone();
        }

        // Inherit name if child doesn't have one
        if child.name.is_none() && parent.name.is_some() {
            child.name = parent.name.clone();
        }

        // Inherit description if child doesn't have one
        if child.description.is_none() && parent.description.is_some() {
            child.description = parent.description.clone();
        }

        // Inherit metadata if child doesn't have one
        if child.metadata.is_none() && parent.metadata.is_some() {
            child.metadata = parent.metadata.clone();
        }

        Ok(child)
    }

    /// Find a ruleset by ID in the cache
    fn find_ruleset_by_id(&self, id: &str) -> Option<Ruleset> {
        for (ruleset, _) in self.ruleset_cache.values() {
            if ruleset.id == id {
                return Some(ruleset.clone());
            }
        }
        None
    }

    /// Check if there's a circular extends chain
    fn has_circular_extends(&self, ruleset: &Ruleset, original_id: &str) -> Result<bool> {
        let mut visited = HashSet::new();
        let mut current = ruleset.clone();
        visited.insert(current.id.clone());

        #[allow(clippy::while_let_loop)]
        loop {
            if let Some(extends_id) = &current.extends {
                if extends_id == original_id || visited.contains(extends_id) {
                    return Ok(true); // Circular dependency detected
                }

                visited.insert(extends_id.clone());

                // Find parent
                if let Some(parent) = self.find_ruleset_by_id(extends_id) {
                    current = parent;
                } else {
                    // Parent not found, can't continue check
                    break;
                }
            } else {
                // No more extends, no circular dependency
                break;
            }
        }

        Ok(false)
    }

    /// Deduplicate rules (keep first occurrence)
    fn deduplicate_rules(&self, rules: Vec<Rule>) -> Result<Vec<Rule>> {
        let mut seen = HashSet::new();
        let mut unique_rules = Vec::new();

        for rule in rules {
            if seen.insert(rule.id.clone()) {
                unique_rules.push(rule);
            }
        }

        Ok(unique_rules)
    }

    /// Deduplicate rulesets (keep first occurrence)
    fn deduplicate_rulesets(&self, rulesets: Vec<Ruleset>) -> Result<Vec<Ruleset>> {
        let mut seen = HashSet::new();
        let mut unique_rulesets = Vec::new();

        for ruleset in rulesets {
            if seen.insert(ruleset.id.clone()) {
                unique_rulesets.push(ruleset);
            }
        }

        Ok(unique_rulesets)
    }

    /// Validate all rule/ruleset IDs are unique (compile-time check)
    fn validate_unique_ids(&self, rules: &[Rule], rulesets: &[Ruleset]) -> Result<()> {
        // Check Rule IDs uniqueness
        let mut rule_ids = HashMap::new();
        for rule in rules {
            if let Some(existing_path) = rule_ids.insert(&rule.id, self.get_rule_source(&rule.id)) {
                // Found duplicate Rule ID
                return Err(CompileError::DuplicateRuleId {
                    id: rule.id.clone(),
                    first_defined: existing_path.unwrap_or_else(|| "unknown".to_string()),
                    also_defined: self
                        .get_rule_source(&rule.id)
                        .unwrap_or_else(|| "current import".to_string()),
                });
            }
        }

        // Check Ruleset IDs uniqueness
        let mut ruleset_ids = HashMap::new();
        for ruleset in rulesets {
            if let Some(existing_path) =
                ruleset_ids.insert(&ruleset.id, self.get_ruleset_source(&ruleset.id))
            {
                // Found duplicate Ruleset ID
                return Err(CompileError::DuplicateRulesetId {
                    id: ruleset.id.clone(),
                    first_defined: existing_path.unwrap_or_else(|| "unknown".to_string()),
                    also_defined: self
                        .get_ruleset_source(&ruleset.id)
                        .unwrap_or_else(|| "current import".to_string()),
                });
            }
        }

        // Check Rule ID and Ruleset ID don't conflict
        // (Although they're in different namespaces, we check to avoid confusion)
        for ruleset_id in ruleset_ids.keys() {
            if rule_ids.contains_key(ruleset_id) {
                return Err(CompileError::IdConflict {
                    id: ruleset_id.to_string(),
                    conflict: "Rule ID conflicts with Ruleset ID".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get the source file path for a rule (used for error messages)
    fn get_rule_source(&self, rule_id: &str) -> Option<String> {
        for (path, (rule, _)) in &self.rule_cache {
            if rule.id == rule_id {
                return Some(path.clone());
            }
        }
        None
    }

    /// Get the source file path for a ruleset (used for error messages)
    fn get_ruleset_source(&self, ruleset_id: &str) -> Option<String> {
        for (path, (ruleset, _)) in &self.ruleset_cache {
            if ruleset.id == ruleset_id {
                return Some(path.clone());
            }
        }
        None
    }

    /// Clear all caches
    pub fn clear_cache(&mut self) {
        self.rule_cache.clear();
        self.ruleset_cache.clear();
        self.loading_stack.clear();
    }

    /// Get the library base path
    pub fn library_base_path(&self) -> &Path {
        &self.library_base_path
    }
}

/// Dependencies loaded from a ruleset
#[derive(Debug, Clone)]
struct Dependencies {
    rules: Vec<Rule>,
}

/// Resolved document with all imports merged
#[derive(Debug, Clone)]
pub struct ResolvedDocument {
    pub rules: Vec<Rule>,
    pub rulesets: Vec<Ruleset>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_resolver_new() {
        let resolver = ImportResolver::new("repository");
        assert_eq!(resolver.library_base_path(), Path::new("repository"));
        assert_eq!(resolver.rule_cache.len(), 0);
        assert_eq!(resolver.ruleset_cache.len(), 0);
    }

    #[test]
    fn test_import_resolver_clear_cache() {
        let mut resolver = ImportResolver::new("repository");
        resolver.clear_cache();
        assert_eq!(resolver.rule_cache.len(), 0);
    }
}
