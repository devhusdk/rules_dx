use support::fixture_word;

mod cases;

#[test]
fn digest_supports_the_fixture_word() {
    let hashed = api::digest(fixture_word());
    assert_eq!(hashed, api::digest("worker"));
}
