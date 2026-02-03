//! WASM Plugin ABI Definition
//!
//! Plugins must export the following functions:
//!
//! ## Required Exports
//!
//! ### `alloc(size: u32) -> u32`
//! Allocate `size` bytes of memory and return the pointer.
//!
//! ### `dealloc(ptr: u32, size: u32)`
//! Free memory previously allocated by `alloc`.
//!
//! ### `get_formats() -> u64`
//! Return a pointer-length pair (ptr << 32 | len) to a JSON array of supported format strings.
//! Example: `["myapp.TypeA", "myapp.TypeB"]`
//!
//! ### `parse(format_ptr: u32, format_len: u32, key_ptr: u32, key_len: u32, value_ptr: u32, value_len: u32) -> u64`
//! Parse binary data according to the specified format.
//! The key is provided so plugins can use key prefixes to determine the value type.
//! Returns a pointer-length pair to a JSON string, or 0 on failure.

/// Symbol name for memory allocation function
pub const EXPORT_ALLOC: &str = "alloc";

/// Symbol name for memory deallocation function
pub const EXPORT_DEALLOC: &str = "dealloc";

/// Symbol name for getting supported formats
pub const EXPORT_GET_FORMATS: &str = "get_formats";

/// Symbol name for parsing data
pub const EXPORT_PARSE: &str = "parse";

/// Symbol name for WASM memory
pub const EXPORT_MEMORY: &str = "memory";

/// Unpack a u64 result into (ptr, len)
#[inline]
pub fn unpack_ptr_len(packed: u64) -> (u32, u32) {
    let ptr = (packed >> 32) as u32;
    let len = (packed & 0xFFFFFFFF) as u32;
    (ptr, len)
}
