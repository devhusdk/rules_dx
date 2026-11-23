#![expect(
    clippy::disallowed_methods,
    reason = "tests run local programs under the Bazel test timeout"
)]

mod candidate_consumer_test;
mod quickstart_consumer_test;

static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub(crate) fn stage(test: &str, name: &str) {
    use std::io::Write;
    let elapsed = START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs();
    writeln!(std::io::stderr(), "{test}: [{elapsed}s] {name}").expect("stderr is writable");
}
