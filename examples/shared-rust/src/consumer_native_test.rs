//! Asserts the independent consumer's relocated native output.

const PROBE: &str = "DX_SHARED_CONSUMER_PROBE";

#[test]
fn relocated_native_probe_matches_shared_core() {
    let rel = std::env::var(PROBE).expect("consumer probe env is set");
    let output = std::fs::read_to_string(dx_testing::resolve_runfiles(&rel)).expect("read probe");
    assert_eq!(output, "ok:14\n", "unexpected relocated probe output");
}
