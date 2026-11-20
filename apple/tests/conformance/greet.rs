#![expect(unsafe_code, reason = "exports an unmangled C symbol for the native linker fixture")]

use std::ffi::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn greet() -> *const c_char {
    c"hello world".as_ptr()
}
