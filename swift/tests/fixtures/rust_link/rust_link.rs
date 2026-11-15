#![expect(
    unsafe_code,
    reason = "the fixture calls a Swift function through the C ABI"
)]

unsafe extern "C" {
    fn swift_add(a: i32, b: i32) -> i32;
}

#[test]
fn links_swift_add() {
    // SAFETY: swift_add takes plain integers and has no preconditions.
    assert_eq!(unsafe { swift_add(2, 3) }, 5);
}
