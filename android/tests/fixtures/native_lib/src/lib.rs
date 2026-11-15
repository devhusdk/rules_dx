#![expect(
    unsafe_code,
    reason = "the fixture calls the native C dependency and exports unmangled symbols"
)]

include!(concat!(env!("OUT_DIR"), "/build_probe.rs"));

#[cfg(not(has_native_lib_stamp))]
compile_error!("android build script stamp is missing");

unsafe extern "C" {
    fn native_add(left: i32, right: i32) -> i32;
    fn native_offset() -> i32;
}

#[unsafe(no_mangle)]
pub extern "C" fn native_lib_add(left: i32, right: i32) -> i32 {
    // SAFETY: native_add and native_offset take plain integers and have no preconditions.
    unsafe { native_add(left, right) }
}

#[unsafe(no_mangle)]
pub extern "C" fn native_lib_offset() -> i32 {
    // SAFETY: native_add and native_offset take plain integers and have no preconditions.
    unsafe { native_offset() }
}

#[cfg(feature = "neon")]
#[unsafe(no_mangle)]
pub extern "C" fn native_lib_accelerated() -> u32 {
    1
}

pub fn build_script_host() -> &'static str {
    BUILD_SCRIPT_HOST
}

pub fn build_script_target() -> &'static str {
    BUILD_SCRIPT_TARGET
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_add_reaches_the_native_dependency() {
        // SAFETY: native_add and native_offset take plain integers and have no preconditions.
        assert_eq!(unsafe { native_add(2, 3) }, 12);
        // SAFETY: native_add and native_offset take plain integers and have no preconditions.
        assert_eq!(unsafe { native_offset() }, 7);
    }

    #[test]
    fn build_script_records_the_host_toolchain() {
        assert!(!build_script_host().is_empty());
        assert_eq!(build_script_host(), build_script_target());
    }
}
