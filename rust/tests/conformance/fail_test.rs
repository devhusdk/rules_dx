//! Failing conformance subject.

#[test]
fn failing_case() {
    assert_eq!(conformance::greet("world"), "goodbye world");
}
