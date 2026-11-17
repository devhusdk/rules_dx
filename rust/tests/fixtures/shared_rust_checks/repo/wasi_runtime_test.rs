#[test]
fn wrapped_runtime_adds_its_env() {
    assert_eq!(std::env::var("CONSUMER_RUNTIME").as_deref(), Ok("wrapped"));
}
