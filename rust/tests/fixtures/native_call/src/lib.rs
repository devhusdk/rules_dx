include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
include!(concat!(env!("OUT_DIR"), "/build_probe.rs"));

#[cfg(has_native_call_stamp)]
const STAMP: u32 = 1;
#[cfg(not(has_native_call_stamp))]
const STAMP: u32 = 0;

pub fn stamped() -> u32 {
    STAMP
}

pub fn add(left: i32, right: i32) -> i32 {
    unsafe { native_add(left, right) }
}

pub fn offset() -> i32 {
    unsafe { native_offset() }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DivError {
    DivisionByZero,
}

pub fn div(numer: i32, denom: i32) -> Result<i32, DivError> {
    let mut err = 0;
    let value = unsafe { native_div(numer, denom, &mut err) };
    if err == 0 {
        Ok(value)
    } else {
        Err(DivError::DivisionByZero)
    }
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
    fn adds_across_the_boundary() {
        assert_eq!(add(2, 3), 5 + offset());
    }

    #[test]
    fn offset_matches_the_selected_copts() {
        assert!(offset() == 0 || offset() == 100);
    }

    #[test]
    fn divides_across_the_boundary() {
        assert_eq!(div(7, 2), Ok(3));
    }

    #[test]
    fn division_by_zero_is_an_error() {
        assert_eq!(div(1, 0), Err(DivError::DivisionByZero));
    }

    #[test]
    fn fixed_width_types_pin_the_abi() {
        assert_eq!(size_of::<i32>(), 4);
        assert_eq!(size_of::<*mut i32>(), size_of::<usize>());
    }

    #[test]
    fn build_script_ran_on_the_exec_host_for_this_target() {
        assert_eq!(BUILD_SCRIPT_HOST, BUILD_SCRIPT_TARGET);
        let host_arch = BUILD_SCRIPT_HOST.split('-').next();
        assert_eq!(host_arch, Some(std::env::consts::ARCH));
    }
}
