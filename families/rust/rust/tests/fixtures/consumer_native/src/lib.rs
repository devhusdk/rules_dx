include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
include!(concat!(env!("OUT_DIR"), "/build_probe.rs"));

#[cfg(has_consumer_native_stamp)]
const STAMP: u32 = 1;
#[cfg(not(has_consumer_native_stamp))]
const STAMP: u32 = 0;

pub fn stamped() -> u32 {
    STAMP
}

pub fn add(left: i32, right: i32) -> i32 {
    unsafe { consumer_add(left, right) }
}

pub fn offset() -> i32 {
    unsafe { consumer_offset() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_script_stamp_present() {
        assert_eq!(STAMP, 1);
        assert_eq!(stamped(), 1);
    }

    #[test]
    fn binding_version_is_pinned() {
        assert_eq!(BINDING_VERSION, 1);
    }

    #[test]
    fn consumer_dependency_keeps_its_own_offset() {
        assert_eq!(offset(), 7);
        assert_eq!(add(2, 3), 12);
    }

    #[test]
    fn build_script_ran_on_the_exec_host_for_this_target() {
        assert_eq!(BUILD_SCRIPT_HOST, BUILD_SCRIPT_TARGET);
        let host_arch = BUILD_SCRIPT_HOST.split('-').next();
        assert_eq!(host_arch, Some(std::env::consts::ARCH));
    }
}
