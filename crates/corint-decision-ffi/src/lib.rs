//! CORINT Decision Engine FFI
//!
//! Foreign Function Interface for calling CORINT from other languages.
//! This crate provides C-compatible bindings for Python, Go, TypeScript, and Java.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

use corint_decision_engine::snapshot::EngineManager;
use corint_decision_sdk::{DecisionEngineBuilder, DecisionRequest, RepositoryConfig};

mod types;
mod utils;

pub use types::*;
pub use utils::*;

/// Initialize the logging system
#[no_mangle]
pub extern "C" fn corint_init_logging() {
    env_logger::init();
}

/// Create a new decision engine from a repository path
///
/// # Safety
/// - repository_path must be a valid null-terminated C string
/// - The returned pointer must be freed with corint_engine_free
#[no_mangle]
pub unsafe extern "C" fn corint_engine_new(repository_path: *const c_char) -> *mut CorintEngine {
    if repository_path.is_null() {
        return ptr::null_mut();
    }

    let path = match CStr::from_ptr(repository_path).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return ptr::null_mut(),
    };

    let engine = match runtime.block_on(async {
        DecisionEngineBuilder::new()
            .with_repository(RepositoryConfig::file_system(path))
            .build()
            .await
    }) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };

    let engine = match EngineManager::new(Arc::new(engine)) {
        Ok(engine) => engine,
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(CorintEngine {
        engine: Arc::new(engine),
        runtime: Arc::new(runtime),
    }))
}

/// Create a new decision engine from a database URL
///
/// # Safety
/// - database_url must be a valid null-terminated C string
/// - The returned pointer must be freed with corint_engine_free
#[no_mangle]
pub unsafe extern "C" fn corint_engine_new_from_database(
    database_url: *const c_char,
) -> *mut CorintEngine {
    if database_url.is_null() {
        return ptr::null_mut();
    }

    let url = match CStr::from_ptr(database_url).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return ptr::null_mut(),
    };

    let engine = match runtime.block_on(async {
        DecisionEngineBuilder::new()
            .with_repository(RepositoryConfig::database(url))
            .build()
            .await
    }) {
        Ok(e) => e,
        Err(_) => return ptr::null_mut(),
    };

    let engine = match EngineManager::new(Arc::new(engine)) {
        Ok(engine) => engine,
        Err(_) => return ptr::null_mut(),
    };
    Box::into_raw(Box::new(CorintEngine {
        engine: Arc::new(engine),
        runtime: Arc::new(runtime),
    }))
}

/// Execute a decision using the engine
///
/// # Safety
/// - engine must be a valid pointer created by corint_engine_new
/// - request_json must be a valid null-terminated C string containing JSON
/// - The returned string must be freed with corint_string_free
#[no_mangle]
pub unsafe extern "C" fn corint_engine_decide(
    engine: *mut CorintEngine,
    request_json: *const c_char,
) -> *mut c_char {
    if engine.is_null() || request_json.is_null() {
        return ptr::null_mut();
    }

    let engine_ref = &*engine;

    let json_str = match CStr::from_ptr(request_json).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    // Parse as DecisionRequest directly, which will handle all the fields
    let request: DecisionRequest = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return ptr::null_mut(),
    };

    let result = match engine_ref.runtime.block_on(async {
        let snapshot = engine_ref.engine.snapshot().await;
        let mut response = snapshot.engine.decide(request).await?;
        response
            .metadata
            .insert("runtime_revision".into(), snapshot.revision.clone());
        response
            .metadata
            .insert("compiled_sha256".into(), snapshot.compiled_sha256.clone());
        Ok::<_, corint_decision_sdk::EngineError>(response)
    }) {
        Ok(r) => r,
        Err(e) => {
            let error_response = serde_json::json!({
                "error": e.to_string(),
                "success": false
            });
            match serde_json::to_string(&error_response) {
                Ok(s) => return CString::new(s).unwrap().into_raw(),
                Err(_) => return ptr::null_mut(),
            }
        }
    };

    let response_json = match serde_json::to_string(&result) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    match CString::new(response_json) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Free a decision engine
///
/// # Safety
/// - engine must be a valid pointer created by corint_engine_new
/// - After calling this function, the pointer is invalid and must not be used
#[no_mangle]
pub unsafe extern "C" fn corint_engine_free(engine: *mut CorintEngine) {
    if !engine.is_null() {
        drop(Box::from_raw(engine));
    }
}

/// Free a string returned by the FFI
///
/// # Safety
/// - s must be a valid pointer returned by a corint_* function
/// - After calling this function, the pointer is invalid and must not be used
#[no_mangle]
pub unsafe extern "C" fn corint_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// Get the version of the CORINT library
///
/// # Safety
/// - The returned string must be freed with corint_string_free
#[no_mangle]
pub extern "C" fn corint_version() -> *mut c_char {
    let version = env!("CARGO_PKG_VERSION");
    match CString::new(version) {
        Ok(s) => s.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        unsafe {
            let version = corint_version();
            assert!(!version.is_null());
            let version_str = CStr::from_ptr(version).to_str().unwrap();
            assert!(!version_str.is_empty());
            corint_string_free(version);
        }
    }
}

