//! FFI utility functions

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Helper to convert Rust string to C string
pub fn to_c_string(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Helper to convert C string to Rust string
pub unsafe fn from_c_string(s: *const c_char) -> Option<String> {
    if s.is_null() {
        return None;
    }
    CStr::from_ptr(s).to_str().ok().map(|s| s.to_owned())
}
