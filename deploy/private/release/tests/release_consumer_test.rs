//! The release candidate and README quickstart consumers, on one nested Bazel server.

mod candidate_consumer_test;
mod quickstart_consumer_test;

/// The instant the first stage was named.
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Names the step about to run and the seconds since the first step on the uncaptured stderr, so a
/// test killed mid-step shows where its time went.
pub(crate) fn stage(test: &str, name: &str) {
    use std::io::Write;
    let elapsed = START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs();
    drop(writeln!(std::io::stderr(), "{test}: [{elapsed}s] {name}"));
}
