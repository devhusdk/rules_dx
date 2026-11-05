//! Product core of the Android app; the activity in `activity.c` calls into it.

use std::ffi::c_char;

const GREETING: &[u8] = b"Hello from Rust on Android!\0";

/// Returns the greeting the activity logs at startup.
pub fn greeting() -> &'static str {
    "Hello from Rust on Android!"
}

/// Returns the greeting as a NUL-terminated C string for the activity.
#[no_mangle]
pub extern "C" fn app_greeting() -> *const c_char {
    GREETING.as_ptr().cast()
}

#[cfg(test)]
mod tests {
    use super::{app_greeting, greeting};
    use std::ffi::CStr;

    #[test]
    fn c_greeting_matches_the_rust_greeting() {
        // SAFETY: app_greeting returns a pointer to a static NUL-terminated string.
        let text = unsafe { CStr::from_ptr(app_greeting()) };
        assert_eq!(text.to_str(), Ok(greeting()));
    }
}
