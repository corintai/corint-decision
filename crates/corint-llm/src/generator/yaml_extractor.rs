//! YAML extraction utilities for cleaning LLM responses

use crate::error::{LLMError, Result};

/// Extracts valid YAML content from LLM response
///
/// This function handles common LLM output patterns:
/// - Markdown code blocks (```yaml ... ```)
/// - Explanatory text before/after YAML
/// - Multiple YAML documents separated by ---
///
/// # Arguments
/// * `llm_output` - Raw output from LLM
///
/// # Returns
/// * `Ok(String)` - Clean YAML content
/// * `Err(LLMError)` - If no valid YAML found
pub fn extract_yaml(llm_output: &str) -> Result<String> {
    let cleaned = llm_output.trim();

    // Try to extract from markdown code blocks first
    if let Some(yaml_content) = extract_from_markdown(cleaned) {
        return validate_and_return(&yaml_content);
    }

    // Try to find YAML by looking for common start patterns
    if let Some(yaml_content) = extract_by_pattern(cleaned) {
        return validate_and_return(&yaml_content);
    }

    // If nothing worked, try to validate the entire content as YAML
    validate_and_return(cleaned)
}

/// Extract YAML from markdown code blocks
fn extract_from_markdown(content: &str) -> Option<String> {
    // Pattern: ```yaml\n...\n```
    if let Some(start) = content.find("```yaml") {
        let after_start = &content[start + 7..]; // Skip ```yaml
        if let Some(end) = after_start.find("```") {
            let yaml_content = &after_start[..end];
            return Some(yaml_content.trim().to_string());
        }
    }

    // Pattern: ```\n...\n```
    if let Some(start) = content.find("```") {
        let after_start = &content[start + 3..]; // Skip ```
        if let Some(end) = after_start.find("```") {
            let yaml_content = &after_start[..end];
            // Only return if it looks like YAML
            if is_likely_yaml(yaml_content) {
                return Some(yaml_content.trim().to_string());
            }
        }
    }

    None
}

/// Extract YAML by identifying common YAML start patterns
fn extract_by_pattern(content: &str) -> Option<String> {
    let patterns = [
        "rule:",
        "ruleset:",
        "pipeline:",
        "name:", // for API configs
    ];

    let trimmed = content.trim();

    // First, check if content starts with any of the patterns
    for pattern in &patterns {
        if trimmed.starts_with(pattern) {
            // Try to find where YAML ends (common patterns of explanatory text)
            let end_markers = [
                "\n\nNote:",
                "\n\nExplanation:",
                "\n\nThis rule",
                "\n\nThe above",
            ];

            let mut yaml_end = trimmed.len();
            for marker in &end_markers {
                if let Some(pos) = trimmed.find(marker) {
                    yaml_end = yaml_end.min(pos);
                }
            }

            return Some(trimmed[..yaml_end].trim().to_string());
        }
    }

    // If not at the start, check if it appears after a document separator
    for pattern in &patterns {
        let pattern_with_sep = format!("---\n{}", pattern);
        if let Some(start) = content.find(&pattern_with_sep) {
            let yaml_candidate = &content[start + 4..]; // Skip "---\n"

            let end_markers = [
                "\n\nNote:",
                "\n\nExplanation:",
                "\n\nThis rule",
                "\n\nThe above",
            ];

            let mut yaml_end = yaml_candidate.len();
            for marker in &end_markers {
                if let Some(pos) = yaml_candidate.find(marker) {
                    yaml_end = yaml_end.min(pos);
                }
            }

            return Some(yaml_candidate[..yaml_end].trim().to_string());
        }
    }

    None
}

/// Check if content looks like YAML
fn is_likely_yaml(content: &str) -> bool {
    let trimmed = content.trim();

    // Check for YAML-like patterns
    let has_colon = trimmed.contains(':');
    let has_dash = trimmed.contains('-');
    let starts_with_key = trimmed.lines().next().map_or(false, |line| {
        line.trim().contains(':') || line.trim().starts_with('-')
    });

    has_colon && starts_with_key || has_dash && starts_with_key
}

