use std::ffi::{c_char, CStr};

#[unsafe(no_mangle)]
pub extern "C" fn share_total(text: *const c_char) -> i64 {
    if text.is_null() {
        return -1;
    }
    let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
        return -1;
    };
    let mut total = 0;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some((_, cents)) = line.rsplit_once(' ') else {
            return -1;
        };
        let Ok(cents) = cents.parse::<i64>() else {
            return -1;
        };
        total += cents;
    }
    total
}

#[unsafe(no_mangle)]
pub extern "C" fn share_platform() -> *const c_char {
    if cfg!(all(target_os = "ios", target_abi = "sim")) {
        c"iphonesimulator".as_ptr()
    } else if cfg!(all(target_os = "ios", target_arch = "x86_64")) {
        c"iphonesimulator".as_ptr()
    } else if cfg!(target_os = "ios") {
        c"iphoneos".as_ptr()
    } else {
        c"other".as_ptr()
    }
}