/// Atomically reload the configured repository using an expected runtime revision.
/// Returns JSON with success and identity or a stable error code. No policy upload.
/// # Safety
/// Pointers must be valid; expected_revision is a NUL-terminated UTF-8 string.
/// Keep the engine alive for all concurrent calls; free the returned string.
#[no_mangle]
pub unsafe extern "C" fn corint_engine_reload(
    engine: *mut CorintEngine,
    expected_revision: *const c_char,
) -> *mut c_char {
    if engine.is_null() || expected_revision.is_null() {
        return ptr::null_mut();
    }
    let expected = match CStr::from_ptr(expected_revision).to_str() {
        Ok(s) if !s.is_empty() => s,
        _ => return ptr::null_mut(),
    };
    let engine = &*engine;
    let value = match engine
        .runtime
        .block_on(engine.engine.reload(Some(expected)))
    {
        Ok(s) => {
            serde_json::json!({"success":true,"runtime_revision":s.revision,"compiled_sha256":s.compiled_sha256})
        }
        Err(error) => {
            use corint_decision_engine::snapshot::ReloadError;
            let code = match error {
                ReloadError::Busy => "RELOAD_BUSY",
                ReloadError::Stale => "REVISION_CONFLICT",
                _ => "RELOAD_FAILED",
            };
            serde_json::json!({"success":false,"error":code})
        }
    };
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;
    const PIPELINE: &str = r#"version: "0.1"
import:
  rulesets: [library/rulesets/risk.yaml]
---
pipeline:
  id: payment
  name: Payment
  entry: check
  when:
    all: [event.amount > 0]
  steps:
    - step:
        id: check
        name: Check
        type: ruleset
        ruleset: risk
        next: end
  decision:
    - when: results.risk.score >= 60
      result: decline
      actions: [BLOCK]
    - default: true
      result: approve
"#;
    const RULESET: &str = r#"version: "0.1"
import:
  rules: [library/rules/amount.yaml]
---
ruleset:
  id: risk
  rules: [amount]
  conclusion:
    - when: total_score >= 60
      signal: decline
    - default: true
      signal: approve
"#;
    fn write_rule(root: &Path, score: i32) {
        std::fs::write(
            root.join("library/rules/amount.yaml"),
            format!(
                r#"version: "0.1"
rule:
  id: amount
  name: Amount
  when: event.amount > 0
  score: {score}
"#
            ),
        )
        .unwrap();
    }
    fn repository() -> TempDir {
        let dir = TempDir::new().unwrap();
        for path in ["pipelines", "library/rules", "library/rulesets"] {
            std::fs::create_dir_all(dir.path().join(path)).unwrap();
        }
        std::fs::write(dir.path().join("pipelines/payment.yaml"), PIPELINE).unwrap();
        std::fs::write(dir.path().join("library/rulesets/risk.yaml"), RULESET).unwrap();
        write_rule(dir.path(), 10);
        dir
    }

    unsafe fn json(ptr: *mut c_char) -> serde_json::Value {
        assert!(!ptr.is_null());
        let value = serde_json::from_str(CStr::from_ptr(ptr).to_str().unwrap()).unwrap();
        corint_string_free(ptr);
        value
    }
    #[test]
    fn ffi_decisions_and_reload_pin_a_single_version_and_match_native_domain() {
        let repo = repository();
        unsafe {
            let path = CString::new(repo.path().to_str().unwrap()).unwrap();
            let handle = corint_engine_new(path.as_ptr());
            assert!(!handle.is_null());
            let request =
                CString::new(r#"{"event_data":{"amount":100},"options":{"enable_trace":true}}"#)
                    .unwrap();
            let first = json(corint_engine_decide(handle, request.as_ptr()));
            assert_eq!(first["result"]["signal"]["type"], "approve");
            assert!(first["trace"].is_object());
            let revision =
                CString::new(first["metadata"]["runtime_revision"].as_str().unwrap()).unwrap();
            write_rule(repo.path(), 70);
            let reload = json(corint_engine_reload(handle, revision.as_ptr()));
            assert_eq!(reload["success"], true);
            let second = json(corint_engine_decide(handle, request.as_ptr()));
            assert_eq!(second["result"]["signal"]["type"], "decline");
            assert_eq!(
                second["metadata"]["runtime_revision"],
                reload["runtime_revision"]
            );
            assert_ne!(
                first["metadata"]["compiled_sha256"],
                second["metadata"]["compiled_sha256"]
            );
            assert_eq!(
                json(corint_engine_reload(handle, revision.as_ptr()))["error"],
                "REVISION_CONFLICT"
            );
            let revision =
                CString::new(second["metadata"]["runtime_revision"].as_str().unwrap()).unwrap();
            std::fs::write(repo.path().join("pipelines/payment.yaml"), "invalid: [").unwrap();
            assert_eq!(
                json(corint_engine_reload(handle, revision.as_ptr()))["error"],
                "RELOAD_FAILED"
            );
            assert_eq!(
                json(corint_engine_decide(handle, request.as_ptr()))["metadata"]
                    ["runtime_revision"],
                second["metadata"]["runtime_revision"]
            );
            corint_engine_free(handle);
        }
    }
}
