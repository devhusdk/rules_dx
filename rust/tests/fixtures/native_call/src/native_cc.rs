include!(concat!(env!("OUT_DIR"), "/cc_probe.rs"));

unsafe extern "C" {
    fn native_cc_c_value() -> i32;
    fn native_cc_cxx_value() -> i32;
}

pub fn c_value() -> i32 {
    unsafe { native_cc_c_value() }
}

pub fn cxx_value() -> i32 {
    unsafe { native_cc_cxx_value() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_c_and_cxx() {
        assert_eq!(c_value(), 7);
        assert_eq!(cxx_value(), 10);
    }

    #[test]
    fn compiler_is_the_selected_toolchain() {
        assert!(!CC_PATH.starts_with("/usr/"), "{CC_PATH}");
        assert!(!["cc", "gcc", "c++"].contains(&CC_PATH), "{CC_PATH}");
    }
}
