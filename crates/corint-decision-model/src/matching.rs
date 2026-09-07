//! Shared bounded regular expressions for Core validation and VM execution.
use regex::{Regex, RegexBuilder};
use std::{collections::VecDeque, sync::Mutex};

pub const MAX_PATTERN_BYTES: usize = 4096;
pub const MAX_REGEX_BYTES: usize = 1024 * 1024;
pub const MAX_REGEX_NESTING: u32 = 64;
const CACHE_ENTRIES: usize = 32;
static CACHE: Mutex<VecDeque<(String, Regex)>> = Mutex::new(VecDeque::new());

/// Uses Unicode-aware search semantics. Patterns cannot use backreferences or
/// look-around. Successful compilations are cached with a fixed entry bound;
/// eviction changes cost only, never results. No lock is held during matching.
pub fn compile_regex(pattern: &str) -> Result<Regex, &'static str> {
    if pattern.len() > MAX_PATTERN_BYTES {
        return Err("Regex pattern exceeds 4096 bytes");
    }
    {
        let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, compiled)) = cache.iter().find(|(p, _)| p == pattern) {
            return Ok(compiled.clone());
        }
    }
    let compiled = RegexBuilder::new(pattern)
        .size_limit(MAX_REGEX_BYTES)
        .dfa_size_limit(256 * 1024)
        .nest_limit(MAX_REGEX_NESTING)
        .build()
        .map_err(|_| "Invalid regex or regex exceeds compilation limits")?;
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if !cache.iter().any(|(p, _)| p == pattern) {
        if cache.len() == CACHE_ENTRIES {
            cache.pop_front();
        }
        cache.push_back((pattern.into(), compiled.clone()));
    }
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex_limits_and_cache_preserve_search_semantics() {
        for pattern in ["[", "(?=x)", r"(x)\1", "a{10000000}"] {
            assert!(compile_regex(pattern).is_err(), "{pattern}");
        }
        assert!(compile_regex(&"a".repeat(MAX_PATTERN_BYTES + 1)).is_err());
        assert!(compile_regex(&format!("{}a{}", "(".repeat(65), ")".repeat(65))).is_err());
        assert!(compile_regex("支付").unwrap().is_match("一次支付成功"));
        assert!(!compile_regex("^支付$").unwrap().is_match("一次支付成功"));
        assert!(compile_regex("").unwrap().is_match(""));
        for i in 0..CACHE_ENTRIES + 1 {
            assert!(compile_regex(&format!("^{i}$"))
                .unwrap()
                .is_match(&i.to_string()));
        }
        assert!(CACHE.lock().unwrap().len() <= CACHE_ENTRIES);
        assert!(compile_regex("支付").unwrap().is_match("一次支付成功"));
    }
}
