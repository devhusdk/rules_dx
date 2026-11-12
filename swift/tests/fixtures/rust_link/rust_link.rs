unsafe extern "C" {
    fn swift_add(a: i32, b: i32) -> i32;
}

fn main() {
    assert_eq!(unsafe { swift_add(2, 3) }, 5);
}
