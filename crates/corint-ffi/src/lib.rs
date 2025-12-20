//! CORINT Decision Engine FFI
//!
//! Foreign Function Interface for calling CORINT from other languages.
//! This crate provides C-compatible bindings for Python, Go, TypeScript, and Java.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::sync::Arc;

use corint_sdk::{DecisionEngineBuilder, DecisionRequest, RepositoryConfig};
use serde_json::Value as JsonValue;

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
        engine_ref.engine.decide(request).await
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