/// Validate YAML and return if valid
fn validate_and_return(content: &str) -> Result<String> {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err(LLMError::InvalidResponse(
            "Empty YAML content".to_string(),
        ));
    }

    // Check if content looks like YAML before parsing
    if !is_likely_yaml(trimmed) {
        return Err(LLMError::InvalidResponse(
            "Content does not appear to be valid YAML".to_string(),
        ));
    }

    // For multiple documents, validate each separately
    if trimmed.contains("\n---\n") || trimmed.starts_with("---\n") {
        // This is handled by extract_multiple_yaml
        // Just do basic validation that it contains YAML-like content
        return Ok(trimmed.to_string());
    }

    // Basic YAML validation - try to parse it
    serde_yaml::from_str::<serde_yaml::Value>(trimmed).map_err(|e| {
        LLMError::InvalidResponse(format!("Invalid YAML: {}", e))
    })?;

    Ok(trimmed.to_string())
}

/// Extract multiple YAML documents separated by ---
///
/// # Arguments
/// * `llm_output` - Raw output from LLM containing multiple YAML documents
///
/// # Returns
/// * `Ok(Vec<String>)` - Vector of clean YAML documents
/// * `Err(LLMError)` - If no valid YAML found
pub fn extract_multiple_yaml(llm_output: &str) -> Result<Vec<String>> {
    // First extract the main YAML content
    let yaml_content = extract_yaml(llm_output)?;

    // Split by document separator (handle various formats)
    // Match: newline + optional whitespace + --- + optional whitespace + newline
    let separator_pattern = regex::Regex::new(r"\n\s*---\s*\n").unwrap();

    let documents: Vec<String> = separator_pattern
        .split(&yaml_content)
        .filter_map(|doc| {
            let trimmed = doc.trim();
            if trimmed.is_empty() {
                return None;
            }

            // For individual documents, skip the multi-document check
            match validate_single_document(trimmed) {
                Ok(valid_doc) => Some(valid_doc),
                Err(_) => None,
            }
        })
        .collect();

    if documents.is_empty() {
        return Err(LLMError::InvalidResponse(
            "No valid YAML documents found".to_string(),
        ));
    }

    Ok(documents)
}

/// Validate a single YAML document (not multi-document)
fn validate_single_document(content: &str) -> Result<String> {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err(LLMError::InvalidResponse(
            "Empty YAML content".to_string(),
        ));
    }

    // Check if content looks like YAML
    if !is_likely_yaml(trimmed) {
        return Err(LLMError::InvalidResponse(
            "Content does not appear to be valid YAML".to_string(),
        ));
    }

    // Basic YAML validation - try to parse it
    serde_yaml::from_str::<serde_yaml::Value>(trimmed).map_err(|e| {
        LLMError::InvalidResponse(format!("Invalid YAML: {}", e))
    })?;

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_markdown_yaml() {
        let input = r#"Here's the rule:

```yaml
rule:
  id: test_rule
  description: Test
```

This is a test rule."#;

        let result = extract_yaml(input).unwrap();
        assert!(result.contains("rule:"));
        assert!(result.contains("test_rule"));
    }

    #[test]
    fn test_extract_from_markdown_generic() {
        let input = r#"```
rule:
  id: test_rule
  description: Test
```"#;

        let result = extract_yaml(input).unwrap();
        assert!(result.contains("rule:"));
    }

    #[test]
    fn test_extract_direct_yaml() {
        let input = r#"rule:
  id: test_rule
  description: Test
  when:
    all:
      - event.amount > 1000"#;

        let result = extract_yaml(input).unwrap();
        assert!(result.contains("rule:"));
        assert!(result.contains("test_rule"));
    }

    #[test]
    fn test_extract_with_explanation_after() {
        let input = r#"rule:
  id: test_rule
  description: Test

Note: This rule checks for high amounts."#;

        let result = extract_yaml(input).unwrap();
        assert!(result.contains("rule:"));
        assert!(!result.contains("Note:"));
    }

    #[test]
    fn test_extract_multiple_documents() {
        let input = r#"rule:
  id: rule_1
  description: First rule
---
rule:
  id: rule_2
  description: Second rule"#;

        let results = extract_multiple_yaml(input).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].contains("rule_1"));
        assert!(results[1].contains("rule_2"));
    }

    #[test]
    fn test_extract_multiple_with_markdown() {
        let input = r#"```yaml
rule:
  id: rule_1
  description: First
---
rule:
  id: rule_2
  description: Second
```"#;

        let results = extract_multiple_yaml(input).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_invalid_yaml_error() {
        let input = "This is not YAML at all, just random text.";
        let result = extract_yaml(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_content_error() {
        let input = "   \n\n  ";
        let result = extract_yaml(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_malformed_yaml_error() {
        let input = r#"rule:
  id: test
  invalid: [unclosed bracket
  more: stuff"#;

        let result = extract_yaml(input);
        assert!(result.is_err());
    }
}
