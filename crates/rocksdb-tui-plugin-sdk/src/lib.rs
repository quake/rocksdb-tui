//! SDK for building rocksdb-tui WASM plugins
//!
//! This crate provides helper functions and macros for implementing
//! WASM plugins that can parse custom binary formats.
//!
//! # Example
//!
//! ```ignore
//! use rocksdb_tui_plugin_sdk::*;
//!
//! // Define your parser function
//! // The key is provided so plugins can route based on key prefix
//! fn parse_format(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
//!     // For single-CF databases with prefix routing (like Fiber):
//!     if format == "fiber" {
//!         return match key.first() {
//!             Some(0x00) => parse_channel_state(value),
//!             Some(0x01) => parse_payment_session(value),
//!             _ => Err("Unknown prefix".to_string()),
//!         };
//!     }
//!     // For format-per-type databases:
//!     match format {
//!         "myapp.User" => {
//!             let user: User = bincode::deserialize(value).map_err(|e| e.to_string())?;
//!             serde_json::to_string_pretty(&user).map_err(|e| e.to_string())
//!         }
//!         _ => Err(format!("Unknown format: {}", format))
//!     }
//! }
//!
//! // Export the plugin
//! export_plugin! {
//!     formats: ["fiber", "myapp.User"],
//!     parse: parse_format
//! }
//! ```

use std::alloc::{alloc, dealloc, Layout};
use std::slice;
use std::str;

/// Pack a pointer and length into a single u64 value
/// Format: (ptr << 32) | len
#[inline]
pub fn pack_ptr_len(ptr: u32, len: u32) -> u64 {
    ((ptr as u64) << 32) | (len as u64)
}

/// Allocate memory and return a pointer
/// This is exported as `alloc` from the WASM module
#[inline]
pub fn sdk_alloc(size: u32) -> u32 {
    if size == 0 {
        return 0;
    }
    // Allocate with proper alignment
    let layout = match Layout::from_size_align(size as usize, std::mem::align_of::<u8>()) {
        Ok(l) => l,
        Err(_) => return 0,
    };
    unsafe { alloc(layout) as u32 }
}

/// Deallocate memory
/// This is exported as `dealloc` from the WASM module
#[inline]
pub fn sdk_dealloc(ptr: u32, len: u32) {
    if ptr == 0 || len == 0 {
        return;
    }
    let layout = match Layout::from_size_align(len as usize, std::mem::align_of::<u8>()) {
        Ok(l) => l,
        Err(_) => return,
    };
    unsafe { dealloc(ptr as *mut u8, layout) }
}

/// Read a string from WASM memory
/// # Safety
/// The caller must ensure ptr and len are valid
#[inline]
pub unsafe fn read_str(ptr: u32, len: u32) -> &'static str {
    let slice = slice::from_raw_parts(ptr as *const u8, len as usize);
    str::from_utf8_unchecked(slice)
}

/// Read bytes from WASM memory  
/// # Safety
/// The caller must ensure ptr and len are valid
#[inline]
pub unsafe fn read_bytes(ptr: u32, len: u32) -> &'static [u8] {
    slice::from_raw_parts(ptr as *const u8, len as usize)
}

/// Allocate and write a string to WASM memory, returning packed ptr+len
pub fn pack_string(s: &str) -> u64 {
    let bytes = s.as_bytes();
    let ptr = sdk_alloc(bytes.len() as u32);
    if ptr != 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
        }
    }
    pack_ptr_len(ptr, bytes.len() as u32)
}

/// Allocate and write a JSON array of format names
pub fn pack_formats(formats: &[&str]) -> u64 {
    let json = serde_json::to_string(formats).unwrap_or_else(|_| "[]".to_string());
    pack_string(&json)
}

/// Macro to export a plugin with the required WASM interface
///
/// The parse function receives format, key, and value. The key is provided
/// so plugins can route parsing based on key prefixes (useful for single-CF
/// databases like Fiber that use prefix bytes to distinguish value types).
///
/// # Example
///
/// ```ignore
/// use rocksdb_tui_plugin_sdk::*;
///
/// fn my_parser(format: &str, key: &[u8], value: &[u8]) -> Result<String, String> {
///     // Route by key prefix for single-CF databases
///     if format == "mydb" {
///         return match key.first() {
///             Some(0x00) => parse_type_a(value),
///             Some(0x01) => parse_type_b(value),
///             _ => Ok("{}".to_string()),
///         };
///     }
///     Ok("{}".to_string())
/// }
///
/// export_plugin! {
///     formats: ["mydb"],
///     parse: my_parser
/// }
/// ```
#[macro_export]
macro_rules! export_plugin {
    (
        formats: [$($format:literal),* $(,)?],
        parse: $parse_fn:expr
    ) => {
        #[no_mangle]
        pub extern "C" fn alloc(size: u32) -> u32 {
            $crate::sdk_alloc(size)
        }

        #[no_mangle]
        pub extern "C" fn dealloc(ptr: u32, len: u32) {
            $crate::sdk_dealloc(ptr, len)
        }

        #[no_mangle]
        pub extern "C" fn get_formats() -> u64 {
            static FORMATS: &[&str] = &[$($format),*];
            $crate::pack_formats(FORMATS)
        }

        #[no_mangle]
        pub extern "C" fn parse(
            format_ptr: u32,
            format_len: u32,
            key_ptr: u32,
            key_len: u32,
            value_ptr: u32,
            value_len: u32,
        ) -> u64 {
            let format = unsafe { $crate::read_str(format_ptr, format_len) };
            let key = unsafe { $crate::read_bytes(key_ptr, key_len) };
            let value = unsafe { $crate::read_bytes(value_ptr, value_len) };

            match ($parse_fn)(format, key, value) {
                Ok(json) => $crate::pack_string(&json),
                Err(e) => {
                    let error_json = serde_json::json!({
                        "error": e.to_string()
                    }).to_string();
                    $crate::pack_string(&error_json)
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_ptr_len() {
        let packed = pack_ptr_len(0x12345678, 0x00000100);
        assert_eq!(packed >> 32, 0x12345678);
        assert_eq!(packed & 0xFFFFFFFF, 0x00000100);
    }

    #[test]
    fn test_sdk_alloc_zero() {
        let ptr = sdk_alloc(0);
        assert_eq!(ptr, 0);
    }

    // Note: Tests that involve memory allocation and dereferencing are only
    // valid on 32-bit platforms (like WASM32) because this SDK uses u32 pointers.
    // On 64-bit platforms, pointer truncation causes undefined behavior.
    //
    // To test the full SDK, use: cargo test --target wasm32-unknown-unknown
}
