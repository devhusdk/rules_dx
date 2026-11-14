use cxx::let_cxx_string;
use std::os::raw::{c_char, c_ulong};

unsafe extern "C" {
    fn zlibVersion() -> *const c_char;
    fn crc32(crc: c_ulong, buf: *const u8, len: u32) -> c_ulong;
}

pub fn zlib_version() -> String {
    // SAFETY: zlibVersion returns a static NUL-terminated string.
    unsafe { std::ffi::CStr::from_ptr(zlibVersion()) }
        .to_string_lossy()
        .into_owned()
}

pub fn checksum(bytes: &[u8]) -> u64 {
    let len = u32::try_from(bytes.len()).expect("fits in u32");
    // SAFETY: the pointer and length describe `bytes`.
    u64::from(unsafe { crc32(0, bytes.as_ptr(), len) })
}

pub fn cxx_string(text: &str) -> String {
    let_cxx_string!(value = text);
    value.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_crate_links() {
        assert!(zlib_version().starts_with("1."));
        assert_eq!(checksum(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn cpp_crate_links() {
        assert_eq!(cxx_string("cc-rs"), "cc-rs");
    }
}
