mod one;

#[test]
fn nested_helper_runs_inside_the_loading_root() {
    assert_eq!(one::nested_word(), "worker");
}
