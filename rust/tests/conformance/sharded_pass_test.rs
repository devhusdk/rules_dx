//! Passing conformance subject split across test shards.

#[test]
fn first_case() {
    assert_eq!(conformance::greet("world"), "hello world");
}

#[test]
fn second_case() {
    assert_eq!(conformance::greet("world"), "hello world");
}
