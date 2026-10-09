unsafe extern "C" {
    fn native_add(left: i32, right: i32) -> i32;
    fn native_offset() -> i32;
}

#[no_mangle]
pub extern "C" fn native_lib_add(left: i32, right: i32) -> i32 {
    unsafe { native_add(left, right) }
}

#[no_mangle]
pub extern "C" fn native_lib_offset() -> i32 {
    unsafe { native_offset() }
}

#[cfg(feature = "neon")]
#[no_mangle]
pub extern "C" fn native_lib_accelerated() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_add_reaches_the_native_dependency() {
        assert_eq!(unsafe { native_add(2, 3) }, 12);
        assert_eq!(unsafe { native_offset() }, 7);
    }
}
