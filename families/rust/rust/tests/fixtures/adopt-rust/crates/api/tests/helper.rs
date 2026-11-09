#[test]
fn helper_file_is_still_a_root() {
    assert_ne!(api::digest("api"), api::digest("worker"));
}
