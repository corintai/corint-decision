//! Shared, offline CDL Core behavior tests and experimental source packages.
//! Synchronous entry points create a runtime; async callers must use a blocking
//! worker. No Work dependency, LLM call, deployment or business evaluation.
pub mod behavior;
pub mod contracts;
pub mod package;
pub mod phase0;
pub mod repository;
pub mod resolve;
pub mod transfer;

use corint_decision_compiler::core::{diagnostic, CoreError};
use std::path::Path;

fn failure(source: &str, stage: &str, code: &str, message: impl Into<String>) -> CoreError {
    diagnostic(source, "", stage, code, message)
}
fn label(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
